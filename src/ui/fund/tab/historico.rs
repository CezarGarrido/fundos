use std::collections::HashMap;

use egui::{Align, Color32, Frame, Layout, RichText, ScrollArea, Sense, Ui, WidgetText};
use egui_extras::{Column, TableBuilder};
use egui_plot::{AxisHints, GridMark, Legend, Line, Plot};
use polars::prelude::*;
use tokio::sync::mpsc::UnboundedSender;

use crate::{message::Message, ui::tabs::Tab};

fn get_str(col: &polars::series::Series, row: usize) -> String {
    col.get(row)
        .ok()
        .and_then(|v| v.get_str().map(|s| s.to_string()))
        .unwrap_or_default()
}

fn fmt_num(n: f64) -> String {
    let n = n.round() as i64;
    if n == 0 {
        return "0".into();
    }
    let neg = n < 0;
    let s = n.unsigned_abs().to_string();
    let mut result = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push('.');
        }
        result.push(c);
    }
    if neg {
        result.push('-');
    }
    result.chars().rev().collect()
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

#[allow(dead_code)]
pub struct HistoricoTab {
    pub title: String,
    pub cnpj: String,
    pub sender: UnboundedSender<Message>,
    pub data: DataFrame,
    pub loading: bool,
    pub monthly_series: Vec<MonthlySeries>,
    pub selected_asset: Option<String>,
    pub yahoo_prices: HashMap<String, DataFrame>,
    pub yahoo_loading: bool,
    pub last_yahoo_fetch: Option<String>,
    pub analytics_cache: HashMap<String, crate::analytics::AssetAnalytics>, // tracks last fetched asset to avoid re-fetch
}

#[derive(Clone)]
pub struct MonthlySeries {
    pub key: String,
    pub label: String,
    pub color: Color32,
    pub points: Vec<(f64, f64)>,
}

impl HistoricoTab {
    pub fn new(cnpj: String, sender: UnboundedSender<Message>) -> Self {
        let tab = Self {
            title: format!("{} Histórico", cnpj),
            cnpj,
            sender,
            data: DataFrame::empty(),
            loading: true,
            monthly_series: vec![],
            selected_asset: None,
            yahoo_prices: HashMap::new(),
            yahoo_loading: false,
            last_yahoo_fetch: None,
            analytics_cache: HashMap::new(),
        };
        tab.send_load_request();
        tab
    }

    fn send_load_request(&self) {
        let end = chrono::Local::now().naive_local().date();
        let start = end
            .checked_sub_months(chrono::Months::new(12))
            .unwrap_or(end - chrono::Duration::days(365));
        let _ = self
            .sender
            .send(Message::OpenHistoricoTab(self.cnpj.clone(), start, end));
    }

    pub fn set_data(&mut self, df: DataFrame) {
        self.data = df;
        self.analytics_cache.clear();
        // Não processa séries aqui — envia para background thread
        let data = self.data.clone();
        let sender = self.sender.clone();
        let cnpj = self.cnpj.clone();
        tokio::spawn(async move {
            let raw_series = crate::analytics::compute_top_series(&data);
            let palette: [Color32; 12] = [
                Color32::from_rgb(37, 99, 235),
                Color32::from_rgb(34, 197, 94),
                Color32::from_rgb(239, 68, 68),
                Color32::from_rgb(234, 179, 8),
                Color32::from_rgb(139, 92, 246),
                Color32::from_rgb(249, 115, 22),
                Color32::from_rgb(20, 184, 166),
                Color32::from_rgb(236, 72, 153),
                Color32::from_rgb(59, 130, 246),
                Color32::from_rgb(168, 85, 247),
                Color32::from_rgb(251, 146, 60),
                Color32::from_rgb(45, 212, 191),
            ];
            let series: Vec<MonthlySeries> = raw_series
                .into_iter()
                .enumerate()
                .map(|(i, (key, label, points))| MonthlySeries {
                    key,
                    label,
                    color: palette[i % palette.len()],
                    points,
                })
                .collect();
            let _ = sender.send(Message::HistoricoSeriesResult(cnpj, series));
        });
    }

    fn extract_months(&self) -> Vec<String> {
        let dt_col = match self.data.column("DT_COMPTC") {
            Ok(c) => c,
            Err(_) => return vec![],
        };
        let mut months: Vec<String> = vec![];
        for i in 0..self.data.height() {
            let d = get_str(dt_col, i);
            let m = if d.len() >= 7 { d[..7].to_string() } else { d };
            if !months.contains(&m) {
                months.push(m);
            }
        }
        months.sort();
        months
    }

    pub fn set_yahoo_prices(&mut self, codigo: String, prices: DataFrame) {
        self.analytics_cache.remove(&codigo); // invalida cache desse ativo
        self.yahoo_prices.insert(codigo, prices);
        self.yahoo_loading = false;
    }

