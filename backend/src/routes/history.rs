use axum::{Json, extract::Query, http::StatusCode};
use chrono::{Datelike, Months, NaiveDate};
use fundos_common::types::{HistoryResponse, HistoryRow};
use serde::Deserialize;

use crate::providers::cvm::portfolio::Portfolio;

#[derive(Deserialize)]
pub struct HistoryParams {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub batch: Option<u32>,
}

/// GET /api/funds/{cnpj}/history?start=...&end=...&batch=...
pub async fn get_history(
    axum::extract::Path(cnpj): axum::extract::Path<String>,
    Query(params): Query<HistoryParams>,
) -> Result<Json<HistoryResponse>, StatusCode> {
    let cnpj = crate::cnpj::normalize(&cnpj);
    let portfolio = Portfolio::new();
    let total_months = ((params.end.year() - params.start.year()) * 12
        + (params.end.month() as i32 - params.start.month() as i32))
        .unsigned_abs();

    let batch_index = params.batch.unwrap_or(0);

    if total_months > 12 {
        // Progressive: return one batch at a time
        let batch_size_months = Months::new(12);
        let batch_start_offset = batch_index * 12;
        let batch_start = params.start
            .checked_add_months(Months::new(batch_start_offset))
            .unwrap_or(params.start);
        let batch_end = std::cmp::min(
            params.end,
            batch_start.checked_add_months(batch_size_months).unwrap_or(params.end),
        );

        let total_batches = (total_months + 11) / 12;
        let is_last = batch_index + 1 >= total_batches;

        match portfolio.async_historical_assets(cnpj.clone(), batch_start, batch_end).await {
            Ok(df) => Ok(Json(HistoryResponse {
                cnpj,
                batch_index,
                is_last,
                total_batches,
                rows: df_to_history_rows(&df),
                status: None,
            })),
            Err(e) => {
                log::error!("History error for {} batch {}: {}", cnpj, batch_index, e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        // Single load
        match portfolio.async_historical_assets(cnpj.clone(), params.start, params.end).await {
            Ok(df) => Ok(Json(HistoryResponse {
                cnpj,
                batch_index: 0,
                is_last: true,
                total_batches: 1,
                rows: df_to_history_rows(&df),
                status: None,
            })),
            Err(e) => {
                log::error!("History error for {}: {}", cnpj, e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    }
}

/// GET /api/funds/{cnpj}/history/status
pub async fn get_history_status(
    axum::extract::Path(cnpj): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let cnpj = crate::cnpj::normalize(&cnpj);
    Ok(Json(serde_json::json!({ "cnpj": cnpj, "status": "ok" })))
}

fn df_to_history_rows(df: &polars::frame::DataFrame) -> Vec<HistoryRow> {
    let mut rows = Vec::new();
    for i in 0..df.height() {
        rows.push(HistoryRow {
            reference_date: get_str(df, i, "DT_COMPTC")
                .or_else(|| get_str(df, i, "month"))
                .unwrap_or_default(),
            asset_name: get_str(df, i, "DENOM_ATIVO")
                .or_else(|| get_str(df, i, "NM_ATIVO"))
                .or_else(|| get_str(df, i, "DS_ATIVO"))
                .unwrap_or_default(),
            asset_code: get_str(df, i, "CD_ATIVO")
                .or_else(|| get_str(df, i, "CD_NEGOCIACAO"))
                .or_else(|| get_str(df, i, "CD_ISIN"))
                .unwrap_or_default(),
            codigo_isin: get_str(df, i, "CD_ISIN").unwrap_or_default(),
            tipo_ativo: get_str(df, i, "TP_ATIVO").unwrap_or_default(),
            tipo_aplic: get_str(df, i, "TP_APLIC").unwrap_or_default(),
            ds_ativo: get_str(df, i, "DS_ATIVO").unwrap_or_default(),
            nm_fundo_cota: get_str(df, i, "NM_FUNDO_COTA").unwrap_or_default(),
            tp_titpub: get_str(df, i, "TP_TITPUB").unwrap_or_default(),
            cnpj_fundo: get_str(df, i, "CNPJ_FUNDO").unwrap_or_default(),
            quantity: get_f64(df, i, "QT_POS_FINAL")
                .or_else(|| get_f64(df, i, "QT_ESCRT"))
                .unwrap_or(0.0),
            quantity_sold: get_f64(df, i, "QT_VENDA").unwrap_or(0.0),
            market_value: get_f64(df, i, "VL_MERC_POS_FINAL").unwrap_or(0.0),
            vl_aquis_negoc: get_f64(df, i, "VL_AQUIS_NEGOC").unwrap_or(0.0),
            percentage_pl: get_f64(df, i, "VL_PORCENTAGEM_PL").unwrap_or(0.0),
            vl_patrim_liq: get_f64(df, i, "VL_PATRIM_LIQ").unwrap_or(0.0),
        });
    }
    rows
}

fn get_str(df: &polars::frame::DataFrame, idx: usize, col_name: &str) -> Option<String> {
    let series = df.column(col_name).ok()?;
    let any_value = series.get(idx).ok()?;
    match any_value {
        polars::datatypes::AnyValue::Null => None,
        polars::datatypes::AnyValue::Date(days) => {
            chrono::NaiveDate::from_ymd_opt(1970, 1, 1)
                .unwrap()
                .checked_add_signed(chrono::Duration::days(days as i64))
                .map(|d| d.format("%Y-%m-%d").to_string())
        }
        _ => {
            if let Some(s) = any_value.get_str() {
                Some(s.to_string())
            } else {
                Some(any_value.to_string())
            }
        }
    }
}

fn get_f64(df: &polars::frame::DataFrame, idx: usize, col_name: &str) -> Option<f64> {
    let series = df.column(col_name).ok()?;
    let any_value = series.get(idx).ok()?;
    if let Ok(v) = any_value.try_extract::<f64>() {
        return Some(v);
    }
    if let Some(s) = any_value.get_str() {
        return s.replace(',', ".").parse::<f64>().ok();
    }
    None
}
