use chrono::{Datelike, Local, NaiveDate};

use egui::{Frame, Sense, Ui, Widget, WidgetText};
use egui_extras::{Column, DatePickerButton, TableBuilder};
use jiff::civil::{date as jiff_date, Date as JiffDate};
use polars::frame::DataFrame;
use tokio::sync::mpsc::UnboundedSender;

use crate::{message::Message, ui::fund::modal::asset_detail::AssetDetailModal, ui::tabs::Tab};

// ─── State ───────────────────────────────────────────────────────────────────
pub struct AssetsMarketTab {
    pub title: String,
    pub sender: UnboundedSender<Message>,
    pub data: DataFrame,
    pub loading: bool,
    // filter state
    pub start_date: JiffDate,
    pub end_date: JiffDate,
    pub filter_type: String, // "" = all
    pub search_text: String,
    // detail modal
    pub selected_row: Option<usize>,
    pub asset_modal: AssetDetailModal,
}

impl AssetsMarketTab {
    pub fn new(sender: UnboundedSender<Message>) -> Self {
        let now = Local::now().naive_local().date();
        let end = jiff_date(now.year() as i16, now.month() as i8, now.day() as i8);
        let start = end
            .checked_sub(jiff::Span::new().months(3))
            .unwrap_or(end - jiff::Span::new().days(183));

        // Auto-trigger load
        let start_chrono = NaiveDate::from_ymd_opt(
            start.year() as i32,
            start.month() as u32,
            start.day() as u32,
        )
        .unwrap();
        let end_chrono =
            NaiveDate::from_ymd_opt(end.year() as i32, end.month() as u32, end.day() as u32)
                .unwrap();
        let _ = sender.send(Message::OpenAtivosTab(start_chrono, end_chrono));

        Self {
            title: "Ativos do Mercado".to_string(),
            sender,
            data: DataFrame::empty(),
            loading: true,
            start_date: start,
            end_date: end,
            filter_type: String::new(),
            search_text: String::new(),
            selected_row: None,
            asset_modal: AssetDetailModal::default(),
        }
    }

    pub fn set_data(&mut self, df: DataFrame) {
        self.data = df;
        self.loading = false;
    }

    // Collect unique TP_APLIC values from loaded data
    fn available_types(&self) -> Vec<String> {
        let Ok(col) = self.data.column("TP_APLIC") else {
            return vec![];
        };
        let mut types: Vec<String> = (0..col.len())
            .filter_map(|i| col.get(i).ok()?.get_str().map(|s| s.to_string()))
            .filter(|s| !s.is_empty())
            .collect();
        types.sort();
        types.dedup();
        types
    }

