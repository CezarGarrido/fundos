use chrono::{DateTime, NaiveDate, Utc};
use std::sync::Arc;
use tokio::sync::Mutex;
use yahoo_finance_api as yahoo;

#[derive(Debug, Clone)]
pub struct MonthlyQuote {
    pub date: String, // Format: YYYY-MM
    pub close_price: f64,
}

pub struct YahooProvider {
    pub cache: Arc<Mutex<std::collections::HashMap<String, Vec<MonthlyQuote>>>>,
}

impl YahooProvider {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Busca as cotações históricas de um ativo, retornando o preço de fechamento no último dia de cada mês.
    pub async fn get_monthly_quotes(&self, ticker: &str, months_back: i64) -> Result<Vec<MonthlyQuote>, String> {
        let mut cache = self.cache.lock().await;
        if let Some(cached) = cache.get(ticker) {
            return Ok(cached.clone());
        }

        let provider = yahoo::YahooConnector::new().map_err(|e| e.to_string())?;
        
        let end = Utc::now();
        let start = end - chrono::Duration::days(months_back * 30);
        
        let start_ts = yahoo_finance_api::time::OffsetDateTime::from_unix_timestamp(start.timestamp()).unwrap();
        let end_ts = yahoo_finance_api::time::OffsetDateTime::from_unix_timestamp(end.timestamp()).unwrap();

        // Ensure .SA for brazilian stocks if not provided
        let mut symbol = ticker.to_string();
        if !symbol.ends_with(".SA") && symbol.len() <= 6 {
            symbol = format!("{}.SA", symbol);
        }

        let response = provider.get_quote_history(&symbol, start_ts, end_ts).await.map_err(|e| e.to_string())?;
        let quotes = response.quotes().map_err(|e| e.to_string())?;

        let mut monthly_quotes: std::collections::HashMap<String, f64> = std::collections::HashMap::new();

        for q in quotes {
            if let Some(dt) = DateTime::from_timestamp(q.timestamp as i64, 0) {
                let month_key = dt.format("%Y-%m").to_string();
                // Overwrite with the latest in the month
                monthly_quotes.insert(month_key, q.close);
            }
        }

        let mut result: Vec<MonthlyQuote> = monthly_quotes
            .into_iter()
            .map(|(date, close_price)| MonthlyQuote { date, close_price })
            .collect();

        // Sort chronologically
        result.sort_by(|a, b| a.date.cmp(&b.date));

        cache.insert(ticker.to_string(), result.clone());

        Ok(result)
    }
}
