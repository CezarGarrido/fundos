use std::collections::{BTreeMap, HashMap};

use egui::{Align, Color32, Frame, Layout, RichText, ScrollArea, Sense, Ui, WidgetText};
use egui_extras::{Column, TableBuilder};
use egui_plot::{AxisHints, GridMark, Legend, Line, Plot};
use polars::frame::DataFrame;
use tokio::sync::mpsc::UnboundedSender;

use crate::{message::Message, ui::tabs::Tab};

fn fmt_val(v: f64) -> String {
    if v >= 1_000_000_000.0 { format!("R$ {:.2} Bi", v / 1e9) }
    else if v >= 1_000_000.0 { format!("R$ {:.2} Mi", v / 1e6) }
    else if v >= 1_000.0 { format!("R$ {:.1} K", v / 1e3) }
    else if v == 0.0 { "-".into() }
    else { format!("R$ {:.2}", v) }
}

fn get_str<'a>(col: &'a polars::series::Series, row: usize) -> String {
    col.get(row)
        .ok()
        .and_then(|v| v.get_str().map(|s| s.to_string()))
        .unwrap_or_default()
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
    pub last_yahoo_fetch: Option<String>, // tracks last fetched asset to avoid re-fetch
}

pub struct MonthlySeries {
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
        self.process_series();
        self.loading = false;
    }

    pub fn set_yahoo_prices(&mut self, codigo: String, prices: DataFrame) {
        self.yahoo_prices.insert(codigo, prices);
        self.yahoo_loading = false;
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

    fn process_series(&mut self) {
        self.monthly_series.clear();

        let dt_col = match self.data.column("DT_COMPTC") {
            Ok(c) => c,
            Err(_) => return,
        };
        let cd_ativo_col = self.data.column("CD_ATIVO").ok();
        let cd_isin_col = self.data.column("CD_ISIN").ok();
        let titpub_col = self.data.column("TP_TITPUB").ok();
        let ds_ativo_col = self.data.column("DS_ATIVO").ok();
        let nm_fundo_col = self.data.column("NM_FUNDO_COTA").ok();
        let aplic_col = self.data.column("TP_APLIC").ok();
        let merc_col = match self.data.column("VL_MERC_POS_FINAL") {
            Ok(c) => c,
            Err(_) => return,
        };
        let pct_col = self.data.column("VL_PORCENTAGEM_PL").ok();

        let height = self.data.height();
        if height == 0 {
            return;
        }

        let months = self.extract_months();

        // Build asset identity: key = CD_ATIVO or CD_ISIN, name = best available description
        let mut asset_keys: Vec<(String, String)> = vec![]; // (key, display_name)
        for i in 0..height {
            let cd_ativo = cd_ativo_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let cd_isin = cd_isin_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let key = if !cd_ativo.is_empty() {
                cd_ativo
            } else if !cd_isin.is_empty() {
                cd_isin
            } else {
                continue;
            }; // skip if no identifier

            let titpub = titpub_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let ds_ativo = ds_ativo_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let nm_fundo = nm_fundo_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let aplic = aplic_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();

            let name = if !titpub.is_empty() {
                titpub
            } else if !ds_ativo.is_empty() {
                ds_ativo
            } else if !nm_fundo.is_empty() {
                nm_fundo
            } else if !aplic.is_empty() {
                aplic
            } else {
                key.clone()
            };

            if !asset_keys.iter().any(|(k, _)| k == &key) {
                asset_keys.push((key, name));
            }
        }

        // Compute total value per asset for ranking
        let mut total_per_asset: BTreeMap<String, f64> = BTreeMap::new();
        for i in 0..height {
            let cd_ativo = cd_ativo_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let cd_isin = cd_isin_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let key = if !cd_ativo.is_empty() {
                cd_ativo
            } else if !cd_isin.is_empty() {
                cd_isin
            } else {
                continue;
            };
            let val = get_f64(merc_col, i);
            *total_per_asset.entry(key).or_default() += val;
        }

        // Sort by total value, take top 8
        let mut sorted: Vec<(String, f64)> = total_per_asset.into_iter().collect();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top_keys: Vec<String> = sorted.into_iter().take(8).map(|(k, _)| k).collect();

        let palette = [
            Color32::from_rgb(37, 99, 235),
            Color32::from_rgb(34, 197, 94),
            Color32::from_rgb(239, 68, 68),
            Color32::from_rgb(234, 179, 8),
            Color32::from_rgb(139, 92, 246),
            Color32::from_rgb(249, 115, 22),
            Color32::from_rgb(20, 184, 166),
            Color32::from_rgb(236, 72, 153),
        ];

        for (idx, key) in top_keys.iter().enumerate() {
            // Get display name
            let name = asset_keys
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, n)| n.clone())
                .unwrap_or_else(|| key.clone());
            let mut points: Vec<(f64, f64)> = vec![];
            for (mi, month) in months.iter().enumerate() {
                let mut asset_val = 0.0_f64;
                let mut month_total = 0.0_f64;
                for i in 0..height {
                    let d = get_str(dt_col, i);
                    let val = get_f64(merc_col, i);
                    let m_prefix = if d.len() >= 7 { &d[..7] } else { &d };
                    if m_prefix == month {
                        month_total += val;
                        let cd_ativo = cd_ativo_col
                            .as_ref()
                            .map(|c| get_str(c, i))
                            .unwrap_or_default();
                        let cd_isin = cd_isin_col
                            .as_ref()
                            .map(|c| get_str(c, i))
                            .unwrap_or_default();
                        let row_key = if !cd_ativo.is_empty() {
                            cd_ativo
                        } else {
                            cd_isin
                        };
                        if row_key == *key {
                            asset_val += val;
                        }
                    }
                }
                let pct = if pct_col.is_some() {
                    let mut total = 0.0;
                    for i in 0..height {
                        let d = get_str(dt_col, i);
                        let m_prefix = if d.len() >= 7 { &d[..7] } else { &d };
                        if m_prefix == month {
                            let cd_ativo = cd_ativo_col
                                .as_ref()
                                .map(|c| get_str(c, i))
                                .unwrap_or_default();
                            let cd_isin = cd_isin_col
                                .as_ref()
                                .map(|c| get_str(c, i))
                                .unwrap_or_default();
                            let row_key = if !cd_ativo.is_empty() {
                                cd_ativo
                            } else {
                                cd_isin
                            };
                            if row_key == *key {
                                if let Some(ref pc) = pct_col {
                                    total += get_f64(pc, i);
                                }
                            }
                        }
                    }
                    total
                } else {
                    if month_total > 0.0 {
                        (asset_val / month_total) * 100.0
                    } else {
                        0.0
                    }
                };
                points.push((mi as f64, pct));
            }
            self.monthly_series.push(MonthlySeries {
                label: name,
                color: palette[idx % palette.len()],
                points,
            });
        }
    }

    fn render_asset_line_chart(&self, ui: &mut Ui, months: &[String], selected_name: &str) {
        let series = match self.monthly_series.iter().find(|s| s.label == selected_name) {
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
                if parts.len() >= 2 { return format!("{}/{}", parts[1], &parts[0][2..]); }
                m.clone()
            } else { String::new() }
        };
        let y_fmt = |mark: GridMark, _: &std::ops::RangeInclusive<f64>| format!("{:.1}%", mark.value);
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
        let series = match self.monthly_series.iter().find(|s| s.label == selected_name) {
            Some(s) => s,
            None => return,
        };
        let pcts: Vec<f64> = series.points.iter().map(|(_, p)| *p).collect();
        if pcts.is_empty() { return; }
        let min_p = pcts.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_p = pcts.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let avg_p = pcts.iter().sum::<f64>() / pcts.len() as f64;
        let cur_p = pcts.iter().rev().find(|p| **p > 0.0).copied().unwrap_or(0.0);

        let dark = ui.visuals().dark_mode;
        let card_bg = if dark { Color32::from_rgb(22, 26, 34) } else { Color32::from_rgb(245, 247, 251) };
        let tx = if dark { Color32::from_rgb(140, 155, 175) } else { Color32::from_rgb(90, 100, 120) };

        let items = [
            ("Mínimo",  format!("{:.2}%", min_p), Color32::from_rgb(239, 68, 68)),
            ("Médio",   format!("{:.2}%", avg_p), Color32::from_rgb(148, 163, 184)),
            ("Máximo",  format!("{:.2}%", max_p), Color32::from_rgb(34, 197, 94)),
            ("Atual",   format!("{:.2}%", cur_p), series.color),
        ];
        ui.columns(4, |cols| {
            for (i, (label, value, color)) in items.iter().enumerate() {
                Frame::NONE.fill(card_bg).corner_radius(egui::CornerRadius::same(6))
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
        let name_to_key: HashMap<String, String> = self
            .collect_asset_keys()
            .into_iter()
            .map(|(k, n)| (n, k))
            .collect();
        let asset_key = match name_to_key.get(selected_name) {
            Some(k) => k.clone(),
            None => return,
        };

        let dt_col = match self.data.column("DT_COMPTC") { Ok(c) => c, Err(_) => return };
        let cd_ativo_col = self.data.column("CD_ATIVO").ok();
        let cd_isin_col  = self.data.column("CD_ISIN").ok();
        let merc_col     = match self.data.column("VL_MERC_POS_FINAL") { Ok(c) => c, Err(_) => return };
        let pct_col      = self.data.column("VL_PORCENTAGEM_PL").ok();
        let height = self.data.height();

        // Build month → (merc, pct) map for the selected asset
        let mut month_data: Vec<(String, f64, f64)> = months.iter().map(|m| {
            let mut total_merc = 0.0_f64;
            let mut total_pct  = 0.0_f64;
            for i in 0..height {
                let d = get_str(dt_col, i);
                let mp = if d.len() >= 7 { &d[..7] } else { &d };
                if mp != m.as_str() { continue; }
                let row_key = {
                    let ca = cd_ativo_col.as_ref().map(|c| get_str(c, i)).unwrap_or_default();
                    if !ca.is_empty() { ca } else { cd_isin_col.as_ref().map(|c| get_str(c, i)).unwrap_or_default() }
                };
                if row_key == asset_key {
                    total_merc += get_f64(merc_col, i);
                    if let Some(ref pc) = pct_col { total_pct += get_f64(pc, i); }
                }
            }
            (m.clone(), total_merc, total_pct)
        }).collect();
        month_data.reverse(); // most recent first

        let dark = ui.visuals().dark_mode;
        let header_color = if dark { Color32::from_rgb(160, 175, 200) } else { Color32::from_rgb(60, 75, 100) };

        TableBuilder::new(ui)
            .striped(true)
            .resizable(false)
            .cell_layout(Layout::left_to_right(Align::Center))
            .column(Column::exact(70.0))   // Mês
            .column(Column::initial(90.0)) // %PL
            .column(Column::initial(110.0)) // Valor
            .column(Column::remainder())   // Var
            .header(22.0, |mut h| {
                for label in &["Mês", "%PL", "Valor Merc.", "Variação"] {
                    h.col(|ui| { ui.label(RichText::new(*label).size(10.5).strong().color(header_color)); });
                }
            })
            .body(|body| {
                body.rows(20.0, month_data.len(), |mut row| {
                    let idx = row.index();
                    let (month, merc, pct) = &month_data[idx];
                    let parts: Vec<&str> = month.split('-').collect();
                    let m_label = if parts.len() >= 2 { format!("{}/{}", parts[1], &parts[0][2..]) } else { month.clone() };

                    // Variation vs previous row
                    let prev_pct = if idx + 1 < month_data.len() { month_data[idx + 1].2 } else { *pct };
                    let var = pct - prev_pct;
                    let (var_color, var_prefix) = if var > 0.0 {
                        (Color32::from_rgb(34, 197, 94), "+")
                    } else if var < 0.0 {
                        (Color32::from_rgb(239, 68, 68), "")
                    } else {
                        (Color32::GRAY, "")
                    };

                    let fmt_merc = if *merc >= 1_000_000_000.0 { format!("R$ {:.2} Bi", merc / 1e9) }
                        else if *merc >= 1_000_000.0 { format!("R$ {:.2} Mi", merc / 1e6) }
                        else if *merc >= 1_000.0     { format!("R$ {:.1} K",  merc / 1e3) }
                        else if *merc == 0.0         { "-".to_string() }
                        else                          { format!("R$ {:.2}", merc) };

                    row.col(|ui| { ui.label(RichText::new(&m_label).size(10.5).monospace()); });
                    row.col(|ui| {
                        let c = if *pct > 0.0 { Color32::from_rgb(34, 197, 94) } else { Color32::GRAY };
                        ui.label(RichText::new(if *pct == 0.0 { "-".into() } else { format!("{:.2}%", pct) }).size(10.5).color(c));
                    });
                    row.col(|ui| { ui.label(RichText::new(&fmt_merc).size(10.5)); });
                    row.col(|ui| {
                        if idx + 1 < month_data.len() && *pct != 0.0 {
                            ui.label(RichText::new(format!("{}{:.2}%", var_prefix, var)).size(10.0).color(var_color));
                        }
                    });
                });
            });
    }

    fn collect_asset_keys(&self) -> Vec<(String, String)> {
        let cd_ativo_col = self.data.column("CD_ATIVO").ok();
        let cd_isin_col = self.data.column("CD_ISIN").ok();
        let titpub_col = self.data.column("TP_TITPUB").ok();
        let ds_ativo_col = self.data.column("DS_ATIVO").ok();
        let nm_fundo_col = self.data.column("NM_FUNDO_COTA").ok();
        let aplic_col = self.data.column("TP_APLIC").ok();

        let mut result: Vec<(String, String)> = vec![];
        for i in 0..self.data.height() {
            let cd_ativo = cd_ativo_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let cd_isin = cd_isin_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let key = if !cd_ativo.is_empty() {
                cd_ativo
            } else if !cd_isin.is_empty() {
                cd_isin
            } else {
                continue;
            };

            let titpub = titpub_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let ds_ativo = ds_ativo_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let nm_fundo = nm_fundo_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();
            let aplic = aplic_col
                .as_ref()
                .map(|c| get_str(c, i))
                .unwrap_or_default();

            let name = if !titpub.is_empty() {
                titpub
            } else if !ds_ativo.is_empty() {
                ds_ativo
            } else if !nm_fundo.is_empty() {
                nm_fundo
            } else if !aplic.is_empty() {
                aplic
            } else {
                key.clone()
            };

            if !result.iter().any(|(k, _)| k == &key) {
                result.push((key, name));
            }
        }
        result
    }


    /// Extract first-row info for the selected asset from raw data
    fn get_asset_info(&self, selected_name: &str) -> Option<(String, String, String, f64, f64)> {
        let name_to_key: HashMap<String, String> = self
            .collect_asset_keys()
            .into_iter()
            .map(|(k, n)| (n, k))
            .collect();
        let asset_key = name_to_key.get(selected_name)?.clone();
        let cd_ativo_col = self.data.column("CD_ATIVO").ok();
        let cd_isin_col  = self.data.column("CD_ISIN").ok();
        let tp_ativo_col = self.data.column("TP_ATIVO").ok();
        let tp_aplic_col = self.data.column("TP_APLIC").ok();
        let merc_col     = self.data.column("VL_MERC_POS_FINAL").ok();
        let aquis_col    = self.data.column("VL_AQUIS_NEGOC").ok();
        for i in 0..self.data.height() {
            let ca = cd_ativo_col.as_ref().map(|c| get_str(c, i)).unwrap_or_default();
            let ci = cd_isin_col.as_ref().map(|c| get_str(c, i)).unwrap_or_default();
            let rk = if !ca.is_empty() { ca } else { ci };
            if rk == asset_key {
                let tp = tp_ativo_col.as_ref().map(|c| get_str(c, i)).unwrap_or_default();
                let ap = tp_aplic_col.as_ref().map(|c| get_str(c, i)).unwrap_or_default();
                let merc  = merc_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0);
                let aquis = aquis_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0);
                return Some((asset_key, tp, ap, merc, aquis));
            }
        }
        None
    }

    fn is_yahoo_eligible(tp_ativo: &str) -> bool {
        let u = tp_ativo.to_uppercase();
        u.contains("AÇÃO") || u.contains("STOCK") || u.contains("ETF")
            || u == "BDR" || u == "FII" || u.contains("FUNDO DE INVESTIMENTO IMOBILIÁRIO")
    }

    fn render_yahoo_inline(&self, ui: &mut Ui, codigo: &str, vl_merc: f64, vl_aquis: f64) {
        let dark = ui.visuals().dark_mode;
        let tx = if dark { Color32::from_rgb(140, 155, 175) } else { Color32::from_rgb(90, 100, 120) };

        if self.yahoo_loading {
            ui.vertical_centered(|ui| {
                ui.add_space(6.0);
                ui.spinner();
                ui.label(RichText::new("Buscando preços no Yahoo Finance...").size(11.0).weak());
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
        let close_col = match prices.column("adjclose") { Ok(c) => c, Err(_) => return };
        let n = prices.height().min(60);
        if n < 2 { return; }
        let pts: Vec<[f64; 2]> = (0..n).map(|i| [i as f64, get_f64(close_col, i)]).collect();
        let first_p = pts.first().map(|p| p[1]).unwrap_or(0.0);
        let last_p  = pts.last().map(|p| p[1]).unwrap_or(0.0);
        let change = if first_p > 0.0 { ((last_p - first_p) / first_p) * 100.0 } else { 0.0 };
        let chg_color = if change > 0.0 { Color32::from_rgb(34, 197, 94) } else if change < 0.0 { Color32::from_rgb(239, 68, 68) } else { Color32::GRAY };

        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("R$ {:.2}", last_p)).size(15.0).strong());
            ui.label(RichText::new(format!("({:+.2}%)", change)).size(12.0).color(chg_color));
        });
        Plot::new("hist_yahoo_inline")
            .show_background(false)
            .height(90.0)
            .show(ui, |p| {
                p.line(Line::new(codigo, pts).color(Color32::from_rgb(37, 99, 235)).width(2.0));
            });

        // Estimated qty + avg price
        let qty = if last_p > 0.0 { vl_merc / last_p } else { 0.0 };
        let avg = if qty > 0.0 { vl_aquis / qty } else { 0.0 };
        ui.columns(3, |cols| {
            for (i, (lbl, val)) in [
                ("Qtde. Estimada", format!("{:.0}", qty)),
                ("Valor Posição",  fmt_val(vl_merc)),
                ("PM Compra",       if avg > 0.0 { format!("R$ {:.2}", avg) } else { "N/A".into() }),
            ].iter().enumerate() {
                cols[i].label(RichText::new(*lbl).size(9.0).color(tx));
                cols[i].label(RichText::new(val).size(12.0).strong());
            }
        });
    }

    fn render_analytics_inline(&self, ui: &mut Ui, codigo: &str) {
        let dark = ui.visuals().dark_mode;
        let tx = if dark { Color32::from_rgb(140, 155, 175) } else { Color32::from_rgb(90, 100, 120) };
        let gr = Color32::from_rgb(34, 197, 94);
        let vt = Color32::from_rgb(139, 92, 246);
        
        if let Some(analytics) = crate::analytics::compute_asset_analytics(&self.data, codigo) {
            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    ui.label(RichText::new("Gatilho Take-Profit Estimado").size(9.0).color(tx));
                    if analytics.take_profit_trigger > 0.0 {
                        ui.label(RichText::new(format!("{:.2}% PL", analytics.take_profit_trigger)).size(13.0).strong().color(vt));
                    } else {
                        ui.label(RichText::new("Não detectado").size(11.0).weak());
                    }
                });
                cols[1].vertical(|ui| {
                    ui.label(RichText::new("Velocidade de Montagem").size(9.0).color(tx));
                    if analytics.speed_to_peak > 0 {
                        ui.label(RichText::new(format!("{} meses até o pico", analytics.speed_to_peak)).size(13.0).strong().color(gr));
                    } else {
                        ui.label(RichText::new("N/A").size(11.0).weak());
                    }
                });
            });
        }
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
                    ui.label(RichText::new(egui_phosphor::regular::CLOCK_COUNTER_CLOCKWISE.to_string()).size(36.0).color(Color32::from_rgb(37, 99, 235)));
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
                    ui.label(RichText::new("Nenhum dado histórico encontrado.").size(13.0).weak());
                });
                return;
            }

            let months = self.extract_months();
            let dark = ui.visuals().dark_mode;

            // Collect asset items for the left panel (avoid borrow conflict)
            let asset_items: Vec<(String, Color32, f64)> = self.monthly_series.iter().map(|s| {
                // Use the last month with non-zero data (CVM may not have published the current month)
                let latest = s.points.iter().rev().find(|(_, p)| *p > 0.0).map(|(_, p)| *p).unwrap_or(0.0);
                (s.label.clone(), s.color, latest)
            }).collect();

            let mut new_selected: Option<String> = None;
            let current_sel = self.selected_asset.clone();

            // ── Left panel: clickable asset list ──────────────────
            egui::Panel::left("hist_left_panel")
                .resizable(true)
                .default_size(230.0)
                .show_inside(ui, |ui| {
                    let panel_bg = if dark { Color32::from_rgb(18, 20, 28) } else { Color32::from_rgb(250, 251, 253) };
                    Frame::NONE.fill(panel_bg).show(ui, |ui| {
                        ui.add_space(6.0);
                        ui.label(RichText::new("ATIVOS").size(9.5).strong().color(
                            if dark { Color32::from_rgb(90, 105, 130) } else { Color32::from_rgb(150, 165, 185) }
                        ));
                        ui.add_space(4.0);

                        ScrollArea::vertical().show(ui, |ui| {
                            for (name, color, latest_pct) in &asset_items {
                                let is_sel = current_sel.as_deref() == Some(name.as_str());
                                let sel_bg = if dark { Color32::from_rgb(30, 42, 68) } else { Color32::from_rgb(219, 234, 254) };
                                let hov_bg = if dark { Color32::from_rgb(26, 30, 40) } else { Color32::from_rgb(240, 244, 252) };
                                let txt_color = if dark { Color32::from_rgb(210, 220, 235) } else { Color32::from_rgb(30, 40, 60) };
                                let pct_color = if *latest_pct > 0.0 { Color32::from_rgb(34, 197, 94) } else { Color32::GRAY };

                                let avail_w = ui.available_width();
                                let (rect, resp) = ui.allocate_exact_size(egui::vec2(avail_w, 30.0), Sense::click());
                                let bg = if is_sel { sel_bg } else if resp.hovered() { hov_bg } else { Color32::TRANSPARENT };
                                ui.painter().rect_filled(rect, 5.0, bg);

                                // Color dot
                                ui.painter().circle_filled(egui::pos2(rect.min.x + 12.0, rect.center().y), 4.0, *color);

                                // Name (truncated)
                                let display = if name.len() > 18 { format!("{}…", &name[..18]) } else { name.clone() };
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
                                    &format!("{:.1}%", latest_pct),
                                    egui::FontId::proportional(10.0),
                                    pct_color,
                                );

                                if resp.clicked() { new_selected = Some(name.clone()); }
                            }
                        });
                    });
                });

            // Apply actions after panel
            if let Some(name) = new_selected { self.selected_asset = Some(name); }

            // Right panel: chart + stats + table + yahoo
            ui.vertical(|ui| {
                if let Some(sel) = self.selected_asset.clone() {
                    // Auto-fetch Yahoo if eligible and not yet fetched
                    if let Some((codigo, tp_ativo, ap, vl_merc, vl_aquis)) = self.get_asset_info(&sel) {
                        if Self::is_yahoo_eligible(&tp_ativo)
                            && !self.yahoo_prices.contains_key(&codigo)
                            && !self.yahoo_loading
                            && self.last_yahoo_fetch.as_deref() != Some(&codigo)
                        {
                            self.yahoo_loading = true;
                            self.last_yahoo_fetch = Some(codigo.clone());
                            let end = chrono::Local::now().naive_local().date();
                            let start = end.checked_sub_months(chrono::Months::new(24))
                                .unwrap_or(end - chrono::Duration::days(730));
                            let _ = self.sender.send(Message::FetchYahooPrice(
                                codigo.clone(), self.cnpj.clone(), start, end,
                            ));
                        }

                        // Header
                        let dark = ui.visuals().dark_mode;
                        let tx = if dark { Color32::from_rgb(140, 155, 175) } else { Color32::from_rgb(90, 100, 120) };
                        let hd = if dark { Color32::WHITE } else { Color32::from_rgb(20, 30, 50) };

                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&codigo).size(14.0).strong().monospace().color(hd));
                            ui.label(RichText::new(&sel).size(13.0).color(hd));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(RichText::new(format!("{}m", months.len())).size(10.0).weak());
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

                        // Behavioral Analytics (RF07/RF08)
                        ui.separator();
                        ui.add_space(4.0);
                        ui.label(RichText::new("Análise Comportamental").size(11.0).strong());
                        ui.add_space(3.0);
                        self.render_analytics_inline(ui, &codigo);
                        ui.add_space(6.0);

                        // Yahoo section (only for eligible assets)
                        if Self::is_yahoo_eligible(&tp_ativo) {
                            ui.separator();
                            ui.add_space(4.0);
                            ui.label(RichText::new("Histórico de Preço — Yahoo Finance").size(11.0).strong());
                            ui.add_space(3.0);
                            self.render_yahoo_inline(ui, &codigo, vl_merc, vl_aquis);
                            ui.add_space(6.0);
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
                        ui.label(RichText::new(egui_phosphor::regular::CHART_LINE_UP.to_string()).size(40.0).color(Color32::from_rgb(100, 120, 160)));
                        ui.add_space(12.0);
                        ui.label(RichText::new("Selecione um ativo à esquerda").size(14.0).strong());
                        ui.add_space(4.0);
                        ui.label(RichText::new("para ver o histórico e as estatísticas").size(11.0).weak());
                    });
                }
            });
        });
    }
}
