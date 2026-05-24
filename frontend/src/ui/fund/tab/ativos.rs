use crate::ui::design::Scale;
use chrono::{Datelike, Local, NaiveDate};
use egui::{Sense, Ui, Widget, WidgetText};
use egui_extras::{Column, DatePickerButton, TableBuilder};
use jiff::civil::{date as jiff_date, Date as JiffDate};
use std::sync::{Arc, Mutex};

use crate::{
    ui::{
        design::Components,
        fund::modal::asset_detail::{AssetDetailModal, AssetModalContext},
        tabs::Tab,
    },
    util,
};
use fundos_common::types::{AssetHolder, AssetHoldersResponse, MarketAsset, YahooPricePoint, YahooPriceResponse};

// ─── State ───────────────────────────────────────────────────────────────────
#[derive(Clone)]
pub struct AssetsMarketTab {
    pub title: String,
    pub data: Vec<MarketAsset>,
    pub loading: bool,
    // filter state
    pub start_date: JiffDate,
    pub end_date: JiffDate,
    pub filter_type: String, // "" = all
    pub search_text: String,
    pub active_preset_months: u32, // 0 = custom date range
    // detail modal
    pub selected_row: Option<usize>,
    pub asset_modal: AssetDetailModal,
    // async pending result
    pending_result: Option<Arc<Mutex<Option<Vec<MarketAsset>>>>>,
    // async pending Yahoo and Holders results for the modal
    pending_yahoo: Option<Arc<Mutex<Option<YahooPriceResponse>>>>,
    pending_holders: Option<Arc<Mutex<Option<AssetHoldersResponse>>>>,
    // ctx for repaint requests from spawned futures
    repaint_ctx: Option<egui::Context>,
}

impl AssetsMarketTab {
    pub fn new() -> Self {
        let now = Local::now().naive_local().date();
        let end = jiff_date(now.year() as i16, now.month() as i8, now.day() as i8);
        let start = end
            .checked_sub(jiff::Span::new().months(3))
            .unwrap_or(end - jiff::Span::new().days(183));

        let mut tab = Self {
            title: "Ativos do Mercado".to_string(),
            data: Vec::new(),
            loading: true,
            start_date: start,
            end_date: end,
            filter_type: String::new(),
            search_text: String::new(),
            active_preset_months: 3,
            selected_row: None,
            asset_modal: AssetDetailModal {
                context: AssetModalContext::GlobalMarket,
                ..Default::default()
            },
            pending_result: None,
            pending_yahoo: None,
            pending_holders: None,
            repaint_ctx: None,
        };

        tab.trigger_load();
        tab
    }

    /// Check for pending async results (called at start of ui())
    fn poll_pending(&mut self) {
        if let Some(pending) = self.pending_result.take() {
            let data = pending.try_lock().ok().and_then(|mut g| g.take());
            if let Some(data) = data {
                self.data = data;
                self.loading = false;
            } else {
                self.pending_result = Some(pending);
            }
        }
        if let Some(pending) = self.pending_yahoo.take() {
            let data = pending.try_lock().ok().and_then(|mut g| g.take());
            if let Some(resp) = data {
                self.asset_modal.yahoo_prices = Some(resp.prices);
                self.asset_modal.yahoo_loading = false;
            } else {
                self.pending_yahoo = Some(pending);
            }
        }
        if let Some(pending) = self.pending_holders.take() {
            let data = pending.try_lock().ok().and_then(|mut g| g.take());
            if let Some(resp) = data {
                self.asset_modal.holders_data = Some(resp.holders);
                self.asset_modal.holders_loading = false;
            } else {
                self.pending_holders = Some(pending);
            }
        }
    }

    // Collect unique TP_APLIC values from loaded data
    fn available_types(&self) -> Vec<String> {
        let mut types: Vec<String> = self
            .data
            .iter()
            .map(|a| a.tipo_aplic.clone())
            .filter(|s| !s.is_empty())
            .collect();
        types.sort();
        types.dedup();
        types
    }