    fn render_asset_line_chart(&self, ui: &mut Ui, months: &[String], selected_name: &str) {
        let series = match self.monthly_series.iter().find(|s| s.key == selected_name) {
            Some(s) => s,
            None => return,
        };
        if series.points.is_empty() {
            ui.weak("Sem dados para exibir.");
            return;
        }
        let pts: Vec<[f64; 2]> = series.points.iter().map(|(x, y)| [*x, *y]).collect();
        let mc = months.to_vec();
        let x_fmt = move |mark: GridMark, _: &std::ops::RangeInclusive<f64>| {
            let idx = mark.value as i32;
            if idx >= 0 && (idx as usize) < mc.len() {
                let m = &mc[idx as usize];
                let parts: Vec<&str> = m.split('-').collect();
                if parts.len() >= 2 {
                    return format!("{}/{}", parts[1], &parts[0][2..]);
                }
                m.clone()
            } else {
                String::new()
            }
        };
        let y_fmt =
            |mark: GridMark, _: &std::ops::RangeInclusive<f64>| format!("{:.1}%", mark.value);
        Plot::new("historico_line")
            .show_background(false)
            .legend(Legend::default())
            .custom_x_axes(vec![AxisHints::new_x().formatter(x_fmt)])
            .custom_y_axes(vec![AxisHints::new_y().formatter(y_fmt)])
            .include_y(0.0)
            .height(155.0)
            .show(ui, |p| {
                p.line(Line::new(selected_name, pts).color(series.color).width(2.5));
            });
    }

