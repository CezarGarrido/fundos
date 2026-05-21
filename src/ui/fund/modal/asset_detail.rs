use std::collections::HashMap;

use egui::{Color32, Ui};
use egui_plot::{Line, Plot};
use polars::frame::DataFrame;
use tokio::sync::mpsc::UnboundedSender;

use crate::message::Message;

#[derive(Clone, PartialEq, Default)]
pub enum AssetModalContext {
    #[default]
    GlobalMarket,
    FundPortfolio,
}

/// Unified asset detail modal — rico em informações, usado em ativos.rs, historico.rs, etc.
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
    pub sender: Option<UnboundedSender<Message>>,
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
    // Yahoo state
    pub yahoo_prices: HashMap<String, DataFrame>,
    pub yahoo_loading: bool,
    // Holders state
    pub holders_data: Option<DataFrame>,
    pub holders_loading: bool,
    pub holders_filter: String,
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
            sender: None,
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
            yahoo_prices: HashMap::new(),
            yahoo_loading: false,
            holders_data: None,
            holders_loading: false,
            holders_filter: String::new(),
        }
    }
}

fn fmt(value: f64) -> String {
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
            .size(12.0)
            .strong()
            .color(heading_color),
    );
    ui.add_space(4.0);
}

fn kv(ui: &mut Ui, text_color: Color32, label: &str, value: String, value_color: Option<Color32>) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(9.0).color(text_color));
        if let Some(c) = value_color {
            ui.label(egui::RichText::new(value).size(12.0).strong().color(c));
        } else {
            ui.label(egui::RichText::new(value).size(12.0).strong());
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
            || upper == "FUNDO DE INVESTIMENTO IMOBILIÁRIO"
    }

    fn calc_estimated_qty(price: f64, total_value: f64) -> f64 {
        if price > 0.0 {
            total_value / price
        } else {
            0.0
        }
    }

    fn calc_avg_price(acquisition: f64, qty: f64) -> f64 {
        if qty > 0.0 {
            acquisition / qty
        } else {
            0.0
        }
    }

    /// Abre o modal e dispara automaticamente o fetch Yahoo Finance se o ativo for elegível.
    /// Deve ser chamado ao invés de setar `open = true` diretamente.
    pub fn open_with_auto_fetch(&mut self) {
        self.open = true;

        if let Some(sender) = &self.sender {
            let end = chrono::Local::now().naive_local().date();
            let start = end
                .checked_sub_months(chrono::Months::new(24))
                .unwrap_or(end - chrono::Duration::days(730));

            // Fetch holders for this asset
            if !self.codigo.is_empty() && !self.holders_loading {
                self.holders_loading = true;
                let _ = sender.send(crate::message::Message::FetchAssetHolders(
                    self.codigo.clone(),
                    start,
                    end,
                ));
            }

            // Fetch Yahoo prices if eligible
            if Self::is_yahoo_eligible(&self.tp_ativo)
                && !self.codigo.is_empty()
                && !self.yahoo_prices.contains_key(&self.codigo)
                && !self.yahoo_loading
            {
                self.yahoo_loading = true;
                let _ = sender.send(crate::message::Message::FetchYahooPrice(
                    self.codigo.clone(),
                    self.cnpj.clone(), // This is empty in AssetsMarketTab
                    start,
                    end,
                ));
            }
        }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        if !self.open {
            return;
        }

        let elegivel = Self::is_yahoo_eligible(&self.tp_ativo);
        let fluxo = self.vl_comprado - self.vl_vendido;

        let dark = ui.visuals().dark_mode;
        let hd = if dark {
            Color32::WHITE
        } else {
            Color32::from_rgb(20, 25, 35)
        };
        let tx = if dark {
            Color32::from_rgb(180, 190, 205)
        } else {
            Color32::from_rgb(70, 75, 90)
        };
        let gr = Color32::from_rgb(34, 197, 94);
        let rd = Color32::from_rgb(239, 68, 68);
        let vt = Color32::from_rgb(139, 92, 246);
        let az = Color32::from_rgb(37, 99, 235);

        // ── extract values ──────────────────────────────────────
        let modal_open = &mut self.open;
        let codigo = self.codigo.clone();
        let nome = self.nome.clone();
        let tp_aplic = self.tp_aplic.clone();
        let cnpj = self.cnpj.clone();
        let sender = self.sender.clone();
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
        let _pct = self.pct_pl;
        let flx = fluxo;

        let yahoo_loading = &mut self.yahoo_loading;
        let yahoo_prices = &mut self.yahoo_prices;
        let _has_prices = yahoo_prices.contains_key(&codigo);

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
                                        .size(16.0)
                                        .strong()
                                        .monospace()
                                        .color(hd),
                                );
                            }
                            ui.label(egui::RichText::new(&nome).size(16.0).strong().color(hd));
                        });
                        ui.horizontal(|ui| {
                            if !tp_aplic.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!("Tipo: {}", tp_aplic))
                                        .size(11.0)
                                        .color(tx),
                                );
                            }
                            if !dt_venc.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!("Venc: {}", dt_venc))
                                        .size(11.0)
                                        .color(tx),
                                );
                            }
                            if elegivel {
                                ui.label(egui::RichText::new("✅ Yahoo").size(10.0).color(gr));
                            }
                        });
                        if !nm_fundo.is_empty() {
                            ui.label(
                                egui::RichText::new(format!("Fundo: {}", nm_fundo))
                                    .size(11.0)
                                    .color(tx),
                            );
                        }
                        ui.separator();
                        ui.add_space(6.0);

                        // ── KPIs (Apenas para Global Market) ───────────────────────────────────
                        if self.context == AssetModalContext::GlobalMarket {
                            section(ui, hd, "Presença no Mercado");
                            ui.columns(3, |cols| {
                                cols[0].vertical(|ui| {
                                    ui.label(egui::RichText::new("Fundos").size(9.0).color(tx));
                                    ui.label(
                                        egui::RichText::new(format!("{}", n_fd))
                                            .size(18.0)
                                            .strong()
                                            .color(az),
                                    );
                                });
                                cols[1].vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new("Compradores").size(9.0).color(tx),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{}", n_cp))
                                            .size(18.0)
                                            .strong()
                                            .color(gr),
                                    );
                                });
                                cols[2].vertical(|ui| {
                                    ui.label(egui::RichText::new("Vendedores").size(9.0).color(tx));
                                    ui.label(
                                        egui::RichText::new(format!("{}", n_vd))
                                            .size(18.0)
                                            .strong()
                                            .color(rd),
                                    );
                                });
                            });
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);

                            // ── Position distribution ──────────────────
                            section(ui, hd, "Valor da Posição (por Fundo)");
                            ui.columns(3, |cols| {
                                cols[0].vertical(|ui| {
                                    kv(ui, tx, "Mínimo", fmt(vmin), None);
                                });
                                cols[1].vertical(|ui| {
                                    kv(ui, tx, "Médio", fmt(vmed), None);
                                });
                                cols[2].vertical(|ui| {
                                    kv(ui, tx, "Máximo", fmt(vmax), None);
                                });
                            });
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);

                            // ── Buy per transaction ────────────────────
                            section(ui, hd, "Valor de Compra (por transação)");
                            ui.columns(3, |cols| {
                                cols[0].vertical(|ui| {
                                    ui.label(egui::RichText::new("Mínimo").size(9.0).color(tx));
                                    ui.label(
                                        egui::RichText::new(if vcmin == 0.0 {
                                            "-".to_string()
                                        } else {
                                            fmt(vcmin)
                                        })
                                        .size(12.0)
                                        .strong(),
                                    );
                                });
                                cols[1].vertical(|ui| {
                                    ui.label(egui::RichText::new("Médio").size(9.0).color(tx));
                                    ui.label(
                                        egui::RichText::new(if vcmed == 0.0 {
                                            "-".to_string()
                                        } else {
                                            fmt(vcmed)
                                        })
                                        .size(12.0)
                                        .strong(),
                                    );
                                });
                                cols[2].vertical(|ui| {
                                    ui.label(egui::RichText::new("Máximo").size(9.0).color(tx));
                                    ui.label(
                                        egui::RichText::new(if vcmax == 0.0 {
                                            "-".to_string()
                                        } else {
                                            fmt(vcmax)
                                        })
                                        .size(12.0)
                                        .strong(),
                                    );
                                });
                            });
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);

                            // ── Volume / PL / Fluxo ────────────────────
                            section(ui, hd, "Volume e Exposição");
                            ui.columns(3, |cols| {
                                cols[0].vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new("Volume Total").size(9.0).color(tx),
                                    );
                                    ui.label(
                                        egui::RichText::new(fmt(vl_md))
                                            .size(13.0)
                                            .strong()
                                            .color(vt),
                                    );
                                });
                                cols[1].vertical(|ui| {
                                    ui.label(egui::RichText::new("PL Médio").size(9.0).color(tx));
                                    ui.label(egui::RichText::new(fmt(pl_md)).size(13.0).strong());
                                });
                                cols[2].vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new("Fluxo Líquido").size(9.0).color(tx),
                                    );
                                    let flx_color = if flx > 0.0 {
                                        gr
                                    } else if flx < 0.0 {
                                        rd
                                    } else {
                                        tx
                                    };
                                    let prefix = if flx > 0.0 { "+" } else { "" };
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{}{}",
                                            prefix,
                                            fmt(flx.abs())
                                        ))
                                        .size(13.0)
                                        .strong()
                                        .color(flx_color),
                                    );
                                });
                            });
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);

                            // ── Trading summary ────────────────────────
                            section(ui, hd, "Resumo de Negociação (período)");
                            ui.columns(2, |cols| {
                                cols[0].vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new("Total Comprado").size(9.0).color(tx),
                                    );
                                    ui.label(
                                        egui::RichText::new(fmt(vcp)).size(13.0).strong().color(gr),
                                    );
                                });
                                cols[1].vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new("Total Vendido").size(9.0).color(tx),
                                    );
                                    ui.label(
                                        egui::RichText::new(fmt(vvd)).size(13.0).strong().color(rd),
                                    );
                                });
                            });
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);
                        } else {
                            // Contexto de Portfólio (FundPortfolio)
                            section(ui, hd, "Posição do Fundo");
                            ui.columns(3, |cols| {
                                cols[0].vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new("Valor da Posição").size(9.0).color(tx),
                                    );
                                    ui.label(
                                        egui::RichText::new(fmt(vl_md))
                                            .size(13.0)
                                            .strong()
                                            .color(vt),
                                    );
                                });
                                cols[1].vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new("Custo de Aquisição")
                                            .size(9.0)
                                            .color(tx),
                                    );
                                    ui.label(
                                        egui::RichText::new(fmt(vl_aq))
                                            .size(13.0)
                                            .strong()
                                            .color(tx),
                                    );
                                });
                                cols[2].vertical(|ui| {
                                    ui.label(egui::RichText::new("% no PL").size(9.0).color(tx));
                                    ui.label(
                                        egui::RichText::new(format!("{:.2}%", self.pct_pl))
                                            .size(13.0)
                                            .strong(),
                                    );
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

                            if let Some(prices) = yahoo_prices.get(&codigo) {
                                // Já carregado — exibe o gráfico
                                Self::render_yahoo_detail(ui, prices, hd, tx, vl_md, vl_aq);
                            } else if *yahoo_loading {
                                // Carregando
                                ui.vertical_centered(|ui| {
                                    ui.add_space(8.0);
                                    ui.spinner();
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new("Buscando preços no Yahoo Finance...")
                                            .size(11.0)
                                            .weak(),
                                    );
                                    ui.add_space(8.0);
                                });
                            } else if let Some(sender) = &sender {
                                // Não carregado e não em loading — botão de retry
                                ui.vertical_centered(|ui| {
                                    ui.add_space(4.0);
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new(format!(
                                                    "{} Carregar histórico",
                                                    egui_phosphor::regular::ARROW_CLOCKWISE
                                                ))
                                                .size(11.5)
                                                .color(egui::Color32::WHITE),
                                            )
                                            .fill(egui::Color32::from_rgb(37, 99, 235))
                                            .min_size(egui::vec2(160.0, 28.0)),
                                        )
                                        .clicked()
                                    {
                                        *yahoo_loading = true;
                                        let end = chrono::Local::now().naive_local().date();
                                        let start = end
                                            .checked_sub_months(chrono::Months::new(24))
                                            .unwrap_or(end - chrono::Duration::days(730));
                                        let _ = sender.send(Message::FetchYahooPrice(
                                            codigo.clone(),
                                            cnpj.clone(),
                                            start,
                                            end,
                                        ));
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
                                ui.label(
                                    egui::RichText::new("Filtrar Fundo:").size(11.0).color(tx),
                                );
                                ui.add(
                                    egui::TextEdit::singleline(&mut self.holders_filter)
                                        .margin(egui::vec2(6.0, 4.0))
                                        .desired_width(200.0)
                                        .hint_text("Nome ou CNPJ..."),
                                );
                            });
                            ui.add_space(6.0);

                            if self.holders_loading {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(8.0);
                                    ui.spinner();
                                    ui.add_space(4.0);
                                    ui.label(
                                        egui::RichText::new("Carregando fundos...")
                                            .size(11.0)
                                            .weak(),
                                    );
                                    ui.add_space(8.0);
                                });
                            } else if let Some(holders) = &self.holders_data {
                                let mut has_nm = false;
                                let mut nm_col = polars::series::Series::default();
                                if let Ok(c) = holders.column("NM_FUNDO") {
                                    has_nm = true;
                                    nm_col = c.clone();
                                }
                                let cnpj_col = holders
                                    .column("CNPJ_FUNDO")
                                    .unwrap_or(&polars::series::Series::default())
                                    .clone();
                                let vl_col = holders
                                    .column("VL_MERC_POS_FINAL")
                                    .unwrap_or(&polars::series::Series::default())
                                    .clone();
                                let pct_col = holders
                                    .column("VL_PORCENTAGEM_PL")
                                    .unwrap_or(&polars::series::Series::default())
                                    .clone();

                                let mut rows = Vec::new();
                                for i in 0..holders.height() {
                                    let c = cnpj_col
                                        .get(i)
                                        .unwrap_or(polars::datatypes::AnyValue::Null)
                                        .to_string();
                                    let n = if has_nm {
                                        nm_col
                                            .get(i)
                                            .unwrap_or(polars::datatypes::AnyValue::Null)
                                            .to_string()
                                    } else {
                                        String::new()
                                    };
                                    let filter = self.holders_filter.to_lowercase();
                                    if filter.is_empty()
                                        || c.to_lowercase().contains(&filter)
                                        || n.to_lowercase().contains(&filter)
                                    {
                                        rows.push(i);
                                    }
                                }

                                egui::ScrollArea::horizontal()
                                    .id_salt("holders_h_scroll")
                                    .show(ui, |ui| {
                                        egui_extras::TableBuilder::new(ui)
                                            .striped(true)
                                            .resizable(true)
                                            .cell_layout(egui::Layout::left_to_right(
                                                egui::Align::Center,
                                            ))
                                            .column(
                                                egui_extras::Column::initial(220.0).at_least(100.0),
                                            )
                                            .column(
                                                egui_extras::Column::initial(110.0).at_least(80.0),
                                            )
                                            .column(
                                                egui_extras::Column::initial(110.0).at_least(80.0),
                                            )
                                            .header(24.0, |mut header| {
                                                header.col(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(if has_nm {
                                                            "Fundo"
                                                        } else {
                                                            "CNPJ"
                                                        })
                                                        .size(11.0)
                                                        .strong(),
                                                    );
                                                });
                                                header.col(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("Volume (R$)")
                                                            .size(11.0)
                                                            .strong(),
                                                    );
                                                });
                                                header.col(|ui| {
                                                    ui.label(
                                                        egui::RichText::new("% PL")
                                                            .size(11.0)
                                                            .strong(),
                                                    );
                                                });
                                            })
                                            .body(|body| {
                                                body.rows(20.0, rows.len().min(100), |mut row| {
                                                    let idx = rows[row.index()];
                                                    let c = cnpj_col
                                                        .get(idx)
                                                        .unwrap_or(
                                                            polars::datatypes::AnyValue::Null,
                                                        )
                                                        .to_string()
                                                        .replace("\"", "");
                                                    let n = if has_nm {
                                                        nm_col
                                                            .get(idx)
                                                            .unwrap_or(
                                                                polars::datatypes::AnyValue::Null,
                                                            )
                                                            .to_string()
                                                            .replace("\"", "")
                                                    } else {
                                                        String::new()
                                                    };
                                                    let v = get_f64(&vl_col, idx);
                                                    let p = get_f64(&pct_col, idx);

                                                    row.col(|ui| {
                                                        ui.label(
                                                            egui::RichText::new(if n.is_empty() {
                                                                c
                                                            } else {
                                                                n
                                                            })
                                                            .size(10.5),
                                                        );
                                                    });
                                                    row.col(|ui| {
                                                        ui.label(
                                                            egui::RichText::new(fmt(v)).size(10.5),
                                                        );
                                                    });
                                                    row.col(|ui| {
                                                        ui.label(
                                                            egui::RichText::new(format!(
                                                                "{:.2}%",
                                                                p
                                                            ))
                                                            .size(10.5),
                                                        );
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
        prices: &DataFrame,
        hd: Color32,
        tx: Color32,
        vl_mercado: f64,
        vl_aquisicao: f64,
    ) {
        let close_col = match prices.column("adjclose") {
            Ok(c) => c,
            Err(_) => return,
        };
        let n = prices.height().min(60);
        if n < 2 {
            return;
        }

        let mut pts: Vec<[f64; 2]> = vec![];
        for i in 0..n {
            let p = get_f64(close_col, i);
            pts.push([i as f64, p]);
        }
        let first_p = pts.first().map(|p| p[1]).unwrap_or(0.0);
        let last_p = pts.last().map(|p| p[1]).unwrap_or(0.0);
        let change = if first_p > 0.0 {
            ((last_p - first_p) / first_p) * 100.0
        } else {
            0.0
        };
        let change_color = if change > 0.0 {
            Color32::from_rgb(34, 197, 94)
        } else if change < 0.0 {
            Color32::from_rgb(239, 68, 68)
        } else {
            Color32::GRAY
        };

        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("Preço: R$ {:.2}", last_p))
                    .size(14.0)
                    .strong(),
            );
            ui.label(
                egui::RichText::new(format!("({:+.2}%)", change))
                    .size(12.0)
                    .color(change_color),
            );
        });

        Plot::new("plot::yahoo_asset")
            .show_background(false)
            .height(100.0)
            .show(ui, |plot_ui| {
                plot_ui.line(
                    Line::new("Preço", pts)
                        .color(Color32::from_rgb(37, 99, 235))
                        .width(1.5),
                );
            });

        let qty = Self::calc_estimated_qty(last_p, vl_mercado);
        let avg_price = Self::calc_avg_price(vl_aquisicao, qty);

        ui.separator();
        ui.add_space(4.0);
        section(ui, hd, "Estimativas");
        ui.columns(3, |cols| {
            cols[0].vertical(|ui| {
                ui.label(egui::RichText::new("Qtde. Estimada").size(9.0).color(tx));
                ui.label(
                    egui::RichText::new(format!("{:.0}", qty))
                        .size(13.0)
                        .strong(),
                );
            });
            cols[1].vertical(|ui| {
                ui.label(egui::RichText::new("Valor Posição").size(9.0).color(tx));
                ui.label(egui::RichText::new(fmt(vl_mercado)).size(13.0).strong());
            });
            cols[2].vertical(|ui| {
                ui.label(
                    egui::RichText::new("Preço Médio Compra")
                        .size(9.0)
                        .color(tx),
                );
                ui.label(
                    egui::RichText::new(if avg_price > 0.0 {
                        format!("R$ {:.2}", avg_price)
                    } else {
                        "N/A".to_string()
                    })
                    .size(13.0)
                    .strong(),
                );
            });
        });
    }
}

fn get_f64(col: &polars::series::Series, row: usize) -> f64 {
    col.get(row)
        .ok()
        .and_then(|v| {
            v.try_extract::<f64>().ok().or_else(|| {
                v.get_str()
                    .and_then(|s| s.replace(',', ".").parse::<f64>().ok())
            })
        })
        .unwrap_or(0.0)
}
