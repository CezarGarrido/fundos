use std::path::PathBuf;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::config::get;

const ROOT: &str = "app.cvm.fundo.carteira";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Options {
    pub description: String,
    pub url: String,
    pub path: PathBuf,
    pub start_date: String,
    pub end_date: String,
}

impl Options {
    pub fn start_date(&self) -> NaiveDate {
        NaiveDate::parse_from_str(&self.start_date, "%Y/%m/%d").unwrap()
    }
    pub fn end_date(&self) -> NaiveDate {
        NaiveDate::parse_from_str(&self.end_date, "%Y/%m/%d").unwrap()
    }
}

pub fn load() -> Result<Options, config::ConfigError> {
    get::<Options>(ROOT)
}
