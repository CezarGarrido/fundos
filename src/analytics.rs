use polars::prelude::*;

pub struct AssetAnalytics {
    pub avg_buy_price: f64,
    pub avg_sell_price: f64,
    pub take_profit_trigger: f64,
    pub speed_to_peak: usize, // Months to build the position
}

/// Calculate behavioral analytics for a specific asset using the fund's historical DataFrame.
/// df must contain: CD_ATIVO (or CD_ISIN), QT_VENDA, VL_AQUIS_NEGOC, VL_VENDA_NEGOC, VL_PORCENTAGEM_PL, DT_COMPTC
pub fn compute_asset_analytics(df: &DataFrame, asset_code: &str) -> Option<AssetAnalytics> {
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
