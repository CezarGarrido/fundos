use crate::ui::design::Scale;
use std::collections::HashMap;

use egui::{Align, Color32, Frame, Layout, RichText, ScrollArea, Sense, Ui, WidgetText};
use egui_extras::{Column, TableBuilder};
use egui_plot::{AxisHints, GridMark, Legend, Line, Plot};
use polars::prelude::*;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    message::Message,
    ui::{
        design::{Components, Typography},
        tabs::Tab,
    },
};

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

type MonthlyRow = (String, f64, f64, f64, f64); // (month, merc, pct, delta_qt, pm)

#[allow(dead_code)]
pub struct HistoricoTab {
    pub title: String,
    pub cnpj: String,
    pub sender: UnboundedSender<Message>,
    pub data: DataFrame,
    pub loading: bool,
    pub loading_status: String,
    pub monthly_series: Vec<MonthlySeries>,
    pub selected_asset: Option<String>,
    pub yahoo_prices: HashMap<String, DataFrame>,
    pub yahoo_loading: bool,
    pub last_yahoo_fetch: Option<String>,
    pub analytics_cache: HashMap<String, crate::analytics::AssetAnalytics>,
    cached_months: Vec<String>,
    cached_asset_info: Option<(String, String, String, f64, f64, f64)>,
    last_selected: Option<String>,
    cached_sparkline: Option<(String, Vec<[f64; 2]>, f64, f64)>,
    cached_monthly_table: HashMap<String, Vec<MonthlyRow>>,
    pub cancel_token: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub asset_task_token: std::sync::Arc<std::sync::atomic::AtomicBool>,
    selected_period_months: u32,
    streaming: bool,
}

#[derive(Clone)]
pub struct MonthlySeries {
    pub key: String,
    pub label: String,
    pub color: Color32,
    pub points: Vec<(f64, f64)>,
    pub tp_ativo: String,
}

