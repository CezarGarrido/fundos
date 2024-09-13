use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use crate::config::get;

use chrono::{DateTime, Datelike, NaiveDate};
use serde::{Deserialize, Serialize};

use yahoo_finance_api::{time::OffsetDateTime, YahooConnector};

const ROOT: &str = "indices.ibovespa";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Options {
    pub description: String,
    pub path: PathBuf,
}

impl Options {
    pub async fn async_path(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<PathBuf, yahoo_finance_api::YahooError> {
        let provider = YahooConnector::new()?; // Removido o unwrap aqui
        let start = OffsetDateTime::from_unix_timestamp(
            NaiveDate::from_ymd_opt(start_date.year(), start_date.month(), start_date.day())
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap()
                .and_utc()
                .timestamp(),
        )
        .unwrap();
        let end = OffsetDateTime::from_unix_timestamp(
            NaiveDate::from_ymd_opt(end_date.year(), end_date.month(), end_date.day())
                .unwrap()
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_utc()
                .timestamp(),
        )
        .unwrap();

        let path = self.path.clone();
        let h = tokio::spawn(async move {
            let resp = provider.get_quote_history("^BVSP", start, end).await?;
            let quotes: Vec<yahoo_finance_api::Quote> = resp.quotes()?;
            let mut ibovs = Vec::new();
            for q in quotes.into_iter() {
                let dt = DateTime::from_timestamp(q.timestamp as i64, 0).unwrap();
                let date_str = dt.naive_utc().format("%d/%m/%Y").to_string();

                let ibov = Ibov {
                    timestamp: q.timestamp,
                    adjclose: q.adjclose,
                    date: date_str,
                    open: q.open,
                    high: q.high,
                    low: q.low,
                    volume: q.volume,
                    close: q.close,
                };
                ibovs.push(ibov);
            }

            create_and_write_json(&path, &ibovs).unwrap();
            Ok(path)
        });

        h.await.unwrap() // Aguardar o JoinHandle e retornar o resultado
    }
}

pub fn load() -> Result<Options, config::ConfigError> {
    get::<Options>(ROOT)
}

fn create_and_write_json<P: AsRef<Path>, T: serde::Serialize>(
    path: P,
    data: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create the directories if they do not exist
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }

    // Create the file and write the JSON data
    let file = File::create(&path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, data)?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct Ibov {
    pub date: String,
    pub timestamp: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub volume: u64,
    pub close: f64,
    pub adjclose: f64,
}
