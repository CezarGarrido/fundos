use polars::prelude::*;

pub struct AssetAnalytics {
    pub avg_buy_price: f64,
    pub avg_sell_price: f64,
    pub take_profit_trigger: f64,
    pub speed_to_peak: usize, // Months to build the position
    pub hidden_qty_estimates: Vec<(String, f64, f64)>, // Date, Qty, Value
}

/// Calculate behavioral analytics for a specific asset using the fund's historical DataFrame.
/// df must contain: CD_ATIVO (or CD_ISIN), QT_VENDA, VL_AQUIS_NEGOC, VL_VENDA_NEGOC, VL_PORCENTAGEM_PL, DT_COMPTC
pub fn compute_asset_analytics_v1(df: &DataFrame, asset_code: &str) -> Option<AssetAnalytics> {
    // Filter the dataframe for the specific asset and sort by date
    let filtered_df = df
        .clone()
        .lazy()
        .filter(
            col("CD_ISIN")
                .eq(lit(asset_code))
                .or(col("CD_ATIVO").eq(lit(asset_code)))
                .or(col("TP_ATIVO").eq(lit(asset_code))),
        )
        .sort(
            "DT_COMPTC",
            SortOptions {
                descending: false,
                ..Default::default()
            },
        )
        .collect()
        .ok()?;

    if filtered_df.height() == 0 {
        return None;
    }

    // Extract columns (some might be missing in history)
    let qt_venda_col = filtered_df.column("QT_VENDA").ok();
    let vl_aquis_col = filtered_df.column("VL_AQUIS_NEGOC").ok();
    let vl_venda_col = filtered_df.column("VL_VENDA_NEGOC").ok();
    // VL_PORCENTAGEM_PL must exist
    let pct_pl_col = filtered_df.column("VL_PORCENTAGEM_PL").ok()?;

    let mut max_pct_pl = 0.0;
    let mut months_to_peak = 0;
    let mut current_streak = 0;

    let mut take_profit = 0.0;
    let mut reached_peak = false;

    let height = filtered_df.height();
    for i in 0..height {
        // Read values safely
        let _qt = qt_venda_col.map(|c| get_f64(c, i)).unwrap_or(0.0);
        let _aq = vl_aquis_col.map(|c| get_f64(c, i)).unwrap_or(0.0);
        let _vd = vl_venda_col.map(|c| get_f64(c, i)).unwrap_or(0.0);
        let pct = get_f64(pct_pl_col, i);

        // Track speed and peaks
        if pct > max_pct_pl {
            max_pct_pl = pct;
            current_streak += 1;
        } else if pct < max_pct_pl && !reached_peak && current_streak > 0 {
            // Reached peak and started selling
            reached_peak = true;
            months_to_peak = current_streak;
            take_profit = max_pct_pl;
        }
    }

    // Since quantity deltas in CVM can be messy or zeroed out, we use a heuristic based on total volume
    // Or we just return the averages if possible.
    // As per user "Sinceramente não sei oque seria o correto, então, tanto faz", I'll provide a simplified version.

    // Average price = Value / Qty at the end? No, Average Buy Price is total_buy_value / total_buy_qty.
    // If we can't get reliable quantities, we just return the total buy value as a placeholder.

    Some(AssetAnalytics {
        avg_buy_price: 0.0, // Hard to calculate precisely without reliable qty
        avg_sell_price: 0.0,
        take_profit_trigger: take_profit,
        speed_to_peak: months_to_peak,
        hidden_qty_estimates: Vec::new(),
    })
}

