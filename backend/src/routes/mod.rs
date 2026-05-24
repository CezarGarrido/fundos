use axum::{Router, routing::get};

mod funds;
mod profit;
mod portfolio_routes;
mod history;
mod assets;
mod yahoo;
mod analytics;
mod dashboard;
mod indices;

pub fn api_router() -> Router {
    Router::new()
        // Funds
        .route("/funds", get(funds::search_funds))
        .route("/funds/:cnpj", get(funds::get_fund))
        .route("/funds/:cnpj/profit", get(profit::get_profit))
        .route("/funds/:cnpj/portfolio", get(portfolio_routes::get_portfolio))
        .route("/funds/:cnpj/history", get(history::get_history))
        .route("/funds/:cnpj/history/status", get(history::get_history_status))
        // Assets
        .route("/assets/market", get(assets::get_market_assets))
        .route("/assets/:id/holders", get(assets::get_asset_holders))
        .route("/assets/:id/detail", get(assets::get_asset_detail))
        .route("/assets/:id/analytics", get(analytics::get_asset_analytics))
        // Yahoo
        .route("/prices/yahoo/:ticker", get(yahoo::get_yahoo_prices))
        // Indices
        .route("/indices/cdi", get(indices::get_cdi))
        .route("/indices/ibovespa", get(indices::get_ibovespa))
        // Dashboard
        .route("/dashboard/stats", get(dashboard::get_dashboard_stats))
}
