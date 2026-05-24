//! HTTP client for the Fundos API backend.
//! All data processing happens server-side — the frontend only calls these functions.

use chrono::NaiveDate;
use fundos_common::types::*;
use serde::de::DeserializeOwned;

const DEFAULT_API_URL: &str = "http://localhost:3000";

fn api_url() -> String {
    std::env::var("FUNDOS_API_URL").unwrap_or_else(|_| DEFAULT_API_URL.to_string())
}

/// Strip CNPJ formatting (dots, slash, dash) — CNPJ is a numeric identifier.
fn clean_cnpj(cnpj: &str) -> String {
    cnpj.chars().filter(|c| c.is_ascii_digit()).collect()
}

async fn get<T: DeserializeOwned>(path: &str) -> Result<T, String> {
    let url = format!("{}{}", api_url(), path);
    reqwest::get(&url)
        .await
        .map_err(|e| format!("Network error: {}", e))?
        .json::<T>()
        .await
        .map_err(|e| format!("Parse error: {}", e))
}

// ── Funds ──────────────────────────────────────────────────────────

pub async fn search_funds(keyword: Option<&str>, class: Option<&str>) -> Result<FundSearchResponse, String> {
    let mut path = "/api/funds?".to_string();
    if let Some(k) = keyword { path.push_str(&format!("keyword={}&", k)); }
    if let Some(c) = class { path.push_str(&format!("class={}&", c)); }
    get(&path).await
}

pub async fn get_fund_detail(cnpj: &str) -> Result<FundDetail, String> {
    get(&format!("/api/funds/{}", clean_cnpj(cnpj))).await
}

// ── Profitability ──────────────────────────────────────────────────

pub async fn get_profit(cnpj: &str, start: NaiveDate, end: NaiveDate) -> Result<ProfitResponse, String> {
    get(&format!(
        "/api/funds/{}/profit?start={}&end={}",
        clean_cnpj(cnpj),
        start.format("%Y-%m-%d"),
        end.format("%Y-%m-%d"),
    )).await
}

// ── Portfolio ──────────────────────────────────────────────────────

pub async fn get_portfolio(cnpj: &str, year: &str, month: &str) -> Result<PortfolioResponse, String> {
    get(&format!(
        "/api/funds/{}/portfolio?year={}&month={}",
        clean_cnpj(cnpj), year, month,
    )).await
}

// ── Historical ─────────────────────────────────────────────────────

pub async fn get_history(
    cnpj: &str, start: NaiveDate, end: NaiveDate, batch: u32,
) -> Result<HistoryResponse, String> {
    get(&format!(
        "/api/funds/{}/history?start={}&end={}&batch={}",
        clean_cnpj(cnpj),
        start.format("%Y-%m-%d"),
        end.format("%Y-%m-%d"),
        batch,
    )).await
}

// ── Assets ─────────────────────────────────────────────────────────

pub async fn get_market_assets(start: NaiveDate, end: NaiveDate) -> Result<MarketAssetsResponse, String> {
    get(&format!(
        "/api/assets/market?start={}&end={}",
        start.format("%Y-%m-%d"),
        end.format("%Y-%m-%d"),
    )).await
}

pub async fn get_asset_holders(asset_id: &str, start: NaiveDate, end: NaiveDate) -> Result<AssetHoldersResponse, String> {
    get(&format!(
        "/api/assets/{}/holders?start={}&end={}",
        asset_id,
        start.format("%Y-%m-%d"),
        end.format("%Y-%m-%d"),
    )).await
}

// ── Yahoo Finance ──────────────────────────────────────────────────

pub async fn get_yahoo_prices(ticker: &str, start: NaiveDate, end: NaiveDate) -> Result<YahooPriceResponse, String> {
    get(&format!(
        "/api/prices/yahoo/{}?start={}&end={}",
        ticker,
        start.format("%Y-%m-%d"),
        end.format("%Y-%m-%d"),
    )).await
}

// ── Dashboard ──────────────────────────────────────────────────────

pub async fn get_dashboard_stats() -> Result<DashboardStats, String> {
    get("/api/dashboard/stats").await
}

// ── Analytics ──────────────────────────────────────────────────────

pub async fn get_asset_analytics(asset_id: &str, fund_cnpj: Option<&str>) -> Result<AssetAnalyticsResponse, String> {
    let mut path = format!("/api/assets/{}/analytics", asset_id);
    if let Some(cnpj) = fund_cnpj {
        path.push_str(&format!("?fund_cnpj={}", clean_cnpj(cnpj)));
    }
    get(&path).await
}
