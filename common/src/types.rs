//! Shared types used by both the backend API and the frontend client.
//! All types implement Serialize/Deserialize for JSON communication.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

// ── Fund Search ────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FundSummary {
    pub cnpj: String,
    pub name: String,
    pub class: String,
    pub situation: String,
    pub administrator: String,
    pub start_date: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FundSearchResponse {
    pub total: usize,
    pub funds: Vec<FundSummary>,
}

// ── Fund Detail ────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FundDetail {
    pub cnpj: String,
    pub denom_social: String,
    pub classe: String,
    pub situacao: String,
    pub administrador: String,
    pub dt_inicio: String,
    pub dt_inicio_sit: String,
    pub rentab_fundo: String,
    pub taxa_adm: String,
    pub taxa_perf: String,
    pub valor_patrim_liq: String,
    pub cotistas_total: String,
    pub auditor: String,
    pub gestor: String,
    pub adm_custodiante: String,
}

// ── Time Series (charts) ───────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeSeriesPoint {
    pub date: NaiveDate,
    pub value: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProfitResponse {
    pub cnpj: String,
    pub fund_series: Vec<TimeSeriesPoint>,
    pub cdi_series: Vec<TimeSeriesPoint>,
    pub ibov_series: Vec<TimeSeriesPoint>,
}

// ── Portfolio Composition ──────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioAsset {
    pub codigo_isin: String,
    pub nome_ativo: String,
    pub tipo_ativo: String,
    pub codigo_negociacao: String,
    pub quantidade: f64,
    pub valor_mercado: f64,
    pub vl_porcentagem_pl: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioPL {
    pub date: NaiveDate,
    pub patrimonio_liquido: f64,
    pub valor_cota: f64,
    pub total_ativos: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PortfolioResponse {
    pub cnpj: String,
    pub reference_year: String,
    pub reference_month: String,
    pub assets: Vec<PortfolioAsset>,
    pub top_assets: Vec<PortfolioAsset>, // top 10 by VL_PORCENTAGEM_PL
    pub patrimonio_liquido: Vec<PortfolioPL>,
}

// ── Historical Portfolio ───────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryRow {
    pub reference_date: String, // YYYY-MM (mês de referência)
    pub asset_name: String,
    pub asset_code: String,
    pub codigo_isin: String,
    pub tipo_ativo: String,
    pub tipo_aplic: String,
    pub ds_ativo: String,
    pub nm_fundo_cota: String,
    pub tp_titpub: String,
    pub cnpj_fundo: String,
    pub quantity: f64,
    pub quantity_sold: f64,
    pub market_value: f64,
    pub vl_aquis_negoc: f64,
    pub percentage_pl: f64,
    pub vl_patrim_liq: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryResponse {
    pub cnpj: String,
    pub batch_index: u32,
    pub is_last: bool,
    pub total_batches: u32,
    pub rows: Vec<HistoryRow>,
    pub status: Option<String>,
}

// ── Market Assets ──────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketAsset {
    pub codigo_isin: String,
    pub nome_ativo: String,
    pub codigo_negociacao: String,
    pub tipo_ativo: String,
    pub tipo_aplic: String,
    pub titpub: String,
    pub cd_ativo: String,
    pub ds_ativo: String,
    pub nm_fundo_cota: String,
    pub cd_selic: String,
    pub dt_venc: String,
    pub fund_count: usize,
    pub n_compradores: usize,
    pub n_vendedores: usize,
    pub vl_comprado: f64,
    pub vl_vendido: f64,
    pub pl_medio: f64,
    pub total_market_value: f64,
    pub vl_min: f64,
    pub vl_max: f64,
    pub vl_medio: f64,
    pub vl_compra_min: f64,
    pub vl_compra_max: f64,
    pub vl_compra_medio: f64,
    pub avg_percentage: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketAssetsResponse {
    pub assets: Vec<MarketAsset>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetHolder {
    pub fund_cnpj: String,
    pub fund_name: String,
    pub quantity: f64,
    pub market_value: f64,
    pub percentage_pl: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetHoldersResponse {
    pub asset_id: String,
    pub holders: Vec<AssetHolder>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetDetailResponse {
    pub isin: String,
    pub nome_ativo: String,
    pub codigo_negociacao: String,
    pub tipo_ativo: String,
    pub fund_count: usize,
    pub holders: Vec<AssetHolder>,
}

// ── Dashboard ──────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YearStat {
    pub year: String,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClassStat {
    pub class: String,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SituationStat {
    pub situation: String,
    pub count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DashboardStats {
    pub total_funds: usize,
    pub by_year: Vec<YearStat>,
    pub by_class: Vec<ClassStat>,
    pub by_situation: Vec<SituationStat>,
}

// ── Yahoo Finance ──────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YahooPricePoint {
    pub date: NaiveDate,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub adjclose: f64,
    pub volume: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct YahooPriceResponse {
    pub ticker: String,
    pub prices: Vec<YahooPricePoint>,
}

// ── Analytics ──────────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MonthlySeries {
    pub date: String,        // YYYY-MM
    pub real_qty: f64,
    pub filtered_qty: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
    pub z_score: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ForwardTrajectoryPoint {
    pub date: String,
    pub estimated_qty: f64,
    pub ci_lower: f64,
    pub ci_upper: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetAnalyticsResponse {
    pub asset_id: String,
    pub avg_buy_price: f64,
    pub avg_sell_price: f64,
    pub take_profit_trigger: f64,
    pub speed_to_peak: usize,
    pub hidden_qty_estimates: Vec<(String, f64, f64)>,
    pub bias_direction: String,
    pub stability_score: f64,
    pub z_score: f64,
    pub baseline_qty: f64,
    pub filtered_qty: f64,
    pub historical_states: Vec<MonthlySeries>,
    pub forward_trajectory: Vec<ForwardTrajectoryPoint>,
}

// ── CDI / IBOV Indices ─────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexSeriesResponse {
    pub points: Vec<TimeSeriesPoint>,
}

// ── Generic / Error ────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub details: Option<String>,
}

/// Query params for fund search
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FundSearchParams {
    pub keyword: Option<String>,
    pub class: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}
