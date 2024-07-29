use std::path::PathBuf;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::config::get;

const ROOT: &str = "app.indicator.ibovespa";

#[derive(Debug, Serialize, Deserialize, Default)]
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
}

pub fn load() -> Result<Options, config::ConfigError> {
    get::<Options>(ROOT)
}