// Se o CSV for o bruto da CVM, use essa abordagem baseada em saldo (Delta)
pub fn compute_asset_analytics(
    df: &DataFrame,
    asset_code: &str,
    quotes: Option<&[crate::provider::yahoo::MonthlyQuote]>,
) -> Option<AssetAnalytics> {
    let filtered_df = df
        .clone()
        .lazy()
        .filter(
            col("CD_ISIN")
                .eq(lit(asset_code))
                .or(col("CD_ATIVO").eq(lit(asset_code))),
        )
        .sort(
            "DT_COMPTC",
            SortOptions {
                descending: false,
                ..Default::default()
            },
        )
        .collect()
        .ok()?;

    if filtered_df.height() == 0 {
        return None;
    }

    // Usando as colunas oficiais da carteira mensal da CVM
    let qt_regis_col = filtered_df
        .column("QT_POS_FINAL")
        .ok()
        .or_else(|| filtered_df.column("QT_REGIS").ok())
        .or_else(|| filtered_df.column("QT_VENDA").ok())?;
    let pct_pl_col = filtered_df.column("VL_PORCENTAGEM_PL").ok()?;
    let vl_aquis_col = filtered_df.column("VL_AQUIS_NEGOC").ok();
    let vl_merc_col = filtered_df.column("VL_MERC_POS_FINAL").ok();

    let mut absolute_max_pct = 0.0;
    let mut index_of_peak = 0;
    let mut active_buying_months = 0;
    let mut months_counting_to_peak = 0;

    let height = filtered_df.height();
    let mut last_qty = 0.0;
    let mut last_aquis = 0.0;

    let mut total_buy_value = 0.0;
    let mut total_buy_qty = 0.0;
    let mut total_sell_value = 0.0;
    let mut total_sell_qty = 0.0;

    for i in 0..height {
        let current_qty = get_f64(qt_regis_col, i);
        let pct = get_f64(pct_pl_col, i);
        let current_aquis = vl_aquis_col.map(|c| get_f64(c, i)).unwrap_or(0.0);
        let current_merc = vl_merc_col.map(|c| get_f64(c, i)).unwrap_or(0.0);

        let est_price = if current_qty > 0.0 {
            current_merc / current_qty
        } else {
            0.0
        };

        if i == 0 {
            if current_qty > 0.0 {
                active_buying_months += 1;
                total_buy_qty += current_qty;
                if current_aquis > 0.0 {
                    total_buy_value += current_aquis;
                } else {
                    total_buy_value += current_qty * est_price;
                }
            }
        } else {
            let delta_qty = current_qty - last_qty;
            let delta_aquis = current_aquis - last_aquis;

            if delta_qty > 0.0 {
                // Compra (RF09)
                active_buying_months += 1;
                total_buy_qty += delta_qty;
                if delta_aquis > 0.0 {
                    total_buy_value += delta_aquis;
                } else {
                    total_buy_value += delta_qty * est_price;
                }
            } else if delta_qty < 0.0 {
                // Venda (RF10)
                total_sell_qty += delta_qty.abs();
                total_sell_value += delta_qty.abs() * est_price;
            }
        }

        if pct > absolute_max_pct {
            absolute_max_pct = pct;
            index_of_peak = i;
            months_counting_to_peak = active_buying_months;
        }

        last_qty = current_qty;
        last_aquis = current_aquis;
    }

    // Verificar realização de lucro (Venda) pós-pico
    let mut take_profit = 0.0;
    if index_of_peak < height - 1 {
        let peak_qty = get_f64(qt_regis_col, index_of_peak);
        let mut last_qty_post = peak_qty;

        for i in (index_of_peak + 1)..height {
            let current_qty = get_f64(qt_regis_col, i);

            // Se a quantidade de papéis caiu, ele realizou lucro
            if current_qty < last_qty_post {
                take_profit = absolute_max_pct;
                break;
            }
            last_qty_post = current_qty;
        }
    }

    let avg_buy_price = if total_buy_qty > 0.0 {
        total_buy_value / total_buy_qty
    } else {
        0.0
    };
    let avg_sell_price = if total_sell_qty > 0.0 {
        total_sell_value / total_sell_qty
    } else {
        0.0
    };

    let mut hidden_qty_estimates = Vec::new();

    // RF11: Extrapolar para os meses ocultos
    if let Some(quotes) = quotes {
        if let Ok(dt_col) = filtered_df.column("DT_COMPTC") {
            if let Some(last_known_dt) = dt_col
                .get(height - 1)
                .ok()
                .and_then(|v| v.get_str().map(|s| s.to_string()))
            {
                // Procurar cotações após a last_known_dt
                for q in quotes {
                    if q.date > last_known_dt && last_qty > 0.0 {
                        let est_value = last_qty * q.close_price;
                        hidden_qty_estimates.push((q.date.clone(), last_qty, est_value));
                    }
                }
            }
        }
    }

    Some(AssetAnalytics {
        avg_buy_price,
        avg_sell_price,
        take_profit_trigger: take_profit,
        speed_to_peak: months_counting_to_peak,
        hidden_qty_estimates,
    })
}

fn get_f64(col: &Series, row: usize) -> f64 {
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
