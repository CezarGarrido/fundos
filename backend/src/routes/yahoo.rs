use axum::{Json, extract::Query, http::StatusCode};
use chrono::NaiveDate;
use fundos_common::types::{YahooPricePoint, YahooPriceResponse};
use serde::Deserialize;

use crate::providers::yahoo::YahooProvider;

#[derive(Deserialize)]
pub struct YahooParams {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

/// GET /api/prices/yahoo/{ticker}?start=...&end=...
pub async fn get_yahoo_prices(
    axum::extract::Path(ticker): axum::extract::Path<String>,
    Query(params): Query<YahooParams>,
) -> Result<Json<YahooPriceResponse>, StatusCode> {
    let provider = YahooProvider::new();
    let months_back = (params.end - params.start).num_days() / 30 + 1;

    match provider.get_monthly_quotes(&ticker, months_back).await {
        Ok(quotes) => {
            let prices: Vec<YahooPricePoint> = quotes
                .into_iter()
                .map(|q| {
                    let date = NaiveDate::parse_from_str(
                        &format!("{}-01", q.date), "%Y-%m-%d",
                    ).unwrap_or(NaiveDate::default());
                    YahooPricePoint {
                        date,
                        open: 0.0,
                        high: 0.0,
                        low: 0.0,
                        close: q.close_price,
                        adjclose: q.close_price,
                        volume: 0.0,
                    }
                })
                .collect();
            Ok(Json(YahooPriceResponse { ticker, prices }))
        }
        Err(e) => {
            log::error!("Yahoo price error for {}: {}", ticker, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