    // Filter rows using iterators
    fn filtered_rows(&self) -> Vec<usize> {
        if self.data.is_empty() {
            return vec![];
        }
        let search = self.search_text.to_lowercase();
        let tp_filter = &self.filter_type;

        let has_filter = !tp_filter.is_empty() || !search.is_empty();

        if !has_filter {
            return (0..self.data.len()).collect();
        }

        self.data
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                let type_ok = tp_filter.is_empty() || a.tipo_aplic == *tp_filter;
                if !type_ok {
                    return false;
                }
                if search.is_empty() {
                    return true;
                }
                a.codigo_isin.to_lowercase().contains(&search)
                    || a.titpub.to_lowercase().contains(&search)
                    || a.tipo_ativo.to_lowercase().contains(&search)
                    || a.tipo_aplic.to_lowercase().contains(&search)
                    || a.cd_ativo.to_lowercase().contains(&search)
                    || a.ds_ativo.to_lowercase().contains(&search)
                    || a.nm_fundo_cota.to_lowercase().contains(&search)
                    || a.cd_selic.to_lowercase().contains(&search)
                    || a.codigo_negociacao.to_lowercase().contains(&search)
            })
            .map(|(i, _)| i)
            .collect()
    }

    // Trigger load with current date range
    fn trigger_load(&mut self) {
        self.active_preset_months = 0;
        self.loading = true;
        self.data.clear();

        let start_chrono = NaiveDate::from_ymd_opt(
            self.start_date.year() as i32,
            self.start_date.month() as u32,
            self.start_date.day() as u32,
        )
        .unwrap();
        let end_chrono = NaiveDate::from_ymd_opt(
            self.end_date.year() as i32,
            self.end_date.month() as u32,
            self.end_date.day() as u32,
        )
        .unwrap();

        let result: Arc<Mutex<Option<Vec<MarketAsset>>>> = Arc::new(Mutex::new(None));
        let r = result.clone();
        let ctx = self.repaint_ctx.clone();

        self.pending_result = Some(result);

        util::spawn_future(async move {
            match crate::api_client::get_market_assets(start_chrono, end_chrono).await {
                Ok(resp) => *r.lock().unwrap() = Some(resp.assets),
                Err(e) => log::error!("Market assets error: {}", e),
            }
            if let Some(ctx) = ctx {
                ctx.request_repaint();
            }
        });
    }

    // Apply a date preset and trigger load
    fn apply_preset(&mut self, months_back: u32) {
        self.active_preset_months = months_back;
        let now = Local::now().naive_local().date();
        let end = jiff_date(now.year() as i16, now.month() as i8, now.day() as i8);
        let start = end
            .checked_sub(jiff::Span::new().months(months_back))
            .unwrap_or(end - jiff::Span::new().days((months_back as i64) * 30));
        self.end_date = end;
        self.start_date = start;
        self.trigger_load();
    }

    // ── Render helpers ──────────────────────────────────────────────

    fn code_or_fallback(&self, asset: &MarketAsset) -> String {
        if !asset.codigo_isin.is_empty() {
            return asset.codigo_isin.clone();
        }
        if !asset.cd_ativo.is_empty() {
            return asset.cd_ativo.clone();
        }
        if !asset.cd_selic.is_empty() {
            return asset.cd_selic.clone();
        }
        if !asset.tipo_ativo.is_empty() {
            return asset.tipo_ativo.clone();
        }
        asset.tipo_aplic.clone()
    }

    fn name_or_fallback(&self, asset: &MarketAsset) -> String {
        if !asset.titpub.is_empty() {
            return asset.titpub.clone();
        }
        if !asset.ds_ativo.is_empty() {
            return asset.ds_ativo.clone();
        }
        if !asset.nm_fundo_cota.is_empty() {
            return asset.nm_fundo_cota.clone();
        }
        if !asset.tipo_ativo.is_empty() {
            return asset.tipo_ativo.clone();
        }
        asset.tipo_aplic.clone()
    }

    fn render_toolbar(&mut self, ui: &mut Ui) {
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new("PERÍODO").size(Scale::DEFAULT.label()).strong());
                ui.add_space(4.0);

                let dark = ui.visuals().dark_mode;
                for (label, months) in &[("3M", 3u32), ("6M", 6), ("1A", 12), ("2A", 24)] {
                    let active = self.active_preset_months == *months;
                    let resp = if active {
                        ui.add(
                            egui::Button::new(
                                egui::RichText::new(*label)
                                    .size(Scale::DEFAULT.small_text())
                                    .strong()
                                    .color(ui.visuals().window_fill),
                            )
                            .fill(ui.visuals().text_color())
                            .corner_radius(egui::CornerRadius::same(4)),
                        )
                    } else {
                        Components::ghost_button(ui, label, dark)
                    };
                    if resp.clicked() {
                        self.apply_preset(*months);
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ui.label(egui::RichText::new("De:").size(Scale::DEFAULT.label()));
                DatePickerButton::new(&mut self.start_date)
                    .id_salt("ativos_start_date")
                    .ui(ui);
                ui.label(egui::RichText::new("→").size(Scale::DEFAULT.small_text()).weak());
                ui.label(egui::RichText::new("Até:").size(Scale::DEFAULT.label()));
                DatePickerButton::new(&mut self.end_date)
                    .id_salt("ativos_end_date")
                    .ui(ui);

                let load_text = if self.loading {
                    format!("{} Carregando...", egui_phosphor::regular::CIRCLE_NOTCH)
                } else {
                    format!("{} Atualizar", egui_phosphor::regular::ARROW_CLOCKWISE)
                };
                if Components::primary_button(ui, &load_text).clicked() && !self.loading {
                    self.trigger_load();
                }
            });

            if !self.data.is_empty() {
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(5.0);

                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("TIPO").size(Scale::DEFAULT.label()).strong());
                    ui.add_space(6.0);
                    let all_selected = self.filter_type.is_empty();
                    if ui
                        .selectable_label(all_selected, egui::RichText::new("Todos").size(Scale::DEFAULT.label()))
                        .clicked()
                    {
                        self.filter_type.clear();
                    }
                    for tp in self.available_types() {
                        let selected = self.filter_type == tp;
                        if ui
                            .selectable_label(selected, egui::RichText::new(&tp).size(Scale::DEFAULT.label()))
                            .clicked()
                        {
                            self.filter_type = if selected { String::new() } else { tp };
                        }
                    }
                });
            }
        });
    }

    fn render_top4_cards(&self, ui: &mut Ui, rows: &[usize]) {
        if rows.is_empty() {
            return;
        }

        let top_rows: Vec<usize> = rows.iter().take(4).copied().collect();

        let accent_colors = [
            egui::Color32::from_rgb(26, 115, 232),
            egui::Color32::from_rgb(106, 27, 154),
            egui::Color32::from_rgb(19, 115, 51),
            egui::Color32::from_rgb(176, 96, 0),
        ];

        let dark = ui.visuals().dark_mode;
        ui.columns(top_rows.len().max(1), |cols| {
            for (card_idx, &row) in top_rows.iter().enumerate() {
                let col_ui = &mut cols[card_idx];
                let accent = accent_colors[card_idx % accent_colors.len()];
                let asset = &self.data[row];

                let codigo = self.code_or_fallback(asset);
                let display_nome = self.name_or_fallback(asset);

                Components::card(col_ui, dark, 8, |ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(120.0);
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(6.0, 16.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, egui::CornerRadius::same(3), accent);
                            ui.add_space(4.0);
                            let name = &display_nome;
                            ui.label(
                                egui::RichText::new(format!(
                                    "{}º {}",
                                    card_idx + 1,
                                    &name[..name.len().min(28)]
                                ))
                                .size(Scale::DEFAULT.label())
                                .strong(),
                            );
                        });
                        ui.add_space(2.0);
                        if !codigo.is_empty() {
                            ui.label(egui::RichText::new(format!("ISIN: {}", codigo)).size(Scale::DEFAULT.badge()));
                        }
                        if !asset.dt_venc.is_empty() {
                            ui.label(egui::RichText::new(format!("Venc: {}", asset.dt_venc)).size(Scale::DEFAULT.badge()));
                        }
                        ui.separator();

                        kv_row(ui, "Consenso:", &format!("{} fundos", asset.fund_count), accent);
                        kv_row(ui, "Volume:", &fmt_currency(asset.total_market_value), egui::Color32::PLACEHOLDER);
                        kv_row(ui, "PL Médio:", &fmt_currency(asset.pl_medio), egui::Color32::PLACEHOLDER);
                    });
                });
            }
        });
    }

    fn render_table(&mut self, ui: &mut Ui, rows: &[usize]) {
        if self.data.is_empty() {
            return;
        }

        Components::configure_table(TableBuilder::new(ui))
            .resizable(true)
            .sense(Sense::click())
            .column(Column::exact(36.0))
            .column(Column::initial(130.0).at_least(90.0))
            .column(Column::initial(Scale::COLUMN_MIN).at_least(100.0))
            .column(Column::initial(120.0).at_least(80.0))
            .column(Column::initial(70.0).at_least(50.0))
            .column(Column::initial(70.0).at_least(50.0))
            .column(Column::initial(70.0).at_least(50.0))
            .column(Column::initial(105.0).at_least(75.0))
            .column(Column::initial(105.0).at_least(75.0))
            .column(Column::initial(105.0).at_least(75.0))
            .column(Column::initial(105.0).at_least(75.0))
            .column(Column::initial(105.0).at_least(75.0))
            .column(Column::initial(105.0).at_least(75.0))
            .column(Column::remainder().at_least(85.0))
            .header(Scale::DEFAULT.table_header_height(), |mut header| {
                let headers = [
                    "#", "Código", "Nome", "Tipo", "Fundos", "Comp.", "Vend.",
                    "Vl Comprado", "Vl Vendido", "PL Médio", "VL Mín", "VL Máx",
                    "VL Méd", "Vol Total",
                ];
                for h in headers.iter() {
                    header.col(|ui| {
                        ui.label(egui::RichText::new(*h).size(Scale::DEFAULT.small_text()).strong());
                    });
                }
            })
            .body(|body| {
                let row_count = rows.len().min(500);
                body.rows(Scale::DEFAULT.table_row_height(), row_count, |mut row| {
                    let rank = row.index();
                    let data_row = rows[rank];
                    let asset = &self.data[data_row];

                    row.col(|ui| {
                        let (badge_color, txt_color) = match rank {
                            0 => (egui::Color32::from_rgb(234, 179, 8), egui::Color32::WHITE),
                            1 => (egui::Color32::from_rgb(148, 163, 184), egui::Color32::WHITE),
                            2 => (egui::Color32::from_rgb(180, 120, 60), egui::Color32::WHITE),
                            _ => (egui::Color32::TRANSPARENT, ui.visuals().weak_text_color()),
                        };
                        let label = format!("{}", rank + 1);
                        if badge_color != egui::Color32::TRANSPARENT {
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(26.0, 16.0), egui::Sense::hover());
                            ui.painter().rect_filled(rect, egui::CornerRadius::same(4), badge_color);
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                &label,
                                egui::FontId::proportional(10.0),
                                txt_color,
                            );
                        } else {
                            ui.label(egui::RichText::new(label).size(Scale::DEFAULT.label()));
                        }
                    });

                    row.col(|ui| {
                        let display = self.code_or_fallback(asset);
                        ui.label(egui::RichText::new(&display).size(Scale::DEFAULT.label()).monospace());
                    });

                    row.col(|ui| {
                        let display = self.name_or_fallback(asset);
                        ui.label(egui::RichText::new(display).size(Scale::DEFAULT.label()))
                            .on_hover_text(&asset.titpub);
                    });

                    row.col(|ui| {
                        ui.label(egui::RichText::new(&asset.tipo_aplic).size(Scale::DEFAULT.table_cell()));
                    });

                    row.col(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{}", asset.fund_count))
                                .size(Scale::DEFAULT.table_cell())
                                .strong()
                                .color(egui::Color32::from_rgb(26, 115, 232)),
                        );
                    });

                    row.col(|ui| {
                        if asset.n_compradores > 0 {
                            ui.label(
                                egui::RichText::new(format!("{}", asset.n_compradores))
                                    .size(Scale::DEFAULT.label())
                                    .strong()
                                    .color(egui::Color32::from_rgb(34, 197, 94)),
                            );
                        } else {
                            ui.label(egui::RichText::new("-").size(Scale::DEFAULT.label()));
                        }
                    });

                    row.col(|ui| {
                        if asset.n_vendedores > 0 {
                            ui.label(
                                egui::RichText::new(format!("{}", asset.n_vendedores))
                                    .size(Scale::DEFAULT.label())
                                    .strong()
                                    .color(egui::Color32::from_rgb(239, 68, 68)),
                            );
                        } else {
                            ui.label(egui::RichText::new("-").size(Scale::DEFAULT.label()));
                        }
                    });

                    row.col(|ui| {
                        ui.label(
                            egui::RichText::new(fmt_currency(asset.vl_comprado))
                                .size(Scale::DEFAULT.label())
                                .color(egui::Color32::from_rgb(34, 197, 94)),
                        );
                    });

                    row.col(|ui| {
                        ui.label(
                            egui::RichText::new(fmt_currency(asset.vl_vendido))
                                .size(Scale::DEFAULT.label())
                                .color(egui::Color32::from_rgb(239, 68, 68)),
                        );
                    });

                    row.col(|ui| {
                        ui.label(egui::RichText::new(fmt_currency(asset.pl_medio)).size(Scale::DEFAULT.label()).strong());
                    });

                    row.col(|ui| {
                        let v = asset.vl_min;
                        ui.label(
                            egui::RichText::new(if v == 0.0 { "-".to_string() } else { fmt_currency(v) })
                                .size(Scale::DEFAULT.table_cell()),
                        );
                    });

                    row.col(|ui| {
                        ui.label(egui::RichText::new(fmt_currency(asset.vl_max)).size(Scale::DEFAULT.table_cell()).strong());
                    });

                    row.col(|ui| {
                        ui.label(egui::RichText::new(fmt_currency(asset.vl_medio)).size(Scale::DEFAULT.table_cell()));
                    });

                    row.col(|ui| {
                        ui.label(
                            egui::RichText::new(fmt_currency(asset.total_market_value))
                                .size(Scale::DEFAULT.caption())
                                .strong()
                                .color(egui::Color32::from_rgb(139, 92, 246)),
                        );
                    });

                    if row.response().clicked() {
                        self.selected_row = if self.selected_row == Some(data_row) { None } else { Some(data_row) };
                    }
                });
            });
    }
}

