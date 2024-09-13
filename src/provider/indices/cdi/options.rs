use crate::config::get;
use cached_path::{cached_path_with_options, Cache};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::task::spawn_blocking;

const ROOT: &str = "indices.cdi";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Options {
    pub description: String,
    pub url: String,
    pub path: String,
}

impl Options {
    pub fn url_with_date(&self, start_date: NaiveDate, end_date: NaiveDate) -> String {
        let mut pattern = self.url.to_string();
        pattern = pattern.replace("{start_date}", &start_date.format("%d/%m/%Y").to_string());
        pattern = pattern.replace("{end_date}", &end_date.format("%d/%m/%Y").to_string());
        pattern
    }

    pub async fn async_path(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<PathBuf, cached_path::Error> {
        let url = self.url_with_date(start_date, end_date).clone();
        let subdir = self.path.clone();
        // Baixa o arquivo usando `cached_path`
        

        spawn_blocking(move || {
            let res = cached_path_with_options(
                url.as_str(),
                &cached_path::Options::default().subdir(&subdir),
            );

            match res {
                Ok(path) => Ok(path),
                Err(_err) => {
                    let cache = Cache::builder()
                        .progress_bar(Some(cached_path::ProgressBar::Full))
                        .offline(true)
                        .build()?;
                    cache.cached_path_with_options(
                        url.as_str(),
                        &cached_path::Options::default().subdir(&subdir),
                    )
                }
            }
        })
        .await
        .unwrap()
    }
}

pub fn load() -> Result<Options, config::ConfigError> {
    get::<Options>(ROOT)
}
