use crate::config::get;
use serde_derive::{Deserialize, Serialize};

use super::downloader::Document;

const ROOT: &str = "app.cvm";

#[derive(Debug, Serialize, Deserialize)]
pub struct DownloaderOptions {
    pub documents: Vec<Document>,
}

pub fn load() -> Result<DownloaderOptions, config::ConfigError> {
    get::<DownloaderOptions>(ROOT)
}
