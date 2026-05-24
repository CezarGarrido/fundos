use axum::{Json, extract::Query, http::StatusCode};
use chrono::NaiveDate;
use fundos_common::types::{
    AssetDetailResponse, AssetHolder, AssetHoldersResponse, MarketAsset, MarketAssetsResponse,
};
use polars::frame::DataFrame;
use serde::Deserialize;

use crate::providers::cvm::portfolio::Portfolio;

#[derive(Deserialize)]
pub struct DateRangeParams {
    pub start: NaiveDate,
    pub end: NaiveDate,
}

/// GET /api/assets/market?start=...&end=...
pub async fn get_market_assets(
    Query(params): Query<DateRangeParams>,
) -> Result<Json<MarketAssetsResponse>, StatusCode> {
    let portfolio = Portfolio::new();
    match portfolio.async_market_assets(params.start, params.end).await {
        Ok(df) => Ok(Json(MarketAssetsResponse {
            assets: df_to_market_assets(&df),
        })),
        Err(e) => {
            log::error!("Market assets error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/assets/{id}/holders?start=...&end=...
pub async fn get_asset_holders(
    axum::extract::Path(asset_id): axum::extract::Path<String>,
    Query(params): Query<DateRangeParams>,
) -> Result<Json<AssetHoldersResponse>, StatusCode> {
    let portfolio = Portfolio::new();
    match portfolio.async_asset_holders(asset_id.clone(), params.start, params.end).await {
        Ok(df) => Ok(Json(AssetHoldersResponse {
            asset_id,
            holders: df_to_holders(&df),
        })),
        Err(e) => {
            log::error!("Asset holders error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/assets/{id}/detail
pub async fn get_asset_detail(
    axum::extract::Path(asset_id): axum::extract::Path<String>,
) -> Result<Json<AssetDetailResponse>, StatusCode> {
    let portfolio = Portfolio::new();
    // Default to last 6 months for detail
    let end = chrono::Local::now().naive_local().date();
    let start = end - chrono::Duration::days(183);

    match portfolio.async_asset_holders(asset_id.clone(), start, end).await {
        Ok(df) => {
            let holders = df_to_holders(&df);
            let fund_count = holders.len();
            // Extract asset metadata from first row if available
            let nome_ativo = get_str(&df, 0, "NM_FUNDO").unwrap_or_default();
            let tipo_ativo = get_str(&df, 0, "TP_ATIVO")
                .or_else(|| get_str(&df, 0, "TP_APLIC"))
                .unwrap_or_default();

            Ok(Json(AssetDetailResponse {
                isin: asset_id,
                nome_ativo,
                codigo_negociacao: String::new(),
                tipo_ativo,
                fund_count,
                holders,
            }))
        }
        Err(e) => {
            log::error!("Asset detail error: {}", e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

// ── DataFrame → typed helpers ──────────────────────────────────────

fn df_to_market_assets(df: &DataFrame) -> Vec<MarketAsset> {
    let mut assets = Vec::new();
    for i in 0..df.height() {
        assets.push(MarketAsset {
            codigo_isin: get_str(df, i, "CD_ISIN").unwrap_or_default(),
            nome_ativo: get_str(df, i, "NM_ATIVO")
                .or_else(|| get_str(df, i, "DENOM_ATIVO"))
                .unwrap_or_default(),
            codigo_negociacao: get_str(df, i, "CD_NEGOCIACAO")
                .or_else(|| get_str(df, i, "CD_ATIVO"))
                .unwrap_or_default(),
            tipo_ativo: get_str(df, i, "TP_ATIVO").unwrap_or_default(),
            tipo_aplic: get_str(df, i, "TP_APLIC").unwrap_or_default(),
            titpub: get_str(df, i, "TP_TITPUB").unwrap_or_default(),
            cd_ativo: get_str(df, i, "CD_ATIVO").unwrap_or_default(),
            ds_ativo: get_str(df, i, "DS_ATIVO").unwrap_or_default(),
            nm_fundo_cota: get_str(df, i, "NM_FUNDO_COTA").unwrap_or_default(),
            cd_selic: get_str(df, i, "CD_SELIC").unwrap_or_default(),
            dt_venc: get_str(df, i, "DT_VENC").unwrap_or_default(),
            fund_count: get_i64(df, i, "N_FUNDOS").unwrap_or(0) as usize,
            n_compradores: get_i64(df, i, "N_COMPRADORES").unwrap_or(0) as usize,
            n_vendedores: get_i64(df, i, "N_VENDEDORES").unwrap_or(0) as usize,
            vl_comprado: get_f64(df, i, "VL_COMPRADO").unwrap_or(0.0),
            vl_vendido: get_f64(df, i, "VL_VENDIDO").unwrap_or(0.0),
            pl_medio: get_f64(df, i, "PL_MEDIO").unwrap_or(0.0),
            total_market_value: get_f64(df, i, "VL_MERC_POS_FINAL_sum")
                .or_else(|| get_f64(df, i, "TOTAL_VL_MERC"))
                .or_else(|| get_f64(df, i, "VL_MERC_TOTAL"))
                .unwrap_or(0.0),
            vl_min: get_f64(df, i, "VL_MIN").unwrap_or(0.0),
            vl_max: get_f64(df, i, "VL_MAX").unwrap_or(0.0),
            vl_medio: get_f64(df, i, "VL_MEDIO").unwrap_or(0.0),
            vl_compra_min: get_f64(df, i, "VL_COMPRA_MIN").unwrap_or(0.0),
            vl_compra_max: get_f64(df, i, "VL_COMPRA_MAX").unwrap_or(0.0),
            vl_compra_medio: get_f64(df, i, "VL_COMPRA_MEDIO").unwrap_or(0.0),
            avg_percentage: get_f64(df, i, "VL_PORCENTAGEM_PL_avg")
                .or_else(|| get_f64(df, i, "AVG_PERCENTAGE"))
                .unwrap_or(0.0),
        });
    }
    assets
}

fn df_to_holders(df: &DataFrame) -> Vec<AssetHolder> {
    let mut holders = Vec::new();
    for i in 0..df.height() {
        holders.push(AssetHolder {
            fund_cnpj: get_str(df, i, "CNPJ_FUNDO").unwrap_or_default(),
            fund_name: get_str(df, i, "NM_FUNDO").unwrap_or_default(),
            quantity: get_f64(df, i, "QT_POS_FINAL")
                .or_else(|| get_f64(df, i, "QT_ESCRT"))
                .unwrap_or(0.0),
            market_value: get_f64(df, i, "VL_MERC_POS_FINAL").unwrap_or(0.0),
            percentage_pl: get_f64(df, i, "VL_PORCENTAGEM_PL").unwrap_or(0.0),
        });
    }
    holders
}

fn get_str(df: &DataFrame, idx: usize, col_name: &str) -> Option<String> {
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

fn get_f64(df: &DataFrame, idx: usize, col_name: &str) -> Option<f64> {
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

fn get_i64(df: &DataFrame, idx: usize, col_name: &str) -> Option<i64> {
    let series = df.column(col_name).ok()?;
    let any_value = series.get(idx).ok()?;
    match any_value {
        polars::datatypes::AnyValue::Int64(v) => Some(v),
        polars::datatypes::AnyValue::Int32(v) => Some(v as i64),
        polars::datatypes::AnyValue::UInt64(v) => Some(v as i64),
        polars::datatypes::AnyValue::UInt32(v) => Some(v as i64),
        _ => {
            if let Ok(v) = any_value.try_extract::<i64>() {
                return Some(v);
            }
            if let Some(s) = any_value.get_str() {
                return s.replace(',', ".").parse::<f64>().ok().map(|f| f as i64);
            }
            None
        }
    }
}
