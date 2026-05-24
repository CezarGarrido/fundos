use crate::ui::design::Scale;
use egui::{Color32, Ui};
use egui_plot::{Line, Plot};
use fundos_common::types::{AssetHolder, YahooPricePoint};

#[derive(Clone, PartialEq, Default)]
pub enum AssetModalContext {
    #[default]
    GlobalMarket,
    FundPortfolio,
}

/// Unified asset detail modal — rich info display for assets from market or portfolio views.
#[derive(Clone)]
pub struct AssetDetailModal {
    pub open: bool,
    pub title: String,
    pub codigo: String,
    pub nome: String,
    pub tp_ativo: String,
    pub tp_aplic: String,
    pub dt_venc: String,
    pub nm_fundo: String,
    pub cnpj: String,
    pub context: AssetModalContext,
    // KPIs
    pub n_fundos: u64,
    pub n_compradores: u64,
    pub n_vendedores: u64,
    // Position values
    pub vl_mercado: f64,
    pub vl_aquisicao: f64,
    pub pl_medio: f64,
    pub vl_min: f64,
    pub vl_max: f64,
    pub vl_medio: f64,
    // Buy values per transaction
    pub vl_compra_min: f64,
    pub vl_compra_max: f64,
    pub vl_compra_medio: f64,
    // Period totals
    pub vl_comprado: f64,
    pub vl_vendido: f64,
    pub pct_pl: f64,
    // Yahoo state (populated by parent after API fetch)
    pub yahoo_prices: Option<Vec<YahooPricePoint>>,
    pub yahoo_loading: bool,
    // Holders state (populated by parent after API fetch)
    pub holders_data: Option<Vec<AssetHolder>>,
    pub holders_loading: bool,
    pub holders_filter: String,
    /// Set by the modal when user opens it; parent reads, fetches, and populates data.
    pub request_yahoo: bool,
    pub request_holders: bool,
}

impl Default for AssetDetailModal {
    fn default() -> Self {
        Self {
            open: false,
            title: String::new(),
            codigo: String::new(),
            nome: String::new(),
            tp_ativo: String::new(),
            tp_aplic: String::new(),
            dt_venc: String::new(),
            nm_fundo: String::new(),
            cnpj: String::new(),
            context: AssetModalContext::default(),
            n_fundos: 0,
            n_compradores: 0,
            n_vendedores: 0,
            vl_mercado: 0.0,
            vl_aquisicao: 0.0,
            pl_medio: 0.0,
            vl_min: 0.0,
            vl_max: 0.0,
            vl_medio: 0.0,
            vl_compra_min: 0.0,
            vl_compra_max: 0.0,
            vl_compra_medio: 0.0,
            vl_comprado: 0.0,
            vl_vendido: 0.0,
            pct_pl: 0.0,
            yahoo_prices: None,
            yahoo_loading: false,
            holders_data: None,
            holders_loading: false,
            holders_filter: String::new(),
            request_yahoo: false,
            request_holders: false,
        }
    }
}

fn fmt_val(value: f64) -> String {
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

fn section(ui: &mut Ui, heading_color: Color32, title: &str) {
    ui.label(
        egui::RichText::new(title)
            .size(Scale::DEFAULT.small_text())
            .strong()
            .color(heading_color),
    );
    ui.add_space(4.0);
}

fn kv(ui: &mut Ui, text_color: Color32, label: &str, value: String, value_color: Option<Color32>) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(Scale::DEFAULT.label_mini()).color(text_color));
        if let Some(c) = value_color {
            ui.label(egui::RichText::new(value).size(Scale::DEFAULT.small_text()).strong().color(c));
        } else {
            ui.label(egui::RichText::new(value).size(Scale::DEFAULT.small_text()).strong());
        }
    });
}

impl AssetDetailModal {
    fn is_yahoo_eligible(tp_ativo: &str) -> bool {
        let upper = tp_ativo.to_uppercase();
        upper.contains("AÇÃO")
            || upper.contains("STOCK")
            || upper.contains("ETF")
            || upper == "BDR"
            || upper == "FII"
            || upper == "FIIs"
            || upper.contains("FUNDO DE INVESTIMENTO IMOBILIÁRIO")
    }