impl Drop for HistoricoTab {
    fn drop(&mut self) {
        self.cancel_token
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.asset_task_token
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

impl HistoricoTab {
    pub fn new(cnpj: String, sender: UnboundedSender<Message>) -> Self {
        let mut tab = Self {
            title: format!("{} Histórico", cnpj),
            cnpj,
            sender,
            data: DataFrame::empty(),
            loading: true,
            loading_status: "Carregando dados históricos...".to_string(),
            monthly_series: vec![],
            selected_asset: None,
            yahoo_prices: HashMap::new(),
            yahoo_loading: false,
            last_yahoo_fetch: None,
            analytics_cache: HashMap::new(),
            cached_months: Vec::new(),
            cached_asset_info: None,
            last_selected: None,
            cached_sparkline: None,
            cached_monthly_table: HashMap::new(),
            cancel_token: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            asset_task_token: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            selected_period_months: 12,
            streaming: false,
        };
        tab.send_load_request(12);
        tab
    }

    fn send_load_request(&mut self, months: u32) {
        self.selected_period_months = months;
        self.loading_status = "Carregando dados históricos...".to_string();
        self.loading = true;
        self.streaming = false;
        self.data = DataFrame::empty();
        self.monthly_series.clear();
        self.selected_asset = None;
        self.last_selected = None;
        self.cached_sparkline = None;
        self.cached_monthly_table.clear();
        self.analytics_cache.clear();

        let end = chrono::Local::now().naive_local().date();
        let start = end
            .checked_sub_months(chrono::Months::new(months))
            .unwrap_or(end - chrono::Duration::days((months as i64) * 30));
        let _ = self
            .sender
            .send(Message::OpenHistoricoTab(self.cnpj.clone(), start, end));
    }

    pub fn set_data(&mut self, df: DataFrame) {
        // Mantém só colunas usadas (evita duplicar 50+ colunas em memória)
        let keep = [
            "DT_COMPTC",
            "CD_ATIVO",
            "CD_ISIN",
            "VL_MERC_POS_FINAL",
            "VL_PORCENTAGEM_PL",
            "VL_AQUIS_NEGOC",
            "QT_REGIS",
            "QT_POS_FINAL",
            "QT_VENDA",
            "TP_ATIVO",
            "TP_APLIC",
            "DS_ATIVO",
            "NM_FUNDO_COTA",
            "TP_TITPUB",
            "VL_PATRIM_LIQ",
            "CNPJ_FUNDO",
        ];
        let cols: Vec<&str> = keep
            .iter()
            .filter(|c| df.column(c).is_ok())
            .copied()
            .collect();
        self.data = df.select(cols.as_slice()).unwrap_or(df);
        self.analytics_cache.clear();
        self.cached_months.clear();
        self.cached_asset_info = None;
        self.last_selected = None;
        self.cached_monthly_table.clear();
        self.spawn_series_computation();
    }

    /// Appends newly loaded data to existing DataFrame (progressive loading).
    /// Triggers background series recomputation.
    pub fn append_data(&mut self, df: DataFrame) {
        if df.is_empty() {
            return;
        }
        if self.data.is_empty() {
            self.set_data(df);
            return;
        }
        // Concatenate and deduplicate
        let combined = self
            .data
            .vstack(&df)
            .unwrap_or_else(|_| self.data.clone());
        // Drop duplicate rows (same DT_COMPTC + CD_ATIVO + CD_ISIN)
        let deduped = combined
            .clone()
            .lazy()
            .unique(
                Some(vec![
                    "DT_COMPTC".to_string(),
                    "CD_ATIVO".to_string(),
                    "CD_ISIN".to_string(),
                ]),
                UniqueKeepStrategy::First,
            )
            .collect()
            .unwrap_or(combined);

        self.data = deduped;
        self.cached_months.clear();
        self.cached_asset_info = None;
        self.cached_monthly_table.clear();
        self.analytics_cache.clear();
        self.spawn_series_computation();
    }

    /// Spawns a background task to compute top series + tp_map and send the result.
    fn spawn_series_computation(&mut self) {
        self.loading_status = "Processando séries históricas...".to_string();
        let data = self.data.clone();
        let sender = self.sender.clone();
        let cnpj = self.cnpj.clone();
        self.cancel_token
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.cancel_token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cancel = self.cancel_token.clone();

        tokio::spawn(async move {
            let raw_series = crate::analytics::compute_top_series(&data, Some(cancel));

            let mut tp_map = std::collections::HashMap::new();
            let tp_result = data
                .clone()
                .lazy()
                .with_column(col("CD_ATIVO").fill_null(lit("")))
                .with_column(col("CD_ISIN").fill_null(lit("")))
                .with_column(
                    when(col("CD_ATIVO").neq(lit("")))
                        .then(col("CD_ATIVO"))
                        .otherwise(col("CD_ISIN"))
                        .alias("asset_key"),
                )
                .filter(col("asset_key").neq(lit("")))
                .select([col("asset_key"), col("TP_ATIVO")])
                .unique(Some(vec!["asset_key".to_string()]), UniqueKeepStrategy::First)
                .collect();
            if let Ok(tp_df) = tp_result {
                if let (Ok(ak), Ok(tc)) = (tp_df.column("asset_key"), tp_df.column("TP_ATIVO")) {
                    for i in 0..tp_df.height() {
                        tp_map.insert(get_str(ak, i), get_str(tc, i));
                    }
                }
            }

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
                .map(|(i, (key, label, points))| {
                    let tp = tp_map
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| "OUTROS".to_string());
                    MonthlySeries {
                        key,
                        label,
                        color: palette[i % palette.len()],
                        points,
                        tp_ativo: tp,
                    }
                })
                .collect();
            let _ = sender.send(Message::HistoricoSeriesResult(cnpj, series));
        });
    }

    fn get_months(&mut self) -> Vec<String> {
        if self.cached_months.is_empty() {
            self.cached_months = self.extract_months();
        }
        self.cached_months.clone()
    }

    fn extract_months(&self) -> Vec<String> {
        if self.data.is_empty() {
            return vec![];
        }
        let result = self
            .data
            .clone()
            .lazy()
            .select([col("DT_COMPTC")
                .str()
                .str_slice(0, Some(7))
                .alias("month")])
            .unique(None, UniqueKeepStrategy::Any)
            .sort("month", SortOptions::default())
            .collect();
        match result {
            Ok(df) => {
                let c = df.column("month").unwrap();
                let mut months: Vec<String> = Vec::with_capacity(df.height());
                for i in 0..df.height() {
                    months.push(get_str(c, i));
                }
                months
            }
            Err(_) => vec![],
        }
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

        let items = [
            (
                "Mínimo",
                format!("{:.2}%", min_p),
                Color32::from_rgb(239, 68, 68),
            ),
            (
                "Médio",
                format!("{:.2}%", avg_p),
                Color32::from_rgb(59, 130, 246),
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
                Components::card(&mut cols[i], dark, 6, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(Typography::small_muted(*label, dark));
                    ui.label(RichText::new(value).size(Scale::DEFAULT.body()).strong().color(*color));
                });
            }
        });
    }

    /// Aggregates monthly metrics for a single asset using a single Polars groupby
    /// (O(n log n) instead of O(n × months)).
    fn compute_monthly_data(
        data: &DataFrame,
        months: &[String],
        asset_key: &str,
    ) -> Vec<MonthlyRow> {
        if data.is_empty() || months.is_empty() {
            return vec![];
        }
        // Pass 1: Polars groupby → HashMap<month, (merc, pct, qt, aquis)>
        let mut aggs: Vec<Expr> = Vec::with_capacity(4);
        let has_merc = data.column("VL_MERC_POS_FINAL").is_ok();
        let has_pct = data.column("VL_PORCENTAGEM_PL").is_ok();
        let has_aquis = data.column("VL_AQUIS_NEGOC").is_ok();
        let (qt_col_name, has_qt) = if data.column("QT_POS_FINAL").is_ok() {
            ("QT_POS_FINAL", true)
        } else if data.column("QT_REGIS").is_ok() {
            ("QT_REGIS", true)
        } else {
            ("QT_REGIS", false)
        };

        if has_merc {
            aggs.push(col("VL_MERC_POS_FINAL").cast(DataType::Float64).sum().alias("total_merc"));
        }
        if has_pct {
            aggs.push(col("VL_PORCENTAGEM_PL").cast(DataType::Float64).sum().alias("total_pct"));
        }
        if has_qt {
            aggs.push(col(qt_col_name).cast(DataType::Float64).sum().alias("total_qt"));
        }
        if has_aquis {
            aggs.push(col("VL_AQUIS_NEGOC").cast(DataType::Float64).sum().alias("total_aquis"));
        }

        let mut monthly_map: std::collections::HashMap<String, (f64, f64, f64, f64)> =
            std::collections::HashMap::new();

        if !aggs.is_empty() {
            let result = data
                .clone()
                .lazy()
                .with_column(col("CD_ATIVO").fill_null(lit("")))
                .with_column(col("CD_ISIN").fill_null(lit("")))
                .with_column(col("DT_COMPTC").fill_null(lit("")))
                .with_column(
                    when(col("CD_ATIVO").neq(lit("")))
                        .then(col("CD_ATIVO"))
                        .otherwise(col("CD_ISIN"))
                        .alias("asset_key"),
                )
                .with_column(col("DT_COMPTC").str().str_slice(0, Some(7)).alias("month"))
                .filter(col("asset_key").eq(lit(asset_key)))
                .groupby([col("month")])
                .agg(aggs)
                .collect();

            let aggregated = match result {
                Ok(df) => df,
                Err(e) => {
                    log::error!("compute_monthly_data falhou para '{}': {:?}", asset_key, e);
                    return vec![];
                }
            };

            let month_col = aggregated.column("month").ok();
            let merc_col = aggregated.column("total_merc").ok();
            let pct_col = aggregated.column("total_pct").ok();
            let qt_col = aggregated.column("total_qt").ok();
            let aquis_col = aggregated.column("total_aquis").ok();

            for i in 0..aggregated.height() {
                let m = month_col.as_ref().map(|c| get_str(c, i)).unwrap_or_default();
                if m.is_empty() {
                    continue;
                }
                let merc = merc_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0);
                let pct = pct_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0);
                let qt = qt_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0);
                let aquis = aquis_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0);
                monthly_map.insert(m, (merc, pct, qt, aquis));
            }
        }

        // Pass 2: walk all months; fill zeros for months where the asset is absent
        let mut month_data: Vec<MonthlyRow> = Vec::with_capacity(months.len());
        let mut last_qt = 0.0;

        for m in months.iter() {
            let (merc, pct, qt, aquis) = monthly_map
                .remove(m.as_str())
                .unwrap_or((0.0, 0.0, 0.0, 0.0));

            let delta_qt = qt - last_qt;
            let est_price = if qt > 0.0 { merc / qt } else { 0.0 };

            let mut pm_transacao = 0.0;
            if delta_qt > 0.0 {
                pm_transacao = if aquis > 0.0 {
                    aquis / delta_qt
                } else {
                    est_price
                };
            } else if delta_qt < 0.0 {
                pm_transacao = est_price;
            }

            month_data.push((m.clone(), merc, pct, delta_qt, pm_transacao));
            last_qt = qt;
        }

        month_data.reverse();
        month_data
    }

    fn render_asset_monthly_table(&mut self, ui: &mut Ui, months: &[String], selected_name: &str) {
        let asset_key = selected_name.to_string();

        // Cache: O(1) lookup into precomputed monthly aggregations
        let month_data = if let Some(cached) = self.cached_monthly_table.get(&asset_key) {
            cached.clone()
        } else {
            let data = Self::compute_monthly_data(&self.data, months, &asset_key);
            self.cached_monthly_table
                .insert(asset_key.clone(), data.clone());
            data
        };

        let dark = ui.visuals().dark_mode;

        Components::configure_table(TableBuilder::new(ui))
            .column(Column::exact(60.0)) // Mês
            .column(Column::initial(70.0)) // %PL
            .column(Column::initial(90.0)) // Valor
            .column(Column::initial(85.0)) // Delta Qtde
            .column(Column::remainder()) // PM Transação
            .header(Scale::DEFAULT.table_header_height(), |mut h| {
                for label in &["Mês", "%PL", "Valor Merc.", "Mov. Qtde", "PM Transação"] {
                    h.col(|ui| {
                        ui.label(Typography::label_strong(*label, dark));
                    });
                }
            })
            .body(|body| {
                body.rows(Scale::DEFAULT.table_row_height(), month_data.len(), |mut row| {
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
                        ui.label(RichText::new(&m_label).size(Scale::DEFAULT.table_cell()).monospace());
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
                            .size(Scale::DEFAULT.table_cell())
                            .color(c),
                        );
                    });
                    row.col(|ui| {
                        ui.label(RichText::new(&fmt_merc).size(Scale::DEFAULT.table_cell()));
                    });
                    row.col(|ui| {
                        if *delta_qt != 0.0 {
                            ui.label(
                                RichText::new(format!("{}{:.0}", qt_prefix, delta_qt))
                                    .size(Scale::DEFAULT.table_cell())
                                    .color(qt_color),
                            );
                        } else {
                            ui.label(RichText::new("-").size(Scale::DEFAULT.table_cell()).weak());
                        }
                    });
                    row.col(|ui| {
                        if *delta_qt != 0.0 && *pm > 0.0 {
                            ui.label(
                                RichText::new(format!("R$ {:.2}", pm))
                                    .size(Scale::DEFAULT.table_cell())
                                    .strong()
                                    .color(qt_color),
                            );
                        } else {
                            ui.label(RichText::new("-").size(Scale::DEFAULT.table_cell()).weak());
                        }
                    });
                });
            });
    }

    /// Extract first-row info for the selected asset using Polars filter (O(n log n) vs O(n))
    fn get_asset_info(
        &self,
        selected_name: &str,
    ) -> Option<(String, String, String, f64, f64, f64)> {
        if self.data.is_empty() {
            return None;
        }
        // Build select list dynamically — only include columns that exist
        let mut select_cols: Vec<Expr> = vec![
            col("asset_key"),
            col("TP_ATIVO"),
            col("TP_APLIC"),
            col("VL_MERC_POS_FINAL"),
            col("VL_AQUIS_NEGOC"),
        ];
        let has_qt_pos = self.data.column("QT_POS_FINAL").is_ok();
        let has_qt_reg = self.data.column("QT_REGIS").is_ok();
        if has_qt_pos {
            select_cols.push(col("QT_POS_FINAL"));
        }
        if has_qt_reg {
            select_cols.push(col("QT_REGIS"));
        }

        let result = self
            .data
            .clone()
            .lazy()
            .with_column(col("CD_ATIVO").fill_null(lit("")))
            .with_column(col("CD_ISIN").fill_null(lit("")))
            .with_column(
                when(col("CD_ATIVO").neq(lit("")))
                    .then(col("CD_ATIVO"))
                    .otherwise(col("CD_ISIN"))
                    .alias("asset_key"),
            )
            .filter(col("asset_key").eq(lit(selected_name)))
            .select(select_cols)
            .limit(1)
            .collect();

        let df = match result {
            Ok(df) => df,
            Err(_) => return None,
        };
        if df.height() == 0 {
            return None;
        }

        let key = df.column("asset_key").map(|c| get_str(c, 0)).unwrap_or_default();
        let tp = df.column("TP_ATIVO").map(|c| get_str(c, 0)).unwrap_or_default();
        let ap = df.column("TP_APLIC").map(|c| get_str(c, 0)).unwrap_or_default();
        let merc = df.column("VL_MERC_POS_FINAL").map(|c| get_f64(c, 0)).unwrap_or(0.0);
        let aquis = df.column("VL_AQUIS_NEGOC").map(|c| get_f64(c, 0)).unwrap_or(0.0);
        let qt = df
            .column("QT_POS_FINAL")
            .ok()
            .or_else(|| df.column("QT_REGIS").ok())
            .map(|c| get_f64(c, 0))
            .unwrap_or(0.0);

        Some((key, tp, ap, merc, aquis, qt))
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
        if self.yahoo_loading {
            crate::ui::loading::show_custom_small(ui, "Buscando Yahoo Finance...");
            return;
        }
        let prices = match self.yahoo_prices.get(codigo) {
            Some(p) => p,
            None => {
                ui.label(RichText::new("Preços não disponíveis.").size(Scale::DEFAULT.badge()).weak());
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
                    .size(Scale::DEFAULT.heading_3())
                    .strong(),
            );
            ui.label(
                RichText::new(format!("({:+.2}%)", change))
                    .size(Scale::DEFAULT.small_text())
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
                Color32::from_rgb(255, 255, 255),
                Color32::from_rgb(240, 240, 240),
                Color32::from_rgb(200, 200, 200),
                Color32::from_rgb(16, 20, 30),
            )
        } else {
            (
                Color32::from_rgb(0, 0, 0),
                Color32::from_rgb(10, 10, 10),
                Color32::from_rgb(30, 30, 30),
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

            self.asset_task_token
                .store(true, std::sync::atomic::Ordering::Relaxed);
            self.asset_task_token = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let cancel = self.asset_task_token.clone();

            tokio::spawn(async move {
                let yahoo_ref = yahoo_df.as_ref();
                if let Some(a) = crate::analytics::compute_asset_analytics(
                    &data,
                    &codigo_owned,
                    yahoo_ref,
                    Some(cancel),
                ) {
                    let _ = sender.send(Message::AssetAnalyticsResult(codigo_owned, a));
                }
            });
        }

        // Cache get_asset_info (O(n) — só recalcula quando ativo muda)
        if self.last_selected.as_deref() != Some(codigo) {
            self.cached_asset_info =
                self.get_asset_info(&self.selected_asset.clone().unwrap_or_default());
            self.last_selected = Some(codigo.to_string());
        }
        let yahoo_data: Option<(&DataFrame, f64, f64, f64)> =
            if let Some((_, _, _, vl_merc, vl_aquis, qt_pos)) = &self.cached_asset_info {
                self.yahoo_prices
                    .get(codigo)
                    .map(|prices| (prices, *vl_merc, *vl_aquis, *qt_pos))
            } else {
                None
            };

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
                        ui.label(RichText::new(price_str).size(Scale::DEFAULT.heading_1()).strong().color(hd));
                        if change != 0.0 {
                            let icon = if change > 0.0 {
                                egui_phosphor::regular::TREND_UP
                            } else {
                                egui_phosphor::regular::TREND_DOWN
                            };
                            ui.label(
                                RichText::new(format!("  {} {:.2}%", icon, change.abs()))
                                    .size(Scale::DEFAULT.body())
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
                                .size(Scale::DEFAULT.button())
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
                                    .size(Scale::DEFAULT.button())
                                    .color(muted),
                                );
                                ui.label(RichText::new(pm_str).size(Scale::DEFAULT.body()).strong().color(accent));
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
                                            .size(Scale::DEFAULT.small_text())
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
                            .size(Scale::DEFAULT.label())
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
                                .size(Scale::DEFAULT.small_text())
                                .strong()
                                .color(hd),
                            );
                            let val_fmt = crate::util::to_real(last_est.2)
                                .map(|r| r.format())
                                .unwrap_or_else(|_| format!("R$ {:.2}", last_est.2));
                            ui.label(RichText::new(format!("≈ {}", val_fmt)).size(Scale::DEFAULT.label()).color(tx));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if let Some(ref inf) = analytics.portfolio_inference {
                                    // Badge 2: Direção do viés (sempre visível)
                                    let (dir_color, dir_bg, dir_icon, dir_label) =
                                        if inf.z_score > 2.0 {
                                            (
                                                green,
                                                if dark {
                                                    Color32::from_rgb(20, 50, 30)
                                                } else {
                                                    Color32::from_rgb(220, 245, 225)
                                                },
                                                egui_phosphor::regular::ARROW_UP,
                                                "Comprando",
                                            )
                                        } else if inf.z_score < -2.0 {
                                            (
                                                red,
                                                if dark {
                                                    Color32::from_rgb(55, 20, 25)
                                                } else {
                                                    Color32::from_rgb(255, 225, 225)
                                                },
                                                egui_phosphor::regular::ARROW_DOWN,
                                                "Vendendo",
                                            )
                                        } else {
                                            (
                                                tx,
                                                Color32::TRANSPARENT,
                                                egui_phosphor::regular::EQUALS,
                                                "Neutro",
                                            )
                                        };
                                    let dir_text =
                                        format!("{} Z={:+.1} {}", dir_icon, inf.z_score, dir_label);
                                    Components::badge(ui, &dir_text, dir_color, dir_bg);
                                    ui.add_space(4.0);
                                    // Badge 1: Estabilidade
                                    let (stab_color, stab_bg) = if inf.stability_score > 0.75 {
                                        (
                                            green,
                                            if dark {
                                                Color32::from_rgb(20, 50, 30)
                                            } else {
                                                Color32::from_rgb(220, 245, 225)
                                            },
                                        )
                                    } else if inf.stability_score > 0.40 {
                                        (
                                            Color32::from_rgb(234, 179, 8),
                                            if dark {
                                                Color32::from_rgb(50, 40, 10)
                                            } else {
                                                Color32::from_rgb(255, 248, 220)
                                            },
                                        )
                                    } else if inf.stability_score > 0.15 {
                                        (
                                            Color32::from_rgb(249, 115, 22),
                                            if dark {
                                                Color32::from_rgb(50, 25, 10)
                                            } else {
                                                Color32::from_rgb(255, 235, 215)
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
                                    let label = if inf.stability_score > 0.75 {
                                        "Previsível"
                                    } else if inf.stability_score > 0.40 {
                                        "Estável"
                                    } else if inf.stability_score > 0.15 {
                                        "Instável"
                                    } else {
                                        "Caótico"
                                    };
                                    let stab_text = format!(
                                        "{} {} ({:.0}%)",
                                        egui_phosphor::regular::BRAIN,
                                        label,
                                        inf.stability_score * 100.0
                                    );
                                    Components::badge(ui, &stab_text, stab_color, stab_bg);
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
                        .size(Scale::DEFAULT.label())
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
                                .size(Scale::DEFAULT.label_mini())
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
                                        .size(Scale::DEFAULT.button())
                                        .strong()
                                        .color(speed_color),
                                );
                            } else {
                                ui.label(RichText::new("—").size(Scale::DEFAULT.button()).weak());
                            }
                        });
                        cols[1].vertical(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} Take-Profit",
                                    egui_phosphor::regular::FLAG_BANNER
                                ))
                                .size(Scale::DEFAULT.label_mini())
                                .color(muted),
                            );
                            if analytics.take_profit_trigger > 0.0 {
                                ui.label(
                                    RichText::new(format!(
                                        "{:.1}% PL",
                                        analytics.take_profit_trigger
                                    ))
                                    .size(Scale::DEFAULT.button())
                                    .strong()
                                    .color(green),
                                );
                            } else {
                                ui.label(RichText::new("—").size(Scale::DEFAULT.button()).weak());
                            }
                        });
                        cols[2].vertical(|ui| {
                            ui.label(
                                RichText::new(format!(
                                    "{} Ganho Oculto",
                                    egui_phosphor::regular::MAGNIFYING_GLASS
                                ))
                                .size(Scale::DEFAULT.label_mini())
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
                                        .size(Scale::DEFAULT.button())
                                        .strong()
                                        .color(gain_color),
                                );
                            } else {
                                ui.label(RichText::new("—").size(Scale::DEFAULT.button()).weak());
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
            // -- Period Selection Bar --
            ui.horizontal(|ui| {
                ui.label(RichText::new("Período Histórico:").strong());

                let periods = [
                    (3, "3 Meses"),
                    (6, "6 Meses"),
                    (12, "1 Ano"),
                    (24, "2 Anos"),
                    (60, "5 Anos"),
                    (120, "10 Anos"),
                ];

                let mut changed_period = None;
                for (months, label) in periods {
                    let is_selected = self.selected_period_months == months;
                    let response = if is_selected {
                        ui.add(
                            egui::Button::new(
                                RichText::new(label)
                                    .strong()
                                    .color(ui.visuals().window_fill),
                            )
                            .fill(ui.visuals().text_color()),
                        )
                    } else {
                        ui.button(label)
                    };

                    if response.clicked() {
                        changed_period = Some(months);
                    }
                }

                if let Some(m) = changed_period {
                    self.send_load_request(m);
                }
            });
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            if self.loading {
                crate::ui::loading::show(ui, &self.loading_status);
                return;
            }
            if self.data.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new("Nenhum dado histórico encontrado.")
                            .size(Scale::DEFAULT.button())
                            .weak(),
                    );
                });
                return;
            }

            // Streaming indicator — shows while more data batches arrive in background
            if !self.loading_status.is_empty() {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(&self.loading_status)
                            .size(Scale::DEFAULT.badge())
                            .weak(),
                    );
                });
                ui.add_space(4.0);
                ui.separator();
                ui.add_space(4.0);
            }

            let months = self.get_months();
            let dark = ui.visuals().dark_mode;

            // Collect asset items for the left panel (avoid borrow conflict)
            let mut asset_items: Vec<(String, String, String, Color32, f64)> = self
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
                    let t = s.tp_ativo.trim().to_uppercase();
                    let tp = if t.is_empty() {
                        "OUTROS".to_string()
                    } else {
                        t
                    };
                    (tp, s.key.clone(), s.label.clone(), s.color, latest)
                })
                .collect();

            fn type_priority(tp: &str) -> i32 {
                if tp.contains("AÇÃO") || tp.contains("AÇÕES") {
                    1
                } else if tp == "BDR" {
                    2
                } else if tp.contains("ETF") || tp.contains("ÍNDICE") {
                    3
                } else if tp == "FII" || tp.contains("IMOBILIÁRIO") {
                    4
                } else if tp == "OUTROS" {
                    99
                } else {
                    50
                }
            }

            // Sort by type priority ascending, then type name ascending, then alphabetically by ticker
            asset_items.sort_by(|a, b| {
                let pa = type_priority(&a.0);
                let pb = type_priority(&b.0);
                match pa.cmp(&pb) {
                    std::cmp::Ordering::Equal => match a.0.cmp(&b.0) {
                        std::cmp::Ordering::Equal => a.1.cmp(&b.1),
                        other => other,
                    },
                    other => other,
                }
            });

            let mut new_selected: Option<String> = None;
            let current_sel = self.selected_asset.clone();

            // ── Left panel: clickable asset list ──────────────────
            egui::Panel::left("hist_left_panel")
                .resizable(true)
                .default_size(300.0)
                .show_inside(ui, |ui| {
                    let panel_bg = if dark {
                        Color32::from_rgb(18, 20, 28)
                    } else {
                        Color32::from_rgb(250, 251, 253)
                    };
                    Frame::NONE.fill(panel_bg).show(ui, |ui| {
                        ui.add_space(6.0);

                        ScrollArea::vertical().show(ui, |ui| {
                            let mut current_group = String::new();

                            for (tp, key, name, color, latest_pct) in &asset_items {
                                if *tp != current_group {
                                    if !current_group.is_empty() {
                                        ui.add_space(8.0);
                                    }
                                    ui.label(RichText::new(tp).size(Scale::DEFAULT.small_text()).strong().color(
                                        if dark {
                                            Color32::from_rgb(160, 175, 195)
                                        } else {
                                            Color32::from_rgb(90, 105, 125)
                                        },
                                    ));
                                    ui.add_space(4.0);
                                    current_group = tp.clone();
                                }

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
                                    Color32::from_rgb(240, 245, 255)
                                } else {
                                    Color32::from_rgb(20, 30, 45)
                                };
                                let pct_color = if *latest_pct > 0.0 {
                                    Color32::from_rgb(34, 197, 94)
                                } else {
                                    Color32::GRAY
                                };

                                let avail_w = ui.available_width();
                                let (rect, resp) = ui
                                    .allocate_exact_size(egui::vec2(avail_w, 34.0), Sense::click());
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
                                    egui::FontId::proportional(12.5),
                                    txt_color,
                                );

                                // %PL right-aligned
                                ui.painter().text(
                                    egui::pos2(rect.max.x - 8.0, rect.center().y),
                                    egui::Align2::RIGHT_CENTER,
                                    format!("{:.1}%", latest_pct),
                                    egui::FontId::proportional(11.5),
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
            ScrollArea::vertical()
                .id_salt("historico_detail_scroll")
                .show(ui, |ui| {
                Frame::NONE
                    .inner_margin(egui::vec2(12.0, 8.0))
                    .show(ui, |ui| {
                if let Some(sel) = self.selected_asset.clone() {
                    // Use cached asset info to avoid Polars filter on every frame,
                    // but refresh when the selected asset changes
                    let cache_stale = self
                        .cached_asset_info
                        .as_ref()
                        .map(|(key, _, _, _, _, _)| key != &sel)
                        .unwrap_or(true);
                    if cache_stale {
                        self.cached_asset_info = self.get_asset_info(&sel);
                    }
                    if let Some((codigo, tp_ativo, ap, _vl_merc, _vl_aquis, _qt_pos)) =
                        self.cached_asset_info.clone()
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
                                    .size(Scale::DEFAULT.body())
                                    .strong()
                                    .monospace()
                                    .color(hd),
                            );
                            ui.label(RichText::new(&sel).size(Scale::DEFAULT.button()).color(hd));
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                ui.label(
                                    RichText::new(format!("{}m", months.len()))
                                        .size(Scale::DEFAULT.badge())
                                        .weak(),
                                );
                                if !ap.is_empty() {
                                    ui.label(RichText::new(&ap).size(Scale::DEFAULT.badge()).color(tx));
                                }
                                if !tp_ativo.is_empty() {
                                    ui.label(RichText::new(&tp_ativo).size(Scale::DEFAULT.badge()).color(tx));
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
                        ui.label(RichText::new("Detalhamento Mensal").size(Scale::DEFAULT.label()).strong());
                        ui.add_space(3.0);
                        self.render_asset_monthly_table(ui, &months, &sel);
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(80.0);
                        ui.label(
                            RichText::new(egui_phosphor::regular::CHART_LINE_UP.to_string())
                                .size(Scale::ICON_HERO)
                                .color(Color32::from_rgb(100, 120, 160)),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("Selecione um ativo à esquerda")
                                .size(Scale::DEFAULT.body())
                                .strong(),
                        );
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("para ver o histórico e as estatísticas")
                                .size(Scale::DEFAULT.label())
                                .weak(),
                        );
                    });
                }
            });
            });
        });
    }
}
