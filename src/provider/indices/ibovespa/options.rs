use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use crate::config::get;
use cached_path::Cache;
use chrono::{DateTime, Datelike, NaiveDate};
use chrono::{TimeZone, Utc};
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;
use tokio::time::sleep;
use yahoo_finance_api::{
    time::{Date, Month, OffsetDateTime, Time},
    YahooConnector,
};

const ROOT: &str = "indices.ibovespa";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Options {
    pub description: String,
    pub start_date: String,
    pub end_date: String,
    pub path: PathBuf,
}

impl Options {
    pub fn start_date(&self) -> NaiveDate {
        NaiveDate::parse_from_str(&self.start_date, "%d/%m/%Y").unwrap()
    }
    pub fn end_date(&self) -> NaiveDate {
        NaiveDate::parse_from_str(&self.end_date, "%d/%m/%Y").unwrap()
    }

    pub async fn async_path(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<PathBuf, yahoo_finance_api::YahooError> {
        let provider = YahooConnector::new()?; // Removido o unwrap aqui
        let start = OffsetDateTime::from_unix_timestamp(
            NaiveDate::from_ymd(start_date.year(), start_date.month(), start_date.day())
                .and_hms(0, 0, 0)
                .timestamp(),
        )
        .unwrap();
        let end = OffsetDateTime::from_unix_timestamp(
            NaiveDate::from_ymd(end_date.year(), end_date.month(), end_date.day())
                .and_hms(23, 59, 59)
                .timestamp(),
        )
        .unwrap();

        let path = self.path.clone();
        let h = tokio::spawn(async move {
            let resp = provider.get_quote_history("^BVSP", start, end).await?;
            let quotes: Vec<yahoo_finance_api::Quote> = resp.quotes()?;
            let mut ibovs = Vec::new();
            for q in quotes.into_iter() {
                let ibov = Ibov {
                    timestamp: q.timestamp,
                    adjclose: q.adjclose,
                    date: DateTime::<Utc>::from_utc(
                        chrono::NaiveDateTime::from_timestamp(q.timestamp as i64, 0),
                        Utc,
                    )
                    .format("%d/%m/%Y")
                    .to_string(),
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
