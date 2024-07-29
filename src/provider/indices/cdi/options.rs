use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::get;

const ROOT: &str = "app.indicator.cdi";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Options {
    pub description: String,
    pub url: String,
    pub path: PathBuf,
}

pub fn load() -> Result<Options, config::ConfigError> {
    get::<Options>(ROOT)
}