// ─── Tab impl ────────────────────────────────────────────────────────────────
impl Tab for AssetsMarketTab {
    fn title(&self) -> WidgetText {
        format!("{} {}", egui_phosphor::regular::CHART_BAR, self.title).into()
    }

    fn closeable(&self) -> bool { true }

    fn ui(&mut self, ui: &mut Ui) {
        // Save ctx for repaint requests from async tasks
        self.repaint_ctx = Some(ui.ctx().clone());

        // Check for pending async results
        self.poll_pending();

        let dark = ui.visuals().dark_mode;
        Components::card(ui, dark, 8, |ui| {
            ui.vertical(|ui| {
                self.render_toolbar(ui);
                ui.add_space(6.0);

                if self.loading {
                    crate::ui::loading::show_custom(
                        ui,
                        "Analisando carteiras do mercado...\nAgregando dados de todos os fundos no período selecionado.",
                        egui_phosphor::regular::CHART_BAR,
                    );
                    return;
                }

                if self.data.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        ui.label(
                            egui::RichText::new("Nenhum dado encontrado. Tente ajustar o período ou verifique sua conexão.")
                                .size(Scale::DEFAULT.small_text())
                                .weak(),
                        );
                    });
                    return;
                }

                let filtered = self.filtered_rows();
                let total = filtered.len();

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} {} ativos encontrados",
                            egui_phosphor::regular::FUNNEL,
                            total
                        ))
                        .size(Scale::DEFAULT.small_text()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let start_fmt = format!(
                            "{:02}/{:02}/{:04}",
                            self.start_date.day(),
                            self.start_date.month(),
                            self.start_date.year()
                        );
                        let end_fmt = format!(
                            "{:02}/{:02}/{:04}",
                            self.end_date.day(),
                            self.end_date.month(),
                            self.end_date.year()
                        );
                        ui.label(
                            egui::RichText::new(format!("Período: {} → {}", start_fmt, end_fmt))
                                .size(Scale::DEFAULT.label()),
                        );
                    });
                });
                ui.add_space(4.0);

                if total > 0 {
                    self.render_top4_cards(ui, &filtered);
                    ui.add_space(8.0);
                }

                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS.to_string())
                                .size(Scale::DEFAULT.button())
                                .weak(),
                        );
                        ui.add(
                            egui::TextEdit::singleline(&mut self.search_text)
                                .desired_width(240.0)
                                .hint_text("Buscar por ISIN, nome ou tipo..."),
                        );
                    });
                    ui.add_space(4.0);
                    egui::ScrollArea::horizontal().show(ui, |ui| {
                        self.render_table(ui, &filtered);
                    });
                });

                // Detail modal — populate from selected row data
                if let Some(row) = self.selected_row {
                    if let Some(asset) = self.data.get(row) {
                        let codigo = if !asset.cd_ativo.is_empty() {
                            asset.cd_ativo.clone()
                        } else if !asset.codigo_isin.is_empty() {
                            asset.codigo_isin.clone()
                        } else {
                            String::new()
                        };
                        let nome_display = if !asset.titpub.is_empty() {
                            asset.titpub.clone()
                        } else if !asset.ds_ativo.is_empty() {
                            asset.ds_ativo.clone()
                        } else {
                            asset.tipo_ativo.clone()
                        };

                        self.asset_modal.title = format!("Detalhes - {}", nome_display);
                        self.asset_modal.codigo = codigo;
                        self.asset_modal.nome = nome_display;
                        self.asset_modal.tp_ativo = asset.tipo_ativo.clone();
                        self.asset_modal.tp_aplic = asset.tipo_aplic.clone();
                        self.asset_modal.dt_venc = asset.dt_venc.clone();
                        self.asset_modal.nm_fundo = asset.nm_fundo_cota.clone();
                        self.asset_modal.cnpj = String::new();
                        self.asset_modal.context = AssetModalContext::GlobalMarket;
                        self.asset_modal.n_fundos = asset.fund_count as u64;
                        self.asset_modal.n_compradores = asset.n_compradores as u64;
                        self.asset_modal.n_vendedores = asset.n_vendedores as u64;
                        self.asset_modal.vl_mercado = asset.total_market_value;
                        self.asset_modal.vl_aquisicao = 0.0;
                        self.asset_modal.pl_medio = asset.pl_medio;
                        self.asset_modal.vl_min = asset.vl_min;
                        self.asset_modal.vl_max = asset.vl_max;
                        self.asset_modal.vl_medio = asset.vl_medio;
                        self.asset_modal.vl_compra_min = asset.vl_compra_min;
                        self.asset_modal.vl_compra_max = asset.vl_compra_max;
                        self.asset_modal.vl_compra_medio = asset.vl_compra_medio;
                        self.asset_modal.vl_comprado = asset.vl_comprado;
                        self.asset_modal.vl_vendido = asset.vl_vendido;
                        self.asset_modal.pct_pl = asset.avg_percentage;
                        self.asset_modal.open_with_auto_fetch();
                    }
                    self.selected_row = None;
                }

                // Handle modal Yahoo/Holders requests via Arc<Mutex> pattern
                if self.asset_modal.request_yahoo {
                    self.asset_modal.request_yahoo = false;
                    let codigo = self.asset_modal.codigo.clone();
                    let result: Arc<Mutex<Option<YahooPriceResponse>>> = Arc::new(Mutex::new(None));
                    let r = result.clone();
                    self.pending_yahoo = Some(result);
                    let ctx = ui.ctx().clone();
                    util::spawn_future(async move {
                        let end = chrono::Local::now().naive_local().date();
                        let start = end.checked_sub_months(chrono::Months::new(24))
                            .unwrap_or(end - chrono::Duration::days(730));
                        match crate::api_client::get_yahoo_prices(&codigo, start, end).await {
                            Ok(resp) => *r.lock().unwrap() = Some(resp),
                            Err(e) => log::error!("Yahoo error: {}", e),
                        }
                        ctx.request_repaint();
                    });
                }
                if self.asset_modal.request_holders {
                    self.asset_modal.request_holders = false;
                    let codigo = self.asset_modal.codigo.clone();
                    let result: Arc<Mutex<Option<AssetHoldersResponse>>> = Arc::new(Mutex::new(None));
                    let r = result.clone();
                    self.pending_holders = Some(result);
                    let ctx = ui.ctx().clone();
                    util::spawn_future(async move {
                        let end = chrono::Local::now().naive_local().date();
                        let start = end.checked_sub_months(chrono::Months::new(24))
                            .unwrap_or(end - chrono::Duration::days(730));
                        match crate::api_client::get_asset_holders(&codigo, start, end).await {
                            Ok(resp) => *r.lock().unwrap() = Some(resp),
                            Err(e) => log::error!("Holders error: {}", e),
                        }
                        ctx.request_repaint();
                    });
                }

                self.asset_modal.show(ui);
            });
        });
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn fmt_currency(value: f64) -> String {
    if value.abs() >= 1_000_000_000.0 {
        format!("R$ {:.2} Bi", value / 1_000_000_000.0)
    } else if value.abs() >= 1_000_000.0 {
        format!("R$ {:.2} Mi", value / 1_000_000.0)
    } else if value.abs() >= 1_000.0 {
        format!("R$ {:.2} Mil", value / 1_000.0)
    } else if value == 0.0 {
        "-".to_string()
    } else {
        format!("R$ {:.2}", value)
    }
}

fn kv_row(ui: &mut Ui, label: &str, value: &str, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(Scale::DEFAULT.badge()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if color == egui::Color32::PLACEHOLDER {
                ui.label(egui::RichText::new(value).size(Scale::DEFAULT.label()).strong());
            } else {
                ui.label(egui::RichText::new(value).size(Scale::DEFAULT.label()).strong().color(color));
            }
        });
    });
}