    fn render_pct_stats(&self, ui: &mut Ui, selected_name: &str) {
        let series = match self.monthly_series.iter().find(|s| s.key == selected_name) {
            Some(s) => s,
            None => return,
        };
        let pcts: Vec<f64> = series.points.iter().map(|(_, p)| *p).collect();
        if pcts.is_empty() {
            return;
        }
        let min_p = pcts.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_p = pcts.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let avg_p = pcts.iter().sum::<f64>() / pcts.len() as f64;
        let cur_p = pcts
            .iter()
            .rev()
            .find(|p| **p > 0.0)
            .copied()
            .unwrap_or(0.0);

        let dark = ui.visuals().dark_mode;
        let card_bg = if dark {
            Color32::from_rgb(22, 26, 34)
        } else {
            Color32::from_rgb(245, 247, 251)
        };
        let tx = if dark {
            Color32::from_rgb(140, 155, 175)
        } else {
            Color32::from_rgb(90, 100, 120)
        };

        let items = [
            (
                "Mínimo",
                format!("{:.2}%", min_p),
                Color32::from_rgb(239, 68, 68),
            ),
            (
                "Médio",
                format!("{:.2}%", avg_p),
                Color32::from_rgb(148, 163, 184),
            ),
            (
                "Máximo",
                format!("{:.2}%", max_p),
                Color32::from_rgb(34, 197, 94),
            ),
            ("Atual", format!("{:.2}%", cur_p), series.color),
        ];
        ui.columns(4, |cols| {
            for (i, (label, value, color)) in items.iter().enumerate() {
                Frame::NONE
                    .fill(card_bg)
                    .corner_radius(egui::CornerRadius::same(6))
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .show(&mut cols[i], |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(RichText::new(*label).size(9.5).color(tx));
                        ui.label(RichText::new(value).size(14.0).strong().color(*color));
                    });
            }
        });
    }

    fn render_asset_monthly_table(&self, ui: &mut Ui, months: &[String], selected_name: &str) {
        // selected_name já é a chave (CD_ATIVO ou CD_ISIN)
        let asset_key = selected_name.to_string();

        let dt_col = match self.data.column("DT_COMPTC") {
            Ok(c) => c,
            Err(_) => return,
        };
        let cd_ativo_col = self.data.column("CD_ATIVO").ok();
        let cd_isin_col = self.data.column("CD_ISIN").ok();
        let merc_col = match self.data.column("VL_MERC_POS_FINAL") {
            Ok(c) => c,
            Err(_) => return,
        };
        let pct_col = self.data.column("VL_PORCENTAGEM_PL").ok();
        let qt_col = self.data.column("QT_REGIS").ok();
        let aquis_col = self.data.column("VL_AQUIS_NEGOC").ok();
        let height = self.data.height();

        // Build month → (merc, pct, qt, aquis) map for the selected asset
        let chronological_data: Vec<(String, f64, f64, f64, f64)> = months
            .iter()
            .map(|m| {
                let mut total_merc = 0.0_f64;
                let mut total_pct = 0.0_f64;
                let mut total_qt = 0.0_f64;
                let mut total_aquis = 0.0_f64;
                for i in 0..height {
                    let d = get_str(dt_col, i);
                    let mp = if d.len() >= 7 { &d[..7] } else { &d };
                    if mp != m.as_str() {
                        continue;
                    }
                    let row_key = {
                        let ca = cd_ativo_col
                            .as_ref()
                            .map(|c| get_str(c, i))
                            .unwrap_or_default();
                        if !ca.is_empty() {
                            ca
                        } else {
                            cd_isin_col
                                .as_ref()
                                .map(|c| get_str(c, i))
                                .unwrap_or_default()
                        }
                    };
                    if row_key == asset_key {
                        total_merc += get_f64(merc_col, i);
                        if let Some(pc) = pct_col {
                            total_pct += get_f64(pc, i);
                        }
                        if let Some(qc) = qt_col {
                            total_qt += get_f64(qc, i);
                        }
                        if let Some(ac) = aquis_col {
                            total_aquis += get_f64(ac, i);
                        }
                    }
                }
                (m.clone(), total_merc, total_pct, total_qt, total_aquis)
            })
            .collect();

        // Calculate Deltas and PM while in chronological order
        // month_data will hold: (month, merc, pct, delta_qt, pm_transacao)
        let mut month_data: Vec<(String, f64, f64, f64, f64)> = Vec::new();
        let mut last_qt = 0.0;
        let mut last_aquis = 0.0;

        for (m, merc, pct, qt, aquis) in chronological_data {
            let delta_qt = qt - last_qt;
            let delta_aquis = aquis - last_aquis;
            let est_price = if qt > 0.0 { merc / qt } else { 0.0 };

            let mut pm_transacao = 0.0;
            if delta_qt > 0.0 {
                // Bought
                pm_transacao = if delta_aquis > 0.0 {
                    delta_aquis / delta_qt
                } else {
                    est_price
                };
            } else if delta_qt < 0.0 {
                // Sold
                pm_transacao = est_price;
            }

            month_data.push((m, merc, pct, delta_qt, pm_transacao));
            last_qt = qt;
            last_aquis = aquis;
        }

        month_data.reverse(); // most recent first

        let dark = ui.visuals().dark_mode;
        let header_color = if dark {
            Color32::from_rgb(160, 175, 200)
        } else {
            Color32::from_rgb(60, 75, 100)
        };

        TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(60.0)) // Mês
            .column(Column::initial(70.0)) // %PL
            .column(Column::initial(90.0)) // Valor
            .column(Column::initial(85.0)) // Delta Qtde
            .column(Column::remainder()) // PM Transação
            .header(22.0, |mut h| {
                for label in &["Mês", "%PL", "Valor Merc.", "Mov. Qtde", "PM Transação"] {
                    h.col(|ui| {
                        ui.label(
                            RichText::new(*label)
                                .size(10.0)
                                .strong()
                                .color(header_color),
                        );
                    });
                }
            })
            .body(|body| {
                body.rows(20.0, month_data.len(), |mut row| {
                    let idx = row.index();
                    let (month, merc, pct, delta_qt, pm) = &month_data[idx];
                    let parts: Vec<&str> = month.split('-').collect();
                    let m_label = if parts.len() >= 2 {
                        format!("{}/{}", parts[1], &parts[0][2..])
                    } else {
                        month.clone()
                    };

                    let fmt_merc = if *merc >= 1_000_000_000.0 {
                        format!("R$ {:.2} Bi", merc / 1e9)
                    } else if *merc >= 1_000_000.0 {
                        format!("R$ {:.2} Mi", merc / 1e6)
                    } else if *merc >= 1_000.0 {
                        format!("R$ {:.1} K", merc / 1e3)
                    } else if *merc == 0.0 {
                        "-".to_string()
                    } else {
                        format!("R$ {:.2}", merc)
                    };

                    let (qt_color, qt_prefix) = if *delta_qt > 0.0 {
                        (Color32::from_rgb(34, 197, 94), "+")
                    } else if *delta_qt < 0.0 {
                        (Color32::from_rgb(239, 68, 68), "")
                    } else {
                        (Color32::GRAY, "")
                    };

                    row.col(|ui| {
                        ui.label(RichText::new(&m_label).size(10.5).monospace());
                    });
                    row.col(|ui| {
                        let c = if *pct > 0.0 {
                            Color32::from_rgb(34, 197, 94)
                        } else {
                            Color32::GRAY
                        };
                        ui.label(
                            RichText::new(if *pct == 0.0 {
                                "-".into()
                            } else {
                                format!("{:.2}%", pct)
                            })
                            .size(10.5)
                            .color(c),
                        );
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(&fmt_merc).size(10.5));
                    });
                    row.col(|ui| {
                        if *delta_qt != 0.0 {
                            ui.label(
                                RichText::new(format!("{}{:.0}", qt_prefix, delta_qt))
                                    .size(10.5)
                                    .color(qt_color),
                            );
                        } else {
                            ui.label(RichText::new("-").size(10.5).weak());
                        }
                    });
                    row.col(|ui| {
                        if *delta_qt != 0.0 && *pm > 0.0 {
                            ui.label(
                                RichText::new(format!("R$ {:.2}", pm))
                                    .size(10.5)
                                    .strong()
                                    .color(qt_color),
                            );
                        } else {
                            ui.label(RichText::new("-").size(10.5).weak());
                        }
                    });
                });
            });
    }

    /// Extract first-row info for the selected asset from raw data
    fn get_asset_info(
        &self,
        selected_name: &str,
    ) -> Option<(String, String, String, f64, f64, f64)> {
        // selected_name já é a chave
        let asset_key = selected_name.to_string();
        let cd_ativo_col = self.data.column("CD_ATIVO").ok();
        let cd_isin_col = self.data.column("CD_ISIN").ok();
        let tp_ativo_col = self.data.column("TP_ATIVO").ok();
        let tp_aplic_col = self.data.column("TP_APLIC").ok();
        let merc_col = self.data.column("VL_MERC_POS_FINAL").ok();
        let aquis_col = self.data.column("VL_AQUIS_NEGOC").ok();
        let qt_col = self
            .data
            .column("QT_POS_FINAL")
            .ok()
            .or_else(|| self.data.column("QT_REGIS").ok());

        for i in 0..self.data.height() {
            let ca = cd_ativo_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let ci = cd_isin_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let rk = if !ca.is_empty() { ca } else { ci };
            if rk == asset_key {
                let tp = tp_ativo_col
                    .as_ref()
                    .map(|c| get_str(c, i))
                    .unwrap_or_default();
                let ap = tp_aplic_col
                    .as_ref()
                    .map(|c| get_str(c, i))
                    .unwrap_or_default();
                let merc = merc_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0);
                let aquis = aquis_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0);
                let qt = qt_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0);
                return Some((asset_key, tp, ap, merc, aquis, qt));
            }
        }
        None
    }

    fn is_yahoo_eligible(tp_ativo: &str) -> bool {
        let u = tp_ativo.to_uppercase();
        u.contains("AÇÃO")
            || u.contains("STOCK")
            || u.contains("ETF")
            || u == "BDR"
            || u == "FII"
            || u.contains("FUNDO DE INVESTIMENTO IMOBILIÁRIO")
    }

    fn render_yahoo_inline(&self, ui: &mut Ui, codigo: &str) {
        let dark = ui.visuals().dark_mode;
        let _tx = if dark {
            Color32::from_rgb(140, 155, 175)
        } else {
            Color32::from_rgb(90, 100, 120)
        };

        if self.yahoo_loading {
            ui.vertical_centered(|ui| {
                ui.add_space(6.0);
                ui.spinner();
                ui.label(
                    RichText::new("Buscando preços no Yahoo Finance...")
                        .size(11.0)
                        .weak(),
                );
            });
            return;
        }
        let prices = match self.yahoo_prices.get(codigo) {
            Some(p) => p,
            None => {
                ui.label(RichText::new("Preços não disponíveis.").size(10.0).weak());
                return;
            }
        };
        let close_col = match prices.column("adjclose") {
            Ok(c) => c,
            Err(_) => return,
        };
        let n = prices.height().min(60);
        if n < 2 {
            return;
        }
        let pts: Vec<[f64; 2]> = (0..n).map(|i| [i as f64, get_f64(close_col, i)]).collect();
        let first_p = pts.first().map(|p| p[1]).unwrap_or(0.0);
        let last_p = pts.last().map(|p| p[1]).unwrap_or(0.0);
        let change = if first_p > 0.0 {
            ((last_p - first_p) / first_p) * 100.0
        } else {
            0.0
        };
        let chg_color = if change > 0.0 {
            Color32::from_rgb(34, 197, 94)
        } else if change < 0.0 {
            Color32::from_rgb(239, 68, 68)
        } else {
            Color32::GRAY
        };

        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("R$ {:.2}", last_p))
                    .size(15.0)
                    .strong(),
            );
            ui.label(
                RichText::new(format!("({:+.2}%)", change))
                    .size(12.0)
                    .color(chg_color),
            );
        });
        Plot::new("hist_yahoo_inline")
            .show_background(false)
            .height(70.0)
            .show(ui, |p| {
                p.line(
                    Line::new(codigo, pts)
                        .color(Color32::from_rgb(37, 99, 235))
                        .width(2.0),
                );
            });
    }

    fn render_analytics_inline(&mut self, ui: &mut Ui, codigo: &str) {
        let dark = ui.visuals().dark_mode;
        let (hd, tx, muted, bg) = if dark {
            (
                Color32::from_rgb(242, 247, 255),
                Color32::from_rgb(200, 210, 230),
                Color32::from_rgb(145, 155, 175),
                Color32::from_rgb(16, 20, 30),
            )
        } else {
            (
                Color32::from_rgb(8, 12, 28),
                Color32::from_rgb(40, 45, 60),
                Color32::from_rgb(105, 110, 125),
                Color32::from_rgb(248, 250, 253),
            )
        };
        let accent = Color32::from_rgb(37, 99, 235);
        let green = Color32::from_rgb(34, 197, 94);
        let red = Color32::from_rgb(239, 68, 68);
        let purple = Color32::from_rgb(139, 92, 246);

        // Cache: dispara task se não computado ainda
        if !self.analytics_cache.contains_key(codigo) {
            let yahoo_df = self.yahoo_prices.get(codigo).cloned();
            let data = self.data.clone();
            let codigo_owned = codigo.to_string();
            let sender = self.sender.clone();
            tokio::spawn(async move {
                let yahoo_ref = yahoo_df.as_ref();
                if let Some(a) =
                    crate::analytics::compute_asset_analytics(&data, &codigo_owned, yahoo_ref)
                {
                    let _ = sender.send(Message::AssetAnalyticsResult(codigo_owned, a));
                }
            });
        }

        let yahoo_data: Option<(&DataFrame, f64, f64, f64)> = self
            .get_asset_info(&self.selected_asset.clone().unwrap_or_default())
            .and_then(|(_, _, _, vl_merc, vl_aquis, qt_pos)| {
                self.yahoo_prices
                    .get(codigo)
                    .map(|prices| (prices, vl_merc, vl_aquis, qt_pos))
            });

        Frame::NONE
            .fill(bg)
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                // ── Linha 1: Preço + cotas ─────────────────────────
                if let Some((prices, _, _, qt_pos)) = yahoo_data {
                    let last_p = prices
                        .column("adjclose")
                        .ok()
                        .and_then(|c| {
                            let n = prices.height();
                            if n > 0 {
                                c.get(n - 1).ok()
                            } else {
                                None
                            }
                        })
                        .and_then(|v| v.try_extract::<f64>().ok())
                        .unwrap_or(0.0);

                    let first_p = prices
                        .column("adjclose")
                        .ok()
                        .and_then(|c| c.get(0).ok())
                        .and_then(|v| v.try_extract::<f64>().ok())
                        .unwrap_or(last_p);

                    let change = if first_p > 0.0 {
                        ((last_p - first_p) / first_p) * 100.0
                    } else {
                        0.0
                    };
                    let chg_color = if change > 0.0 {
                        green
                    } else if change < 0.0 {
                        red
                    } else {
                        tx
                    };

                    ui.horizontal(|ui| {
                        let price_str = crate::util::to_real(last_p)
                            .map(|r| r.format())
                            .unwrap_or_else(|_| format!("R$ {:.2}", last_p));
                        ui.label(RichText::new(price_str).size(26.0).strong().color(hd));
                        if change != 0.0 {
                            let icon = if change > 0.0 {
                                egui_phosphor::regular::TREND_UP
                            } else {
                                egui_phosphor::regular::TREND_DOWN
                            };
                            ui.label(
                                RichText::new(format!("  {} {:.2}%", icon, change.abs()))
                                    .size(14.0)
                                    .color(chg_color),
                            );
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{}  {}",
                                    egui_phosphor::regular::STACK,
                                    fmt_num(qt_pos)
                                ))
                                .size(11.0)
                                .color(muted),
                            );
                        });
                    });

                    // PM Compra
                    ui.add_space(3.0);
                    ui.horizontal(|ui| {
                        if let Some(analytics) = self.analytics_cache.get(codigo) {
                            if analytics.avg_buy_price > 0.0 {
                                let pm_str = crate::util::to_real(analytics.avg_buy_price)
                                    .map(|r| r.format())
                                    .unwrap_or_else(|_| {
                                        format!("R$ {:.2}", analytics.avg_buy_price)
                                    });
                                ui.label(
                                    RichText::new(format!(
                                        "{} PM Compra",
                                        egui_phosphor::regular::SHOPPING_CART
                                    ))
                                    .size(11.0)
                                    .color(muted),
                                );
                                ui.label(RichText::new(pm_str).size(12.0).strong().color(accent));
                            }
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if let Some((_, _, vl_aquis, _)) = yahoo_data {
                                if vl_aquis > 0.0 {
                                    let cost_str = crate::util::to_real(vl_aquis)
                                        .map(|r| r.format())
                                        .unwrap_or_else(|_| format!("R$ {:.2}", vl_aquis));
                                    ui.label(
                                        RichText::new(format!("Custo {}", cost_str))
                                            .size(10.0)
                                            .color(muted),
                                    );
                                }
                            }
                        });
                    });
                }

                // ── Extrapolação ──────────────────────────────────
                if let Some(analytics) = self.analytics_cache.get(codigo) {
                    if let Some(last_est) = analytics.hidden_qty_estimates.last() {
                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(4.0);

                        let max_qty = analytics
                            .hidden_qty_estimates
                            .first()
                            .map(|e| e.1)
                            .unwrap_or(last_est.1)
                            .max(last_est.1)
                            .max(1.0);
                        let bar_pct = (last_est.1 / max_qty).clamp(0.0, 1.0) as f32;

                        ui.label(
                            RichText::new(format!(
                                "{} Posição Oculta Estimada",
                                egui_phosphor::regular::EYE_SLASH
                            ))
                            .size(11.0)
                            .strong()
                            .color(tx),
                        );
                        ui.add_space(3.0);

                        // Barra de progresso
                        let bar_h = 8.0;
                        let (bar_rect, _) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), bar_h),
                            Sense::hover(),
                        );
                        ui.painter().rect_filled(
                            bar_rect,
                            egui::CornerRadius::same(4),
                            if dark {
                                Color32::from_rgb(35, 40, 55)
                            } else {
                                Color32::from_rgb(220, 228, 240)
                            },
                        );
                        let fill = egui::Rect::from_min_size(
                            bar_rect.min,
                            egui::vec2(bar_rect.width() * bar_pct, bar_h),
                        );
                        ui.painter()
                            .rect_filled(fill, egui::CornerRadius::same(4), accent);

                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} {}",
                                    egui_phosphor::regular::CIRCLES_THREE_PLUS,
                                    fmt_num(last_est.1)
                                ))
                                .size(12.0)
                                .strong()
                                .color(hd),
                            );
                            let val_fmt = crate::util::to_real(last_est.2)
                                .map(|r| r.format())
                                .unwrap_or_else(|_| format!("R$ {:.2}", last_est.2));
                            ui.label(RichText::new(format!("≈ {}", val_fmt)).size(11.0).color(tx));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if let Some(ref inf) = analytics.portfolio_inference {
                                    let (score_color, badge_bg) = if inf.stability_score > 0.7 {
                                        (
                                            green,
                                            if dark {
                                                Color32::from_rgb(20, 50, 30)
                                            } else {
                                                Color32::from_rgb(220, 245, 225)
                                            },
                                        )
                                    } else if inf.stability_score > 0.4 {
                                        (
                                            purple,
                                            if dark {
                                                Color32::from_rgb(40, 25, 60)
                                            } else {
                                                Color32::from_rgb(240, 230, 255)
                                            },
                                        )
                                    } else {
                                        (
                                            red,
                                            if dark {
                                                Color32::from_rgb(55, 20, 25)
                                            } else {
                                                Color32::from_rgb(255, 225, 225)
                                            },
                                        )
                                    };
                                    let bias_icon = match inf.bias_direction {
                                        crate::analytics::TradeBias::Acumulando => {
                                            egui_phosphor::regular::ARROW_UP
                                        }
                                        crate::analytics::TradeBias::Distribuindo => {
                                            egui_phosphor::regular::ARROW_DOWN
                                        }
                                        crate::analytics::TradeBias::Consistente => {
                                            egui_phosphor::regular::EQUALS
                                        }
                                    };
                                    let qualifier = if inf.stability_score < 0.05 {
                                        "Caótico"
                                    } else if inf.stability_score < 0.3 {
                                        "Instável"
                                    } else {
                                        "Estável"
                                    };
                                    let badge_text = format!(
                                        "{} {:.0}% inferível  {} {}",
                                        egui_phosphor::regular::BRAIN,
                                        inf.stability_score * 100.0,
                                        bias_icon,
                                        qualifier
                                    );
                                    Frame::NONE
                                        .fill(badge_bg)
                                        .corner_radius(egui::CornerRadius::same(10))
                                        .inner_margin(egui::Margin::symmetric(8, 3))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(badge_text)
                                                    .size(10.0)
                                                    .color(score_color),
                                            );
                                        });
                                }
                            });
                        });
                    }
                }

                // ── Estratégia ────────────────────────────────────
                if let Some(analytics) = self.analytics_cache.get(codigo) {
                    ui.add_space(6.0);
                    ui.separator();
                    ui.add_space(4.0);

                    ui.label(
                        RichText::new(format!(
                            "{} Estratégia do Gestor",
                            egui_phosphor::regular::LIGHTNING
                        ))
                        .size(11.0)
                        .strong()
                        .color(tx),
                    );
                    ui.add_space(3.0);

                    ui.columns(3, |cols| {
                        cols[0].vertical(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} Montagem",
                                    egui_phosphor::regular::HOURGLASS_HIGH
                                ))
                                .size(9.0)
                                .color(muted),
                            );
                            if analytics.speed_to_peak > 0 {
                                let speed_color = if analytics.speed_to_peak <= 2 {
                                    purple
                                } else {
                                    tx
                                };
                                ui.label(
                                    RichText::new(format!("{} meses", analytics.speed_to_peak))
                                        .size(13.0)
                                        .strong()
                                        .color(speed_color),
                                );
                            } else {
                                ui.label(RichText::new("—").size(13.0).weak());
                            }
                        });
                        cols[1].vertical(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} Take-Profit",
                                    egui_phosphor::regular::FLAG_BANNER
                                ))
                                .size(9.0)
                                .color(muted),
                            );
                            if analytics.take_profit_trigger > 0.0 {
                                ui.label(
                                    RichText::new(format!(
                                        "{:.1}% PL",
                                        analytics.take_profit_trigger
                                    ))
                                    .size(13.0)
                                    .strong()
                                    .color(green),
                                );
                            } else {
                                ui.label(RichText::new("—").size(13.0).weak());
                            }
                        });
                        cols[2].vertical(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} Ganho Oculto",
                                    egui_phosphor::regular::MAGNIFYING_GLASS
                                ))
                                .size(9.0)
                                .color(muted),
                            );
                            if let Some((prices, _, vl_aquis, qt_pos)) = yahoo_data {
                                let last_p = prices
                                    .column("adjclose")
                                    .ok()
                                    .and_then(|c| {
                                        let n = prices.height();
                                        if n > 0 {
                                            c.get(n - 1).ok()
                                        } else {
                                            None
                                        }
                                    })
                                    .and_then(|v| v.try_extract::<f64>().ok())
                                    .unwrap_or(0.0);
                                let gain = qt_pos * last_p - vl_aquis;
                                let gain_pct = if vl_aquis > 0.0 {
                                    (gain / vl_aquis) * 100.0
                                } else {
                                    0.0
                                };
                                let gain_color = if gain > 0.0 { green } else { red };
                                ui.label(
                                    RichText::new(format!("{:+.1}%", gain_pct))
                                        .size(13.0)
                                        .strong()
                                        .color(gain_color),
                                );
                            } else {
                                ui.label(RichText::new("—").size(13.0).weak());
                            }
                        });
                    });
                }
            });
    }
}

