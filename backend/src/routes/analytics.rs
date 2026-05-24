use axum::{Json, extract::Query, http::StatusCode};
use chrono::NaiveDate;
use fundos_common::types::{
    AssetAnalyticsResponse, ForwardTrajectoryPoint, MonthlySeries,
};
use serde::Deserialize;

use crate::{
    providers::cvm::portfolio::Portfolio,
    services::analytics::compute_asset_analytics,
};

#[derive(Deserialize)]
pub struct AnalyticsParams {
    pub fund_cnpj: Option<String>,
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
}

/// GET /api/assets/{id}/analytics?fund_cnpj=...&start=...&end=...
pub async fn get_asset_analytics(
    axum::extract::Path(asset_id): axum::extract::Path<String>,
    Query(params): Query<AnalyticsParams>,
) -> Result<Json<AssetAnalyticsResponse>, StatusCode> {
    let portfolio = Portfolio::new();
    let end = params.end.unwrap_or_else(|| chrono::Local::now().naive_local().date());
    let start = params.start.unwrap_or(end - chrono::Duration::days(365 * 2));

    // Try specific fund CNPJ if provided, otherwise search broadly
    let cnpj = crate::cnpj::normalize(&params.fund_cnpj.unwrap_or_default());

    let df = if cnpj.is_empty() {
        // Broad market search for this asset
        portfolio.async_historical_assets(String::new(), start, end).await
    } else {
        portfolio.async_historical_assets(cnpj.clone(), start, end).await
    };

    let df = match df {
        Ok(d) => d,
        Err(e) => {
            log::error!("Analytics data fetch error for {}: {}", asset_id, e);
            return Err(StatusCode::NOT_FOUND);
        }
    };

    match compute_asset_analytics(&df, &asset_id, None, None) {
        Some(analytics) => Ok(Json(analytics_to_response(&asset_id, &analytics))),
        None => {
            log::warn!("Analytics computation returned None for {}", asset_id);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

fn analytics_to_response(asset_id: &str, a: &crate::services::analytics::AssetAnalytics) -> AssetAnalyticsResponse {
    let historical_states: Vec<MonthlySeries> = a.portfolio_inference.as_ref()
        .map(|pi| pi.historical_states.iter().map(|(date, real, filtered, lower, upper)| {
            let idx = pi.historical_states.iter().position(|(d, _, _, _, _)| d == date).unwrap_or(0);
            let z = pi.historical_z_scores.get(idx).copied().unwrap_or(0.0);
            MonthlySeries {
                date: date.clone(),
                real_qty: *real,
                filtered_qty: *filtered,
                ci_lower: *lower,
                ci_upper: *upper,
                z_score: z,
            }
        }).collect())
        .unwrap_or_default();

    let forward_trajectory: Vec<ForwardTrajectoryPoint> = a.portfolio_inference.as_ref()
        .map(|pi| pi.forward_trajectory.iter().map(|(date, qty, lower, upper)| {
            ForwardTrajectoryPoint {
                date: date.clone(),
                estimated_qty: *qty,
                ci_lower: *lower,
                ci_upper: *upper,
            }
        }).collect())
        .unwrap_or_default();

    let pi = a.portfolio_inference.as_ref();

    AssetAnalyticsResponse {
        asset_id: asset_id.to_string(),
        avg_buy_price: a.avg_buy_price,
        avg_sell_price: a.avg_sell_price,
        take_profit_trigger: a.take_profit_trigger,
        speed_to_peak: a.speed_to_peak,
        hidden_qty_estimates: a.hidden_qty_estimates.clone(),
        bias_direction: pi.map(|p| p.bias_direction.to_string()).unwrap_or_default(),
        stability_score: pi.map(|p| p.stability_score).unwrap_or(0.0),
        z_score: pi.map(|p| p.z_score).unwrap_or(0.0),
        baseline_qty: pi.map(|p| p.baseline_qty).unwrap_or(0.0),
        filtered_qty: pi.map(|p| p.filtered_qty).unwrap_or(0.0),
        historical_states,
        forward_trajectory,
    }
}
