use axum::{Json, extract::Query, http::StatusCode};
use fundos_common::types::{FundSummary, FundSearchParams, FundSearchResponse, FundDetail};

use crate::providers::cvm::fund::{Class, Register};

/// GET /api/funds?keyword=&class=&limit=&offset=
pub async fn search_funds(
    Query(params): Query<FundSearchParams>,
) -> Result<Json<FundSearchResponse>, StatusCode> {
    let register = Register::new();
    let class = params.class.and_then(|c| match c.to_lowercase().as_str() {
        "renda fixa" | "renda_fixa" | "rendafixa" => Some(Class::RendaFixa),
        "acoes" | "ações" => Some(Class::Acoes),
        "cambial" => Some(Class::Cambial),
        "multimercado" | "multimarket" | "multi_market" => Some(Class::MultiMarket),
        _ => None,
    });

    match register.async_find(params.keyword, class, None, params.limit.map(|l| l as u32)).await {
        Ok(df) => {
            let funds: Vec<FundSummary> = df_to_fund_summaries(&df);
            let total = funds.len();
            Ok(Json(FundSearchResponse { total, funds }))
        }
        Err(e) => {
            log::error!("Fund search error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// GET /api/funds/{cnpj}
pub async fn get_fund(
    axum::extract::Path(cnpj): axum::extract::Path<String>,
) -> Result<Json<FundDetail>, StatusCode> {
    log::info!("Fetching fund detail for {}", cnpj);
    let cnpj = crate::cnpj::normalize(&cnpj);
    let register = Register::new();

    // Try offline first, then online
    let df = match register.async_find_by_cnpj(cnpj.clone(), true).await {
        Ok(df) if df.height() > 0 => df,
        _ => register.async_find_by_cnpj(cnpj.clone(), false).await.map_err(|e| {
            log::error!("Fund detail error for {}: {}", cnpj, e);
            StatusCode::NOT_FOUND
        })?,
    };

    if df.height() == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    let detail = row_to_fund_detail(&df, 0).ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(detail))
}

// ── DataFrame → typed helpers ──────────────────────────────────────

fn df_to_fund_summaries(df: &polars::frame::DataFrame) -> Vec<FundSummary> {
    let mut funds = Vec::new();
    for i in 0..df.height() {
        let cnpj = get_str(df, i, "CNPJ_FUNDO").unwrap_or_default();
        let name = get_str(df, i, "DENOM_SOCIAL").unwrap_or_default();
        let class = get_str(df, i, "CLASSE").unwrap_or_default();
        let situation = get_str(df, i, "SIT").unwrap_or_default();
        let administrator = get_str(df, i, "ADMIN").unwrap_or_default();
        let start_date = get_str(df, i, "DT_REG").unwrap_or_default();

        funds.push(FundSummary {
            cnpj,
            name,
            class,
            situation,
            administrator,
            start_date,
        });
    }
    funds
}

fn row_to_fund_detail(df: &polars::frame::DataFrame, idx: usize) -> Option<FundDetail> {
    Some(FundDetail {
        cnpj: get_str(df, idx, "CNPJ_FUNDO").unwrap_or_default(),
        denom_social: get_str(df, idx, "DENOM_SOCIAL").unwrap_or_default(),
        classe: get_str(df, idx, "CLASSE").unwrap_or_default(),
        situacao: get_str(df, idx, "SIT").unwrap_or_default(),
        administrador: get_str(df, idx, "ADMIN").unwrap_or_default(),
        dt_inicio: get_str(df, idx, "DT_INI_SIT").unwrap_or_default(),
        dt_inicio_sit: get_str(df, idx, "DT_INI_SIT").unwrap_or_default(),
        rentab_fundo: get_str(df, idx, "RENTAB_FUNDO").unwrap_or_default(),
        taxa_adm: get_str(df, idx, "TAXA_ADM").unwrap_or_default(),
        taxa_perf: get_str(df, idx, "TAXA_PERFM").unwrap_or_default(),
        valor_patrim_liq: get_str(df, idx, "VL_PATRIM_LIQ").unwrap_or_default(),
        cotistas_total: get_str(df, idx, "COTISTAS_TOTAL").unwrap_or_default(),
        auditor: get_str(df, idx, "AUDITOR").unwrap_or_default(),
        gestor: get_str(df, idx, "GESTOR").unwrap_or_default(),
        adm_custodiante: get_str(df, idx, "ADM_CUSTODIANTE").unwrap_or_default(),
    })
}

fn get_str(df: &polars::frame::DataFrame, idx: usize, col_name: &str) -> Option<String> {
    let series = df.column(col_name).ok()?;
    let any_value = series.get(idx).ok()?;
    any_value.get_str().map(|s| s.to_string())
}
