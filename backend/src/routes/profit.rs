use axum::{Json, extract::Query, http::StatusCode};
use chrono::NaiveDate;
use fundos_common::types::{ProfitResponse, TimeSeriesPoint};
use serde::Deserialize;

use crate::providers::{cvm::informe::Informe, indices};

#[derive(Deserialize)]
pub struct ProfitParams {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

/// GET /api/funds/{cnpj}/profit?start=...&end=...
pub async fn get_profit(
    axum::extract::Path(cnpj): axum::extract::Path<String>,
    Query(params): Query<ProfitParams>,
) -> Result<Json<ProfitResponse>, StatusCode> {
    let cnpj = crate::cnpj::normalize(&cnpj);
    let informe = Informe::new();

    let profit_fut = informe.async_profit(cnpj.clone(), params.start, params.end);
    let cdi_fut = indices::cdi::async_dataframe(params.start, params.end);
    let ibov_fut = indices::ibovespa::async_dataframe(params.start, params.end);

    let (profit_res, cdi_res, ibov_res) = tokio::join!(profit_fut, cdi_fut, ibov_fut);

    let fund_series = match profit_res {
        Ok(df) => df_to_profit_series(&df),
        Err(e) => {
            log::error!("async_profit failed: {:?}", e);
            vec![]
        }
    };
    let cdi_series = cdi_res
        .map(|df| df_to_time_series(&df, "as_date", "value"))
        .unwrap_or_default();
    let ibov_series = ibov_res
        .map(|df| df_to_time_series(&df, "AS_DATE", "value"))
        .unwrap_or_default();

    Ok(Json(ProfitResponse {
        cnpj,
        fund_series,
        cdi_series,
        ibov_series,
    }))
}

fn df_to_time_series(df: &polars::frame::DataFrame, date_col: &str, value_col: &str) -> Vec<TimeSeriesPoint> {
    let mut points = Vec::new();
    
    let date_s = match df.column(date_col) { Ok(s) => s, Err(_) => return points };
    let value_s = match df.column(value_col) { Ok(s) => s, Err(_) => return points };

    let date_s_casted = match date_s.cast(&polars::datatypes::DataType::Date) { Ok(s) => s, Err(_) => return points };
    let value_s_casted = match value_s.cast(&polars::datatypes::DataType::Float64) { Ok(s) => s, Err(_) => return points };

    let date_chunk = match date_s_casted.date() { Ok(c) => c, Err(_) => return points };
    let value_chunk = match value_s_casted.f64() { Ok(c) => c, Err(_) => return points };

    let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    
    for (d_opt, v_opt) in date_chunk.into_iter().zip(value_chunk.into_iter()) {
        if let (Some(days), Some(v)) = (d_opt, v_opt) {
            if let Some(d) = epoch.checked_add_signed(chrono::Duration::days(days as i64)) {
                points.push(TimeSeriesPoint { date: d, value: v });
            }
        }
    }
    
    points
}

fn df_to_profit_series(df: &polars::frame::DataFrame) -> Vec<TimeSeriesPoint> {
    df_to_time_series(df, "AS_DATE", "RENT_ACUM")
}