    fn calc_estimated_qty(price: f64, total_value: f64) -> f64 {
        if price > 0.0 { total_value / price } else { 0.0 }
    }

    fn calc_avg_price(acquisition: f64, qty: f64) -> f64 {
        if qty > 0.0 { acquisition / qty } else { 0.0 }
    }

    /// Opens the modal and signals parent to fetch Yahoo + holders data.
    pub fn open_with_auto_fetch(&mut self) {
        self.open = true;
        if Self::is_yahoo_eligible(&self.tp_ativo)
            && !self.codigo.is_empty()
            && self.yahoo_prices.is_none()
            && !self.yahoo_loading
        {
            self.yahoo_loading = true;
            self.request_yahoo = true;
        }
        if !self.codigo.is_empty() && self.holders_data.is_none() && !self.holders_loading {
            self.holders_loading = true;
            self.request_holders = true;
        }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        if !self.open {
            return;
        }

        let elegivel = Self::is_yahoo_eligible(&self.tp_ativo);
        let fluxo = self.vl_comprado - self.vl_vendido;

        let dark = ui.visuals().dark_mode;
        let hd = if dark { Color32::WHITE } else { Color32::from_rgb(20, 25, 35) };
        let tx = if dark { Color32::from_rgb(180, 190, 205) } else { Color32::from_rgb(70, 75, 90) };
        let gr = Color32::from_rgb(34, 197, 94);
        let rd = Color32::from_rgb(239, 68, 68);
        let vt = Color32::from_rgb(139, 92, 246);
        let az = Color32::from_rgb(37, 99, 235);

        let codigo = self.codigo.clone();
        let nome = self.nome.clone();
        let tp_aplic = self.tp_aplic.clone();
        let dt_venc = self.dt_venc.clone();
        let nm_fundo = self.nm_fundo.clone();
        let n_fd = self.n_fundos;
        let n_cp = self.n_compradores;
        let n_vd = self.n_vendedores;
        let vl_md = self.vl_mercado;
        let vl_aq = self.vl_aquisicao;
        let pl_md = self.pl_medio;
        let vmin = self.vl_min;
        let vmax = self.vl_max;
        let vmed = self.vl_medio;
        let vcmin = self.vl_compra_min;
        let vcmax = self.vl_compra_max;
        let vcmed = self.vl_compra_medio;
        let vcp = self.vl_comprado;
        let vvd = self.vl_vendido;
        let flx = fluxo;

        let modal_open = &mut self.open;

        egui::Window::new(&self.title)
            .id(ui.id().with("asset_detail_modal"))
            .resizable(true)
            .default_width(520.0)
            .default_height(500.0)
            .open(modal_open)
            .show(ui.ctx(), |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical(|ui| {
                        // ── Header ──────────────────────────────────
                        ui.horizontal(|ui| {
                            if !codigo.is_empty() {
                                ui.label(
                                    egui::RichText::new(&codigo)
                                        .size(Scale::DEFAULT.heading_3())
                                        .strong()
                                        .monospace()
                                        .color(hd),
                                );
                            }
                            ui.label(egui::RichText::new(&nome).size(Scale::DEFAULT.heading_3()).strong().color(hd));
                        });
                        ui.horizontal(|ui| {
                            if !tp_aplic.is_empty() {
                                ui.label(egui::RichText::new(format!("Tipo: {}", tp_aplic)).size(Scale::DEFAULT.label()).color(tx));
                            }
                            if !dt_venc.is_empty() {
                                ui.label(egui::RichText::new(format!("Venc: {}", dt_venc)).size(Scale::DEFAULT.label()).color(tx));
                            }
                            if elegivel {
                                ui.label(egui::RichText::new("Yahoo").size(Scale::DEFAULT.badge()).color(gr));
                            }
                        });
                        if !nm_fundo.is_empty() {
                            ui.label(egui::RichText::new(format!("Fundo: {}", nm_fundo)).size(Scale::DEFAULT.label()).color(tx));
                        }
                        ui.separator();
                        ui.add_space(6.0);

                        // ── KPIs (Apenas para Global Market) ────────
                        if self.context == AssetModalContext::GlobalMarket {
                            section(ui, hd, "Presença no Mercado");
                            ui.columns(3, |cols| {
                                cols[0].vertical(|ui| {
                                    ui.label(egui::RichText::new("Fundos").size(Scale::DEFAULT.label_mini()).color(tx));
                                    ui.label(egui::RichText::new(format!("{}", n_fd)).size(Scale::DEFAULT.metric()).strong().color(az));
                                });
                                cols[1].vertical(|ui| {
                                    ui.label(egui::RichText::new("Compradores").size(Scale::DEFAULT.label_mini()).color(tx));
                                    ui.label(egui::RichText::new(format!("{}", n_cp)).size(Scale::DEFAULT.metric()).strong().color(gr));
                                });
                                cols[2].vertical(|ui| {
                                    ui.label(egui::RichText::new("Vendedores").size(Scale::DEFAULT.label_mini()).color(tx));
                                    ui.label(egui::RichText::new(format!("{}", n_vd)).size(Scale::DEFAULT.metric()).strong().color(rd));
                                });
                            });
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);

                            section(ui, hd, "Valor da Posição (por Fundo)");
                            ui.columns(3, |cols| {
                                cols[0].vertical(|ui| { kv(ui, tx, "Mínimo", fmt_val(vmin), None); });
                                cols[1].vertical(|ui| { kv(ui, tx, "Médio", fmt_val(vmed), None); });
                                cols[2].vertical(|ui| { kv(ui, tx, "Máximo", fmt_val(vmax), None); });
                            });
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);

                            section(ui, hd, "Valor de Compra (por transação)");
                            ui.columns(3, |cols| {
                                cols[0].vertical(|ui| {
                                    ui.label(egui::RichText::new("Mínimo").size(Scale::DEFAULT.label_mini()).color(tx));
                                    ui.label(egui::RichText::new(if vcmin == 0.0 { "-".to_string() } else { fmt_val(vcmin) }).size(Scale::DEFAULT.small_text()).strong());
                                });
                                cols[1].vertical(|ui| {
                                    ui.label(egui::RichText::new("Médio").size(Scale::DEFAULT.label_mini()).color(tx));
                                    ui.label(egui::RichText::new(if vcmed == 0.0 { "-".to_string() } else { fmt_val(vcmed) }).size(Scale::DEFAULT.small_text()).strong());
                                });
                                cols[2].vertical(|ui| {
                                    ui.label(egui::RichText::new("Máximo").size(Scale::DEFAULT.label_mini()).color(tx));
                                    ui.label(egui::RichText::new(if vcmax == 0.0 { "-".to_string() } else { fmt_val(vcmax) }).size(Scale::DEFAULT.small_text()).strong());
                                });
                            });
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);

                            section(ui, hd, "Volume e Exposição");
                            ui.columns(3, |cols| {
                                cols[0].vertical(|ui| {
                                    ui.label(egui::RichText::new("Volume Total").size(Scale::DEFAULT.label_mini()).color(tx));
                                    ui.label(egui::RichText::new(fmt_val(vl_md)).size(Scale::DEFAULT.button()).strong().color(vt));
                                });
                                cols[1].vertical(|ui| {
                                    ui.label(egui::RichText::new("PL Médio").size(Scale::DEFAULT.label_mini()).color(tx));
                                    ui.label(egui::RichText::new(fmt_val(pl_md)).size(Scale::DEFAULT.button()).strong());
                                });
                                cols[2].vertical(|ui| {
                                    ui.label(egui::RichText::new("Fluxo Líquido").size(Scale::DEFAULT.label_mini()).color(tx));
                                    let flx_color = if flx > 0.0 { gr } else if flx < 0.0 { rd } else { tx };
                                    let prefix = if flx > 0.0 { "+" } else { "" };
                                    ui.label(egui::RichText::new(format!("{}{}", prefix, fmt_val(flx.abs()))).size(Scale::DEFAULT.button()).strong().color(flx_color));
                                });
                            });
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);

                            section(ui, hd, "Resumo de Negociação (período)");
                            ui.columns(2, |cols| {
                                cols[0].vertical(|ui| {
                                    ui.label(egui::RichText::new("Total Comprado").size(Scale::DEFAULT.label_mini()).color(tx));
                                    ui.label(egui::RichText::new(fmt_val(vcp)).size(Scale::DEFAULT.button()).strong().color(gr));
                                });
                                cols[1].vertical(|ui| {
                                    ui.label(egui::RichText::new("Total Vendido").size(Scale::DEFAULT.label_mini()).color(tx));
                                    ui.label(egui::RichText::new(fmt_val(vvd)).size(Scale::DEFAULT.button()).strong().color(rd));
                                });
                            });
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);
                        } else {
                            // FundPortfolio context
                            section(ui, hd, "Posição do Fundo");
                            ui.columns(3, |cols| {
                                cols[0].vertical(|ui| {
                                    ui.label(egui::RichText::new("Valor da Posição").size(Scale::DEFAULT.label_mini()).color(tx));
                                    ui.label(egui::RichText::new(fmt_val(vl_md)).size(Scale::DEFAULT.button()).strong().color(vt));
                                });
                                cols[1].vertical(|ui| {
                                    ui.label(egui::RichText::new("Custo de Aquisição").size(Scale::DEFAULT.label_mini()).color(tx));
                                    ui.label(egui::RichText::new(fmt_val(vl_aq)).size(Scale::DEFAULT.button()).strong().color(tx));
                                });
                                cols[2].vertical(|ui| {
                                    ui.label(egui::RichText::new("% no PL").size(Scale::DEFAULT.label_mini()).color(tx));
                                    ui.label(egui::RichText::new(format!("{:.2}%", self.pct_pl)).size(Scale::DEFAULT.button()).strong());
                                });
                            });
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);
                        }

                        // ── Yahoo ──────────────────────────────────
                        if elegivel && !codigo.is_empty() {
                            ui.separator();
                            ui.add_space(6.0);
                            section(ui, hd, "Histórico de Preço — Yahoo Finance");

                            if let Some(ref prices) = self.yahoo_prices {
                                Self::render_yahoo_detail(ui, prices, hd, tx, vl_md, vl_aq);
                            } else if self.yahoo_loading {
                                crate::ui::loading::show_custom_small(ui, "Buscando Yahoo Finance...");
                            } else {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(4.0);
                                    if ui.add(
                                        egui::Button::new(
                                            egui::RichText::new(format!("{} Carregar histórico", egui_phosphor::regular::ARROW_CLOCKWISE))
                                                .size(Scale::DEFAULT.caption())
                                                .color(egui::Color32::WHITE),
                                        )
                                        .fill(egui::Color32::from_rgb(37, 99, 235))
                                        .min_size(egui::vec2(160.0, 28.0)),
                                    ).clicked()
                                    {
                                        self.yahoo_loading = true;
                                        self.request_yahoo = true;
                                    }
                                    ui.add_space(4.0);
                                });
                            }
                        }

                        // ── Holders ────────────────────────────────
                        if n_fd > 0 {
                            ui.separator();
                            ui.add_space(6.0);
                            section(ui, hd, "Fundos Investidores");

                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Filtrar Fundo:").size(Scale::DEFAULT.label()).color(tx));
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.holders_filter)
                                        .margin(egui::vec2(6.0, 4.0))
                                        .desired_width(Scale::COLUMN_MIN)
                                        .hint_text("Nome ou CNPJ..."),
                                );
                            });
                            ui.add_space(6.0);

                            if self.holders_loading {
                                crate::ui::loading::show_custom_small(ui, "Carregando fundos...");
                            } else if let Some(ref holders) = self.holders_data {
                                let filter = self.holders_filter.to_lowercase();
                                let filtered: Vec<&AssetHolder> = if filter.is_empty() {
                                    holders.iter().collect()
                                } else {
                                    holders.iter().filter(|h| {
                                        h.fund_cnpj.to_lowercase().contains(&filter)
                                            || h.fund_name.to_lowercase().contains(&filter)
                                    }).collect()
                                };

                                egui::ScrollArea::horizontal()
                                    .id_salt("holders_h_scroll")
                                    .show(ui, |ui| {
                                        egui_extras::TableBuilder::new(ui)
                                            .striped(true)
                                            .resizable(true)
                                            .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                                            .column(egui_extras::Column::initial(220.0).at_least(100.0))
                                            .column(egui_extras::Column::initial(110.0).at_least(80.0))
                                            .column(egui_extras::Column::initial(110.0).at_least(80.0))
                                            .header(Scale::DEFAULT.table_header_height(), |mut header| {
                                                header.col(|ui| {
                                                    ui.label(egui::RichText::new("Fundo").size(Scale::DEFAULT.label()).strong());
                                                });
                                                header.col(|ui| {
                                                    ui.label(egui::RichText::new("Volume (R$)").size(Scale::DEFAULT.label()).strong());
                                                });
                                                header.col(|ui| {
                                                    ui.label(egui::RichText::new("% PL").size(Scale::DEFAULT.label()).strong());
                                                });
                                            })
                                            .body(|body| {
                                                body.rows(Scale::DEFAULT.table_row_height(), filtered.len().min(100), |mut row| {
                                                    let h = filtered[row.index()];
                                                    row.col(|ui| {
                                                        let display = if h.fund_name.is_empty() { &h.fund_cnpj } else { &h.fund_name };
                                                        ui.label(egui::RichText::new(display.as_str()).size(Scale::DEFAULT.table_cell()));
                                                    });
                                                    row.col(|ui| {
                                                        ui.label(egui::RichText::new(fmt_val(h.market_value)).size(Scale::DEFAULT.table_cell()));
                                                    });
                                                    row.col(|ui| {
                                                        ui.label(egui::RichText::new(format!("{:.2}%", h.percentage_pl)).size(Scale::DEFAULT.table_cell()));
                                                    });
                                                });
                                            });
                                    });
                            }
                        }
                    });
                });
            });
    }

    fn render_yahoo_detail(
        ui: &mut Ui,
        prices: &[YahooPricePoint],
        hd: Color32,
        _tx: Color32,
        vl_mercado: f64,
        vl_aquisicao: f64,
    ) {
        let n = prices.len().min(60);
        if n < 2 {
            return;
        }

        let pts: Vec<[f64; 2]> = prices.iter().take(n).enumerate().map(|(i, p)| [i as f64, p.adjclose]).collect();
        let first_p = pts.first().map(|p| p[1]).unwrap_or(0.0);
        let last_p = pts.last().map(|p| p[1]).unwrap_or(0.0);
        let change = if first_p > 0.0 { ((last_p - first_p) / first_p) * 100.0 } else { 0.0 };
        let change_color = if change > 0.0 {
            Color32::from_rgb(34, 197, 94)
        } else if change < 0.0 {
            Color32::from_rgb(239, 68, 68)
        } else {
            Color32::GRAY
        };

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(format!("Preço: R$ {:.2}", last_p)).size(Scale::DEFAULT.body()).strong());
            ui.label(egui::RichText::new(format!("({:+.2}%)", change)).size(Scale::DEFAULT.small_text()).color(change_color));
        });

        Plot::new("plot::yahoo_asset")
            .show_background(false)
            .height(100.0)
            .show(ui, |plot_ui| {
                plot_ui.line(Line::new("Preço", pts).color(Color32::from_rgb(37, 99, 235)).width(1.5));
            });

        let qty = Self::calc_estimated_qty(last_p, vl_mercado);
        let avg_price = Self::calc_avg_price(vl_aquisicao, qty);

        ui.separator();
        ui.add_space(4.0);
        section(ui, hd, "Estimativas");
        ui.columns(3, |cols| {
            cols[0].vertical(|ui| {
                ui.label(egui::RichText::new("Qtde. Estimada").size(Scale::DEFAULT.label_mini()).color(Color32::GRAY));
                ui.label(egui::RichText::new(format!("{:.0}", qty)).size(Scale::DEFAULT.button()).strong());
            });
            cols[1].vertical(|ui| {
                ui.label(egui::RichText::new("Valor Posição").size(Scale::DEFAULT.label_mini()).color(Color32::GRAY));
                ui.label(egui::RichText::new(fmt_val(vl_mercado)).size(Scale::DEFAULT.button()).strong());
            });
            cols[2].vertical(|ui| {
                ui.label(egui::RichText::new("Preço Médio Compra").size(Scale::DEFAULT.label_mini()).color(Color32::GRAY));
                ui.label(
                    egui::RichText::new(if avg_price > 0.0 { format!("R$ {:.2}", avg_price) } else { "N/A".to_string() })
                        .size(Scale::DEFAULT.button())
                        .strong(),
                );
            });
        });
    }
}