impl Tab for HistoricoTab {
    fn title(&self) -> WidgetText {
        format!(
            "{} Histórico",
            egui_phosphor::regular::CLOCK_COUNTER_CLOCKWISE
        )
        .into()
    }

    fn closeable(&self) -> bool {
        true
    }

    fn ui(&mut self, ui: &mut Ui) {
        Frame::NONE.inner_margin(6.0).show(ui, |ui| {
            if self.loading {
                ui.vertical_centered(|ui| {
                    ui.add_space(60.0);
                    ui.label(
                        RichText::new(egui_phosphor::regular::CLOCK_COUNTER_CLOCKWISE.to_string())
                            .size(36.0)
                            .color(Color32::from_rgb(37, 99, 235)),
                    );
                    ui.add_space(10.0);
                    ui.label(RichText::new("Carregando histórico...").size(14.0).strong());
                    ui.add_space(6.0);
                    ui.spinner();
                });
                return;
            }
            if self.data.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new("Nenhum dado histórico encontrado.")
                            .size(13.0)
                            .weak(),
                    );
                });
                return;
            }

            let months = self.extract_months();
            let dark = ui.visuals().dark_mode;

            // Collect asset items for the left panel (avoid borrow conflict)
            let mut asset_items: Vec<(String, String, Color32, f64)> = self
                .monthly_series
                .iter()
                .map(|s| {
                    let latest = s
                        .points
                        .iter()
                        .rev()
                        .find(|(_, p)| *p > 0.0)
                        .map(|(_, p)| *p)
                        .unwrap_or(0.0);
                    (s.key.clone(), s.label.clone(), s.color, latest)
                })
                .collect();
            asset_items.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));

            let mut new_selected: Option<String> = None;
            let current_sel = self.selected_asset.clone();

            // ── Left panel: clickable asset list ──────────────────
            egui::Panel::left("hist_left_panel")
                .resizable(true)
                .default_size(230.0)
                .show_inside(ui, |ui| {
                    let panel_bg = if dark {
                        Color32::from_rgb(18, 20, 28)
                    } else {
                        Color32::from_rgb(250, 251, 253)
                    };
                    Frame::NONE.fill(panel_bg).show(ui, |ui| {
                        ui.add_space(6.0);
                        ui.label(RichText::new("ATIVOS").size(9.5).strong().color(if dark {
                            Color32::from_rgb(90, 105, 130)
                        } else {
                            Color32::from_rgb(150, 165, 185)
                        }));
                        ui.add_space(4.0);

                        ScrollArea::vertical().show(ui, |ui| {
                            for (key, name, color, latest_pct) in &asset_items {
                                let is_sel = current_sel.as_deref() == Some(key.as_str());
                                let sel_bg = if dark {
                                    Color32::from_rgb(30, 42, 68)
                                } else {
                                    Color32::from_rgb(219, 234, 254)
                                };
                                let hov_bg = if dark {
                                    Color32::from_rgb(26, 30, 40)
                                } else {
                                    Color32::from_rgb(240, 244, 252)
                                };
                                let txt_color = if dark {
                                    Color32::from_rgb(210, 220, 235)
                                } else {
                                    Color32::from_rgb(30, 40, 60)
                                };
                                let pct_color = if *latest_pct > 0.0 {
                                    Color32::from_rgb(34, 197, 94)
                                } else {
                                    Color32::GRAY
                                };

                                let avail_w = ui.available_width();
                                let (rect, resp) = ui
                                    .allocate_exact_size(egui::vec2(avail_w, 30.0), Sense::click());
                                let bg = if is_sel {
                                    sel_bg
                                } else if resp.hovered() {
                                    hov_bg
                                } else {
                                    Color32::TRANSPARENT
                                };
                                ui.painter().rect_filled(rect, 5.0, bg);

                                // Color dot
                                ui.painter().circle_filled(
                                    egui::pos2(rect.min.x + 12.0, rect.center().y),
                                    4.0,
                                    *color,
                                );

                                // Name (truncated)
                                let display = if name.len() > 18 {
                                    format!("{}…", &name[..18])
                                } else {
                                    name.clone()
                                };
                                ui.painter().text(
                                    egui::pos2(rect.min.x + 22.0, rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    &display,
                                    egui::FontId::proportional(11.0),
                                    txt_color,
                                );

                                // %PL right-aligned
                                ui.painter().text(
                                    egui::pos2(rect.max.x - 8.0, rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    format!("{:.1}%", latest_pct),
                                    egui::FontId::proportional(10.0),
                                    pct_color,
                                );

                                if resp.clicked() {
                                    new_selected = Some(key.clone());
                                }
                            }
                        });
                    });
                });

            // Apply actions after panel
            if let Some(name) = new_selected {
                self.selected_asset = Some(name);
            }

            // Right panel: chart + stats + table + yahoo
            ui.vertical(|ui| {
                if let Some(sel) = self.selected_asset.clone() {
                    // Auto-fetch Yahoo if eligible and not yet fetched
                    if let Some((codigo, tp_ativo, ap, _vl_merc, _vl_aquis, _qt_pos)) =
                        self.get_asset_info(&sel)
                    {
                        if Self::is_yahoo_eligible(&tp_ativo)
                            && !self.yahoo_prices.contains_key(&codigo)
                            && !self.yahoo_loading
                            && self.last_yahoo_fetch.as_deref() != Some(&codigo)
                        {
                            self.yahoo_loading = true;
                            self.last_yahoo_fetch = Some(codigo.clone());
                            let end = chrono::Local::now().naive_local().date();
                            let start = end
                                .checked_sub_months(chrono::Months::new(24))
                                .unwrap_or(end - chrono::Duration::days(730));
                            let _ = self.sender.send(Message::FetchYahooPrice(
                                codigo.clone(),
                                self.cnpj.clone(),
                                start,
                                end,
                            ));
                        }

                        // Header
                        let dark = ui.visuals().dark_mode;
                        let tx = if dark {
                            Color32::from_rgb(140, 155, 175)
                        } else {
                            Color32::from_rgb(90, 100, 120)
                        };
                        let hd = if dark {
                            Color32::WHITE
                        } else {
                            Color32::from_rgb(20, 30, 50)
                        };

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&codigo)
                                    .size(14.0)
                                    .strong()
                                    .monospace()
                                    .color(hd),
                            );
                            ui.label(RichText::new(&sel).size(13.0).color(hd));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format!("{}m", months.len()))
                                        .size(10.0)
                                        .weak(),
                                );
                                if !ap.is_empty() {
                                    ui.label(RichText::new(&ap).size(10.0).color(tx));
                                }
                                if !tp_ativo.is_empty() {
                                    ui.label(RichText::new(&tp_ativo).size(10.0).color(tx));
                                }
                            });
                        });
                        ui.add_space(4.0);

                        // %PL chart + stat cards
                        self.render_asset_line_chart(ui, &months, &sel);
                        ui.add_space(5.0);
                        self.render_pct_stats(ui, &sel);
                        ui.add_space(6.0);

                        // ── Card: Análise do Gestor ────────────────────
                        self.render_analytics_inline(ui, &codigo);
                        ui.add_space(6.0);

                        // Yahoo sparkline (lite)
                        if Self::is_yahoo_eligible(&tp_ativo) {
                            self.render_yahoo_inline(ui, &codigo);
                            ui.add_space(4.0);
                        }

                        // Monthly detail table
                        ui.separator();
                        ui.add_space(4.0);
                        ui.label(RichText::new("Detalhamento Mensal").size(11.0).strong());
                        ui.add_space(3.0);
                        self.render_asset_monthly_table(ui, &months, &sel);
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.label(
                            RichText::new(egui_phosphor::regular::CHART_LINE_UP.to_string())
                                .size(40.0)
                                .color(Color32::from_rgb(100, 120, 160)),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("Selecione um ativo à esquerda")
                                .size(14.0)
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("para ver o histórico e as estatísticas")
                                .size(11.0)
                                .weak(),
                        );
                    });
                }
            });
        });
    }
}