    // Filter rows based on current search_text and filter_type
    fn filtered_rows(&self) -> Vec<usize> {
        let height = self.data.height();
        if height == 0 {
            return vec![];
        }

        let tp_col = self.data.column("TP_APLIC").ok();
        let isin_col = self.data.column("CD_ISIN").ok();
        let nome_col = self.data.column("TP_TITPUB").ok();
        let ativo_col = self.data.column("TP_ATIVO").ok();
        let cd_ativo_col = self.data.column("CD_ATIVO").ok();
        let ds_ativo_col = self.data.column("DS_ATIVO").ok();
        let nm_fundo_col = self.data.column("NM_FUNDO_COTA").ok();
        let cd_selic_col = self.data.column("CD_SELIC").ok();

        let search = self.search_text.to_lowercase();

        (0..height)
            .filter(|&i| {
                // type filter
                if !self.filter_type.is_empty() {
                    let tp = tp_col
                        .as_ref()
                        .and_then(|c| c.get(i).ok())
                        .and_then(|v| v.get_str().map(|s| s.to_string()))
                        .unwrap_or_default();
                    if tp != self.filter_type {
                        return false;
                    }
                }

                // text search
                if !search.is_empty() {
                    let isin = isin_col
                        .as_ref()
                        .and_then(|c| c.get(i).ok())
                        .and_then(|v| v.get_str().map(|s| s.to_lowercase()))
                        .unwrap_or_default();
                    let nome = nome_col
                        .as_ref()
                        .and_then(|c| c.get(i).ok())
                        .and_then(|v| v.get_str().map(|s| s.to_lowercase()))
                        .unwrap_or_default();
                    let ativo = ativo_col
                        .as_ref()
                        .and_then(|c| c.get(i).ok())
                        .and_then(|v| v.get_str().map(|s| s.to_lowercase()))
                        .unwrap_or_default();
                    let aplic = tp_col
                        .as_ref()
                        .and_then(|c| c.get(i).ok())
                        .and_then(|v| v.get_str().map(|s| s.to_lowercase()))
                        .unwrap_or_default();
                    let cd_ativo = cd_ativo_col
                        .as_ref()
                        .and_then(|c| c.get(i).ok())
                        .and_then(|v| v.get_str().map(|s| s.to_lowercase()))
                        .unwrap_or_default();
                    let ds_ativo = ds_ativo_col
                        .as_ref()
                        .and_then(|c| c.get(i).ok())
                        .and_then(|v| v.get_str().map(|s| s.to_lowercase()))
                        .unwrap_or_default();
                    let nm_fundo = nm_fundo_col
                        .as_ref()
                        .and_then(|c| c.get(i).ok())
                        .and_then(|v| v.get_str().map(|s| s.to_lowercase()))
                        .unwrap_or_default();
                    let cd_selic = cd_selic_col
                        .as_ref()
                        .and_then(|c| c.get(i).ok())
                        .and_then(|v| v.get_str().map(|s| s.to_lowercase()))
                        .unwrap_or_default();
                    if !isin.contains(&search)
                        && !nome.contains(&search)
                        && !ativo.contains(&search)
                        && !aplic.contains(&search)
                        && !cd_ativo.contains(&search)
                        && !ds_ativo.contains(&search)
                        && !nm_fundo.contains(&search)
                        && !cd_selic.contains(&search)
                    {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    // Trigger load with current date range
    fn trigger_load(&mut self) {
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
        self.loading = true;
        self.data = DataFrame::empty();
        let _ = self
            .sender
            .send(Message::OpenAtivosTab(start_chrono, end_chrono));
    }

    // Apply a date preset and trigger load
    fn apply_preset(&mut self, months_back: u32) {
        let now = Local::now().naive_local().date();
        let end = jiff_date(now.year() as i16, now.month() as i8, now.day() as i8);
        let start = end
            .checked_sub(jiff::Span::new().months(months_back))
            .unwrap_or(end - jiff::Span::new().days((months_back as i64) * 30));
        self.end_date = end;
        self.start_date = start;

        let start_chrono = NaiveDate::from_ymd_opt(
            start.year() as i32,
            start.month() as u32,
            start.day() as u32,
        )
        .unwrap();
        let end_chrono =
            NaiveDate::from_ymd_opt(end.year() as i32, end.month() as u32, end.day() as u32)
                .unwrap();
        self.loading = true;
        self.data = DataFrame::empty();
        let _ = self
            .sender
            .send(Message::OpenAtivosTab(start_chrono, end_chrono));
    }

    // Render toolbar
    fn render_toolbar(&mut self, ui: &mut Ui) {
        ui.group(|ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                // Label de período
                ui.label(egui::RichText::new("PERÍODO").size(11.0).strong());
                ui.add_space(4.0);

                // Preset buttons styled as pills
                for (label, months) in &[("3M", 3u32), ("6M", 6), ("1A", 12), ("2A", 24)] {
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new(*label).size(11.0).strong())
                                .min_size(egui::vec2(32.0, 22.0)),
                        )
                        .clicked()
                    {
                        self.apply_preset(*months);
                    }
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // Date range inputs
                ui.label(egui::RichText::new("De:").size(11.0));
                DatePickerButton::new(&mut self.start_date)
                    .id_salt("ativos_start_date")
                    .ui(ui);
                ui.label(egui::RichText::new("→").size(12.0).weak());
                ui.label(egui::RichText::new("Até:").size(11.0));
                DatePickerButton::new(&mut self.end_date)
                    .id_salt("ativos_end_date")
                    .ui(ui);

                // Load button — blue accent
                let load_text = if self.loading {
                    format!("{} Carregando...", egui_phosphor::regular::CIRCLE_NOTCH)
                } else {
                    format!("{} Atualizar", egui_phosphor::regular::ARROW_CLOCKWISE)
                };
                let load_btn = egui::Button::new(
                    egui::RichText::new(&load_text)
                        .size(11.5)
                        .color(egui::Color32::WHITE),
                )
                .fill(egui::Color32::from_rgb(37, 99, 235))
                .min_size(egui::vec2(100.0, 24.0));
                if ui.add(load_btn).clicked() && !self.loading {
                    self.trigger_load();
                }
            });

            if !self.data.is_empty() {
                // Type filter chips row
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(5.0);

                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("TIPO").size(11.0).strong());
                    ui.add_space(6.0);
                    let all_selected = self.filter_type.is_empty();
                    if ui
                        .selectable_label(all_selected, egui::RichText::new("Todos").size(11.0))
                        .clicked()
                    {
                        self.filter_type.clear();
                    }
                    for tp in self.available_types() {
                        let selected = self.filter_type == tp;
                        if ui
                            .selectable_label(selected, egui::RichText::new(&tp).size(11.0))
                            .clicked()
                        {
                            self.filter_type = if selected { String::new() } else { tp };
                        }
                    }
                });
            }
        });
    }

    // Render dynamic top-4 cards
    fn render_top4_cards(&self, ui: &mut Ui, rows: &[usize]) {
        if rows.is_empty() {
            return;
        }

        let isin_col = self.data.column("CD_ISIN").ok();
        let ativo_col = self.data.column("TP_ATIVO").ok();
        let titpub_col = self.data.column("TP_TITPUB").ok();
        let aplic_col = self.data.column("TP_APLIC").ok();
        let cd_ativo_col = self.data.column("CD_ATIVO").ok();
        let ds_ativo_col = self.data.column("DS_ATIVO").ok();
        let nm_fundo_col = self.data.column("NM_FUNDO_COTA").ok();
        let cd_selic_col = self.data.column("CD_SELIC").ok();
        let venc_col = self.data.column("DT_VENC").ok();
        let n_fundos_col = self.data.column("N_FUNDOS").ok();
        let merc_col = self.data.column("VL_MERC_TOTAL").ok();
        let pl_col = self.data.column("PL_MEDIO").ok();

        let top_rows: Vec<usize> = rows.iter().take(4).copied().collect();

        let accent_colors = [
            egui::Color32::from_rgb(26, 115, 232), // blue
            egui::Color32::from_rgb(106, 27, 154), // purple
            egui::Color32::from_rgb(19, 115, 51),  // green
            egui::Color32::from_rgb(176, 96, 0),   // amber
        ];

        ui.columns(top_rows.len().max(1), |cols| {
            for (card_idx, &row) in top_rows.iter().enumerate() {
                let col_ui = &mut cols[card_idx];
                let accent = accent_colors[card_idx % accent_colors.len()];

                let isin = get_str(&isin_col, row);
                let ativo = get_str(&ativo_col, row);
                let nome = get_str(&titpub_col, row);
                let aplic = get_str(&aplic_col, row);
                let cd_ativo = get_str(&cd_ativo_col, row);
                let ds_ativo = get_str(&ds_ativo_col, row);
                let nm_fundo = get_str(&nm_fundo_col, row);
                let cd_selic = get_str(&cd_selic_col, row);
                let venc = get_str(&venc_col, row);

                let codigo = (!isin.is_empty())
                    .then_some(isin.clone())
                    .or_else(|| (!cd_ativo.is_empty()).then_some(cd_ativo.clone()))
                    .or_else(|| (!cd_selic.is_empty()).then_some(cd_selic.clone()))
                    .or_else(|| (!ativo.is_empty()).then_some(ativo.clone()))
                    .or_else(|| (!aplic.is_empty()).then_some(aplic.clone()))
                    .unwrap_or_default();

                let display_nome = (!nome.is_empty())
                    .then_some(nome)
                    .or_else(|| (!ds_ativo.is_empty()).then_some(ds_ativo))
                    .or_else(|| (!nm_fundo.is_empty()).then_some(nm_fundo))
                    .or_else(|| (!ativo.is_empty()).then_some(ativo))
                    .or_else(|| (!aplic.is_empty()).then_some(aplic))
                    .unwrap_or_default();

                let n_fundos: u64 = n_fundos_col
                    .as_ref()
                    .and_then(|c| c.get(row).ok())
                    .and_then(|v| v.try_extract::<u64>().ok())
                    .unwrap_or(0);

                let vl_merc: f64 = merc_col
                    .as_ref()
                    .and_then(|c| c.get(row).ok())
                    .and_then(|v| v.try_extract::<f64>().ok())
                    .unwrap_or(0.0);

                let pl_medio: f64 = pl_col
                    .as_ref()
                    .and_then(|c| c.get(row).ok())
                    .and_then(|v| v.try_extract::<f64>().ok())
                    .unwrap_or(0.0);

                Frame::group(col_ui.style())
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(col_ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.set_height(120.0);
                        ui.vertical(|ui| {
                            // Rank + title row
                            ui.horizontal(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(6.0, 16.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter()
                                    .rect_filled(rect, egui::CornerRadius::same(3), accent);
                                ui.add_space(4.0);
                                ui.label(
                                    egui::RichText::new(format!(
                                        "{}º {}",
                                        card_idx + 1,
                                        &display_nome[..display_nome.len().min(28)]
                                    ))
                                    .size(11.0)
                                    .strong(),
                                );
                            });
                            ui.add_space(2.0);
                            if !codigo.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!("ISIN: {}", codigo)).size(10.0),
                                );
                            }
                            if !venc.is_empty() {
                                ui.label(egui::RichText::new(format!("Venc: {}", venc)).size(10.0));
                            }
                            ui.separator();

                            kv_row(ui, "Consenso:", &format!("{} fundos", n_fundos), accent);
                            kv_row(
                                ui,
                                "Volume:",
                                &fmt_currency(vl_merc),
                                egui::Color32::PLACEHOLDER,
                            );
                            kv_row(
                                ui,
                                "PL Médio:",
                                &fmt_currency(pl_medio),
                                egui::Color32::PLACEHOLDER,
                            );
                        });
                    });
            }
        });
    }

    // Render the ranked table
    fn render_table(&mut self, ui: &mut Ui, rows: &[usize]) {
        if self.data.is_empty() {
            return;
        }

        let isin_col = self.data.column("CD_ISIN").ok();
        let ativo_col = self.data.column("TP_ATIVO").ok();
        let titpub_col = self.data.column("TP_TITPUB").ok();
        let tp_aplic_col = self.data.column("TP_APLIC").ok();
        let cd_ativo_col = self.data.column("CD_ATIVO").ok();
        let ds_ativo_col = self.data.column("DS_ATIVO").ok();
        let nm_fundo_col = self.data.column("NM_FUNDO_COTA").ok();
        let cd_selic_col = self.data.column("CD_SELIC").ok();

        let code_or_fallback = |row: usize| -> String {
            let isin = get_str(&isin_col, row);
            if !isin.is_empty() {
                return isin;
            }
            let cd_ativo = get_str(&cd_ativo_col, row);
            if !cd_ativo.is_empty() {
                return cd_ativo;
            }
            let cd_selic = get_str(&cd_selic_col, row);
            if !cd_selic.is_empty() {
                return cd_selic;
            }
            let ativo = get_str(&ativo_col, row);
            if !ativo.is_empty() {
                return ativo;
            }
            get_str(&tp_aplic_col, row)
        };

        let name_or_fallback = |row: usize| -> String {
            let nome = get_str(&titpub_col, row);
            if !nome.is_empty() {
                return nome;
            }
            let ds_ativo = get_str(&ds_ativo_col, row);
            if !ds_ativo.is_empty() {
                return ds_ativo;
            }
            let nm_fundo = get_str(&nm_fundo_col, row);
            if !nm_fundo.is_empty() {
                return nm_fundo;
            }
            let ativo = get_str(&ativo_col, row);
            if !ativo.is_empty() {
                return ativo;
            }
            get_str(&tp_aplic_col, row)
        };
        let n_fundos_col = self.data.column("N_FUNDOS").ok();
        let n_comp_col = self.data.column("N_COMPRADORES").ok();
        let n_vend_col = self.data.column("N_VENDEDORES").ok();
        let comprado_col = self.data.column("VL_COMPRADO").ok();
        let vendido_col = self.data.column("VL_VENDIDO").ok();
        let pl_medio_col = self.data.column("PL_MEDIO").ok();
        let merc_col = self.data.column("VL_MERC_TOTAL").ok();
        let vl_min_col = self.data.column("VL_MIN").ok();
        let vl_max_col = self.data.column("VL_MAX").ok();
        let vl_medio_col = self.data.column("VL_MEDIO").ok();

        TableBuilder::new(ui)
            .striped(true)
            .resizable(true)
            .sense(Sense::click())
            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
            .column(Column::exact(36.0))
            .column(Column::initial(130.0).at_least(90.0))
            .column(Column::initial(200.0).at_least(100.0))
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
            .header(26.0, |mut header| {
                let headers = [
                    "#",
                    "Código",
                    "Nome",
                    "Tipo",
                    "Fundos",
                    "Comp.",
                    "Vend.",
                    "Vl Comprado",
                    "Vl Vendido",
                    "PL Médio",
                    "VL Mín",
                    "VL Máx",
                    "VL Méd",
                    "Vol Total",
                ];
                for h in headers.iter() {
                    header.col(|ui| {
                        ui.label(egui::RichText::new(*h).size(12.0).strong());
                    });
                }
            })
            .body(|body| {
                let row_count = rows.len().min(500);
                body.rows(20.0, row_count, |mut row| {
                    let rank = row.index();
                    let data_row = rows[rank];

                    // # rank — badge style
                    row.col(|ui| {
                        let (badge_color, txt_color) = match rank {
                            0 => (egui::Color32::from_rgb(234, 179, 8), egui::Color32::WHITE),
                            1 => (egui::Color32::from_rgb(148, 163, 184), egui::Color32::WHITE),
                            2 => (egui::Color32::from_rgb(180, 120, 60), egui::Color32::WHITE),
                            _ => (egui::Color32::TRANSPARENT, ui.visuals().weak_text_color()),
                        };
                        let label = format!("{}", rank + 1);
                        if badge_color != egui::Color32::TRANSPARENT {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(26.0, 16.0), egui::Sense::hover());
                            ui.painter().rect_filled(
                                rect,
                                egui::CornerRadius::same(4),
                                badge_color,
                            );
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                &label,
                                egui::FontId::proportional(10.0),
                                txt_color,
                            );
                        } else {
                            ui.label(egui::RichText::new(label).size(11.0));
                        }
                    });

                    // Código (ISIN preferred)
                    row.col(|ui| {
                        let display = code_or_fallback(data_row);
                        ui.label(egui::RichText::new(&display).size(11.0).monospace());
                    });

                    // Nome
                    row.col(|ui| {
                        let display = name_or_fallback(data_row);
                        ui.label(egui::RichText::new(display).size(11.0))
                            .on_hover_text(get_str(&titpub_col, data_row));
                    });

                    // Tipo (TP_APLIC)
                    row.col(|ui| {
                        ui.label(egui::RichText::new(get_str(&tp_aplic_col, data_row)).size(10.5));
                    });

                    // N Fundos
                    row.col(|ui| {
                        let v = get_u64(&n_fundos_col, data_row);
                        ui.label(
                            egui::RichText::new(format!("{}", v))
                                .size(10.5)
                                .strong()
                                .color(egui::Color32::from_rgb(26, 115, 232)),
                        );
                    });

                    // Compradores
                    row.col(|ui| {
                        let v = get_u64(&n_comp_col, data_row);
                        if v > 0 {
                            ui.label(
                                egui::RichText::new(format!("{}", v))
                                    .size(11.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(34, 197, 94)), // emerald
                            );
                        } else {
                            ui.label(egui::RichText::new("-").size(11.0));
                        }
                    });

                    // Vendedores
                    row.col(|ui| {
                        let v = get_u64(&n_vend_col, data_row);
                        if v > 0 {
                            ui.label(
                                egui::RichText::new(format!("{}", v))
                                    .size(11.0)
                                    .strong()
                                    .color(egui::Color32::from_rgb(239, 68, 68)), // red-500
                            );
                        } else {
                            ui.label(egui::RichText::new("-").size(11.0));
                        }
                    });

                    // Valor Comprado
                    row.col(|ui| {
                        let v = get_f64(&comprado_col, data_row);
                        ui.label(
                            egui::RichText::new(fmt_currency(v))
                                .size(11.0)
                                .color(egui::Color32::from_rgb(34, 197, 94)),
                        );
                    });

                    // Valor Vendido
                    row.col(|ui| {
                        let v = get_f64(&vendido_col, data_row);
                        ui.label(
                            egui::RichText::new(fmt_currency(v))
                                .size(11.0)
                                .color(egui::Color32::from_rgb(239, 68, 68)),
                        );
                    });

                    // PL Médio
                    row.col(|ui| {
                        let v = get_f64(&pl_medio_col, data_row);
                        ui.label(egui::RichText::new(fmt_currency(v)).size(11.0).strong());
                    });

                    // VL Mín
                    row.col(|ui| {
                        let v = get_f64(&vl_min_col, data_row);
                        ui.label(
                            egui::RichText::new(if v == 0.0 {
                                "-".to_string()
                            } else {
                                fmt_currency(v)
                            })
                            .size(10.5),
                        );
                    });

                    // VL Máx
                    row.col(|ui| {
                        let v = get_f64(&vl_max_col, data_row);
                        ui.label(egui::RichText::new(fmt_currency(v)).size(10.5).strong());
                    });

                    // VL Médio
                    row.col(|ui| {
                        let v = get_f64(&vl_medio_col, data_row);
                        ui.label(egui::RichText::new(fmt_currency(v)).size(10.5));
                    });

                    // Vol Total (VL_MERC_TOTAL)
                    row.col(|ui| {
                        let v = get_f64(&merc_col, data_row);
                        ui.label(
                            egui::RichText::new(fmt_currency(v))
                                .size(11.5)
                                .strong()
                                .color(egui::Color32::from_rgb(139, 92, 246)),
                        );
                    });

                    if row.response().clicked() {
                        self.selected_row = if self.selected_row == Some(data_row) {
                            None
                        } else {
                            Some(data_row)
                        };
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

    fn closeable(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui) {
        Frame::NONE.inner_margin(8.0).show(ui, |ui| {
            ui.vertical(|ui| {
                // Toolbar
                self.render_toolbar(ui);
                ui.add_space(6.0);

                if self.loading {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.label(
                            egui::RichText::new(
                                egui_phosphor::regular::CHART_BAR.to_string(),
                            )
                            .size(40.0)
                            .color(egui::Color32::from_rgb(37, 99, 235)),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            egui::RichText::new("Analisando carteiras do mercado...")
                                .size(15.0)
                                .strong(),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(
                                "Agregando dados de todos os fundos no período selecionado.",
                            )
                            .size(11.0)
                            .weak(),
                        );
                        ui.add_space(10.0);
                        ui.spinner();
                    });
                    return;
                }

                if self.data.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space(60.0);
                        ui.label(
                            egui::RichText::new("Nenhum dado encontrado. Tente ajustar o período ou verifique sua conexão.")
                                .size(12.0)
                                .weak(),
                        );
                    });
                    return;
                }

                let filtered = self.filtered_rows();
                let total = filtered.len();

                // Header info
                ui.horizontal(|ui| {
                    ui.label(
                                            egui::RichText::new(format!(
                                                "{} {} ativos encontrados",
                                                egui_phosphor::regular::FUNNEL,
                                                total
                                            ))
                                            .size(12.0),
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
                                                        .size(11.0),
                        );
                    });
                });
                ui.add_space(4.0);

                // Top-4 cards
                if total > 0 {
                    self.render_top4_cards(ui, &filtered);
                    ui.add_space(8.0);
                }

                // Ranked table
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(egui_phosphor::regular::MAGNIFYING_GLASS.to_string())
                                .size(13.0)
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

                // Detail modal
                if let Some(row) = self.selected_row {
                    let isin_col = self.data.column("CD_ISIN").ok();
                    let ativo_col = self.data.column("TP_ATIVO").ok();
                    let titpub_col = self.data.column("TP_TITPUB").ok();
                    let aplic_col = self.data.column("TP_APLIC").ok();
                    let cd_ativo_col = self.data.column("CD_ATIVO").ok();
                    let ds_ativo_col = self.data.column("DS_ATIVO").ok();
                    let venc_col = self.data.column("DT_VENC").ok();
                    let nm_fundo_col = self.data.column("NM_FUNDO_COTA").ok();
                    let merc_col = self.data.column("VL_MERC_TOTAL").ok();
                    let n_fundos_col = self.data.column("N_FUNDOS").ok();
                    let n_comp_col = self.data.column("N_COMPRADORES").ok();
                    let n_vend_col = self.data.column("N_VENDEDORES").ok();
                    let comprado_col = self.data.column("VL_COMPRADO").ok();
                    let vendido_col = self.data.column("VL_VENDIDO").ok();
                    let pl_medio_col = self.data.column("PL_MEDIO").ok();
                    let vl_min_col = self.data.column("VL_MIN").ok();
                    let vl_max_col = self.data.column("VL_MAX").ok();
                    let vl_medio_col = self.data.column("VL_MEDIO").ok();
                    let vl_compra_min_col = self.data.column("VL_COMPRA_MIN").ok();
                    let vl_compra_max_col = self.data.column("VL_COMPRA_MAX").ok();
                    let vl_compra_medio_col = self.data.column("VL_COMPRA_MEDIO").ok();

                    let isin = get_str(&isin_col, row);
                    let ativo = get_str(&ativo_col, row);
                    let titpub = get_str(&titpub_col, row);
                    let aplic = get_str(&aplic_col, row);
                    let cd_ativo = get_str(&cd_ativo_col, row);
                    let ds_ativo = get_str(&ds_ativo_col, row);
                    let dt_venc = get_str(&venc_col, row);
                    let nm_fundo = get_str(&nm_fundo_col, row);

                    let codigo = if !cd_ativo.is_empty() { cd_ativo } else if !isin.is_empty() { isin } else { String::new() };
                    let nome_display = if !titpub.is_empty() { titpub } else if !ds_ativo.is_empty() { ds_ativo } else { ativo.clone() };


                    self.asset_modal.title = format!("Detalhes - {}", nome_display);
                    self.asset_modal.codigo = codigo;
                    self.asset_modal.nome = nome_display;
                    self.asset_modal.tp_ativo = ativo;
                    self.asset_modal.tp_aplic = aplic;
                    self.asset_modal.dt_venc = dt_venc;
                    self.asset_modal.nm_fundo = nm_fundo;
                    self.asset_modal.cnpj = String::new();
                    self.asset_modal.sender = Some(self.sender.clone());
                    self.asset_modal.n_fundos = get_u64(&n_fundos_col, row);
                    self.asset_modal.n_compradores = get_u64(&n_comp_col, row);
                    self.asset_modal.n_vendedores = get_u64(&n_vend_col, row);
                    self.asset_modal.vl_mercado = get_f64(&merc_col, row);
                    self.asset_modal.vl_aquisicao = 0.0;
                    self.asset_modal.pl_medio = get_f64(&pl_medio_col, row);
                    self.asset_modal.vl_min = get_f64(&vl_min_col, row);
                    self.asset_modal.vl_max = get_f64(&vl_max_col, row);
                    self.asset_modal.vl_medio = get_f64(&vl_medio_col, row);
                    self.asset_modal.vl_compra_min = get_f64(&vl_compra_min_col, row);
                    self.asset_modal.vl_compra_max = get_f64(&vl_compra_max_col, row);
                    self.asset_modal.vl_compra_medio = get_f64(&vl_compra_medio_col, row);
                    self.asset_modal.vl_comprado = get_f64(&comprado_col, row);
                    self.asset_modal.vl_vendido = get_f64(&vendido_col, row);
                    self.asset_modal.pct_pl = 0.0;
                    self.asset_modal.open_with_auto_fetch();

                    self.selected_row = None;
                }

                self.asset_modal.show(ui);
            });
        });
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn get_str(col: &Option<&polars::series::Series>, row: usize) -> String {
    col.as_ref()
        .and_then(|c| c.get(row).ok())
        .and_then(|v| v.get_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

fn get_f64(col: &Option<&polars::series::Series>, row: usize) -> f64 {
    col.as_ref()
        .and_then(|c| c.get(row).ok())
        .and_then(|v| v.try_extract::<f64>().ok())
        .unwrap_or(0.0)
}

fn get_u64(col: &Option<&polars::series::Series>, row: usize) -> u64 {
    col.as_ref()
        .and_then(|c| c.get(row).ok())
        .and_then(|v| {
            v.try_extract::<u64>()
                .ok()
                .or_else(|| v.try_extract::<u32>().ok().map(|x| x as u64))
        })
        .unwrap_or(0)
}

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
        ui.label(egui::RichText::new(label).size(10.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if color == egui::Color32::PLACEHOLDER {
                ui.label(egui::RichText::new(value).size(11.0).strong());
            } else {
                ui.label(egui::RichText::new(value).size(11.0).strong().color(color));
            }
        });
    });
}
