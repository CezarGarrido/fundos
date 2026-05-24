use axum::{Json, http::StatusCode};
use fundos_common::types::{ClassStat, DashboardStats, SituationStat, YearStat};

use crate::providers::cvm::fund::Register;

/// GET /api/dashboard/stats
pub async fn get_dashboard_stats() -> Result<Json<DashboardStats>, StatusCode> {
    let register = Register::new();

    match register.async_stats().await {
        Ok((by_year_df, by_situation_df, by_class_df)) => {
            let by_year = df_to_year_stats(&by_year_df);
            let by_situation = df_to_situation_stats(&by_situation_df);
            let by_class = df_to_class_stats(&by_class_df);
            let total_funds = by_year.iter().map(|y| y.count).sum();

            Ok(Json(DashboardStats {
                total_funds,
                by_year,
                by_class,
                by_situation,
            }))
        }
        Err(e) => {
            log::error!("Dashboard stats error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn df_to_year_stats(df: &polars::frame::DataFrame) -> Vec<YearStat> {
    let mut stats = Vec::new();
    for i in 0..df.height() {
        let year = df.column("Ano").ok()
            .and_then(|s| s.get(i).ok())
            .and_then(|v| v.try_extract::<i32>().ok().map(|n| n.to_string()))
            .unwrap_or_default();
        let count = df.column("Quant").ok()
            .and_then(|s| s.get(i).ok())
            .and_then(|v| v.try_extract::<i64>().ok())
            .unwrap_or(0) as usize;
        stats.push(YearStat { year, count });
    }
    stats.retain(|y| !y.year.is_empty());
    stats.sort_by_key(|y| y.year.clone());
    stats
}

fn df_to_situation_stats(df: &polars::frame::DataFrame) -> Vec<SituationStat> {
    let mut stats = Vec::new();
    for i in 0..df.height() {
        let situation = df.column("SIT").ok()
            .and_then(|s| s.get(i).ok())
            .and_then(|v| v.get_str().map(|s| s.to_string()))
            .unwrap_or_default();
        let count = df.column("TP_FUNDO").ok()
            .and_then(|s| s.get(i).ok())
            .and_then(|v| v.try_extract::<u32>().ok().map(|n| n as usize))
            .unwrap_or(0);
        stats.push(SituationStat { situation, count });
    }
    stats.sort_by_key(|s| std::cmp::Reverse(s.count));
    stats
}

fn df_to_class_stats(df: &polars::frame::DataFrame) -> Vec<ClassStat> {
    let mut stats = Vec::new();
    for i in 0..df.height() {
        let class = df.column("CLASSE").ok()
            .and_then(|s| s.get(i).ok())
            .and_then(|v| v.get_str().map(|s| s.to_string()))
            .unwrap_or_default();
        let count = df.column("TP_FUNDO").ok()
            .and_then(|s| s.get(i).ok())
            .and_then(|v| v.try_extract::<u32>().ok().map(|n| n as usize))
            .unwrap_or(0);
        stats.push(ClassStat { class, count });
    }
    stats.sort_by_key(|c| std::cmp::Reverse(c.count));
    stats
}
