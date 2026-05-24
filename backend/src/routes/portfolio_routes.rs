use axum::{Json, extract::Query, http::StatusCode};
use chrono::NaiveDate;
use fundos_common::types::{PortfolioAsset, PortfolioPL, PortfolioResponse};
use serde::Deserialize;

use crate::providers::cvm::portfolio::Portfolio;

#[derive(Deserialize)]
pub struct PortfolioParams {
    pub year: String,
    pub month: String,
}

/// GET /api/funds/{cnpj}/portfolio?year=...&month=...
pub async fn get_portfolio(
    axum::extract::Path(cnpj): axum::extract::Path<String>,
    Query(params): Query<PortfolioParams>,
) -> Result<Json<PortfolioResponse>, StatusCode> {
    let cnpj = crate::cnpj::normalize(&cnpj);
    let portfolio = Portfolio::new();
    let mut current_year: i32 = params.year.parse().unwrap_or(2024);
    let mut current_month: u32 = params.month.parse().unwrap_or(1);
    let mut result = None;

    for _ in 0..6 {
        let y_str = current_year.to_string();
        let m_str = format!("{:02}", current_month);
        match portfolio.async_assets(cnpj.clone(), y_str.clone(), m_str.clone(), true).await {
            Ok((pl, assets, top_assets)) if !assets.is_empty() => {
                result = Some((pl, assets, top_assets, y_str, m_str));
                break;
            }
            _ => {}
        }
        if current_month == 1 {
            current_month = 12;
            current_year -= 1;
        } else {
            current_month -= 1;
        }
    }

    match result {
        Some((pl, assets, top_assets, ref_year, ref_month)) => {
            Ok(Json(PortfolioResponse {
                cnpj,
                reference_year: ref_year,
                reference_month: ref_month,
                assets: df_to_portfolio_assets(&assets),
                top_assets: df_to_portfolio_assets(&top_assets),
                patrimonio_liquido: df_to_portfolio_pl(&pl),
            }))
        }
        None => Ok(Json(PortfolioResponse {
            cnpj,
            reference_year: params.year,
            reference_month: params.month,
            assets: vec![],
            top_assets: vec![],
            patrimonio_liquido: vec![],
        })),
    }
}

// ── DataFrame → typed helpers ──────────────────────────────────────

fn df_to_portfolio_assets(df: &polars::frame::DataFrame) -> Vec<PortfolioAsset> {
    let mut assets = Vec::new();
    for i in 0..df.height() {
        assets.push(PortfolioAsset {
            codigo_isin: get_str(df, i, "CD_ISIN")
                .or_else(|| get_str(df, i, "ISIN"))
                .unwrap_or_default(),
            nome_ativo: get_str(df, i, "DENOM_ATIVO")
                .or_else(|| get_str(df, i, "NM_ATIVO"))
                .unwrap_or_default(),
            tipo_ativo: get_str(df, i, "TP_ATIVO")
                .or_else(|| get_str(df, i, "TP_APLIC"))
                .unwrap_or_default(),
            codigo_negociacao: get_str(df, i, "CD_NEGOCIACAO")
                .or_else(|| get_str(df, i, "CD_ATIVO"))
                .unwrap_or_default(),
            quantidade: get_f64(df, i, "QT_POS_FINAL")
                .or_else(|| get_f64(df, i, "QT_ESCRT"))
                .unwrap_or(0.0),
            valor_mercado: get_f64(df, i, "VL_MERC_POS_FINAL").unwrap_or(0.0),
            vl_porcentagem_pl: get_f64(df, i, "VL_PORCENTAGEM_PL").unwrap_or(0.0),
        });
    }
    assets
}

fn df_to_portfolio_pl(df: &polars::frame::DataFrame) -> Vec<PortfolioPL> {
    let mut pls = Vec::new();
    for i in 0..df.height() {
        pls.push(PortfolioPL {
            date: get_str(df, i, "DT_COMPTC")
                .and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok())
                .unwrap_or_default(),
            patrimonio_liquido: get_f64(df, i, "VL_PATRIM_LIQ").unwrap_or(0.0),
            valor_cota: get_f64(df, i, "VL_QUOTA").unwrap_or(0.0),
            total_ativos: get_f64(df, i, "VL_TOTAL").unwrap_or(0.0),
        });
    }
    pls
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
