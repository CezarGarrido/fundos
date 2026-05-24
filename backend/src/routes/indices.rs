use axum::{Json, extract::Query, http::StatusCode};
use chrono::NaiveDate;
use fundos_common::types::{IndexSeriesResponse, TimeSeriesPoint};
use serde::Deserialize;

use crate::providers::indices;

#[derive(Deserialize)]
pub struct DateRangeParams {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

/// GET /api/indices/cdi?start=...&end=...
pub async fn get_cdi(
    Query(params): Query<DateRangeParams>,
) -> Result<Json<IndexSeriesResponse>, StatusCode> {
    match indices::cdi::async_dataframe(params.start, params.end).await {
        Ok(df) => {
            let points = df_to_time_series(&df, "as_date", "value");
            Ok(Json(IndexSeriesResponse { points }))
        }
        Err(e) => {
            log::error!("CDI error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/indices/ibovespa?start=...&end=...
pub async fn get_ibovespa(
    Query(params): Query<DateRangeParams>,
) -> Result<Json<IndexSeriesResponse>, StatusCode> {
    match indices::ibovespa::async_dataframe(params.start, params.end).await {
        Ok(df) => {
            let points = df_to_time_series(&df, "as_date", "value");
            Ok(Json(IndexSeriesResponse { points }))
        }
        Err(e) => {
            log::error!("IBOV error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Helper: extract a date column + f64 value column into Vec<TimeSeriesPoint>
fn df_to_time_series(df: &polars::frame::DataFrame, date_col: &str, value_col: &str) -> Vec<TimeSeriesPoint> {
    let mut points = Vec::new();
    let date_series = match df.column(date_col) { Ok(s) => s, Err(_) => return points };
    let value_series = match df.column(value_col) { Ok(s) => s, Err(_) => return points };
    for i in 0..df.height() {
        let date = date_series.get(i).ok()
            .and_then(|v| v.get_str().map(|s| s.to_string()))
            .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
        let value = value_series.get(i).ok()
            .and_then(|v| v.try_extract::<f64>().ok());
        if let (Some(d), Some(v)) = (date, value) {
            points.push(TimeSeriesPoint { date: d, value: v });
        }
    }
    points
}
