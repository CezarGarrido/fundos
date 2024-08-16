use crate::config::get;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const ROOT: &str = "indices.cdi";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Options {
    pub description: String,
    pub url: String,
    pub start_date: String,
    pub end_date: String,
    pub path: PathBuf,
}

impl Options {
    pub fn urls(&self) -> Vec<String> {
        let mut pattern = self.url.to_string();
        pattern = pattern.replace("{start_date}", &self.start_date);
        pattern = pattern.replace("{end_date}", &self.end_date);
        vec![pattern]
    }
}

pub fn load() -> Result<Options, config::ConfigError> {
    get::<Options>(ROOT)
}
