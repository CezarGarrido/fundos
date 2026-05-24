use fundos_backend::providers::cvm::portfolio::Portfolio;
use polars::prelude::*;

#[tokio::main]
async fn main() {
    let portfolio = Portfolio::new();
    let end = chrono::NaiveDate::from_ymd_opt(2026, 5, 1).unwrap();
    let start = chrono::NaiveDate::from_ymd_opt(2026, 4, 1).unwrap();
    
    let res = portfolio.async_historical_assets("12.055.107/0001-16".to_string(), start, end).await;
    match res {
        Ok(df) => {
            println!("DF Shape: {:?}", df.shape());
            // find columns that have at least one non-null value
            for f in df.schema().iter_fields() {
                let name = f.name();
                if let Ok(col) = df.column(name) {
                    if col.null_count() < col.len() {
                        println!("- {} ({} non-nulls)", name, col.len() - col.null_count());
                    }
                }
            }
        }
        Err(e) => println!("Error: {}", e),
    }
}
