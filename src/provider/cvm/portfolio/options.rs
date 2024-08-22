use std::{path::PathBuf, sync::Arc};

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::{config::get, provider::cvm::try_download};

const ROOT: &str = "cvm.fundo.carteira";

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct Options {
    pub description: String,
    pub url: String,
    pub historical_url: String,
    pub path: PathBuf,
    pub start_date: String,
    pub end_date: String,
}

impl Options {
    pub fn start_date(&self) -> NaiveDate {
        NaiveDate::parse_from_str(&self.start_date, "%d/%m/%Y").unwrap()
    }
    pub fn end_date(&self) -> NaiveDate {
        NaiveDate::parse_from_str(&self.end_date, "%d/%m/%Y").unwrap()
    }
    pub fn urls(&self) -> Vec<String> {
        // self.generate_patterns(self.start_date(), self.end_date(), &self.url)
        vec![]
    }

    pub fn urls_with_dates(
        &self,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Vec<(String, String)> {
        let start_date = start_date.unwrap_or(self.start_date());
        let end_date = end_date.unwrap_or(self.end_date());
        self.generate_patterns(start_date, end_date, &self.url, &self.historical_url)
    }

    pub async fn async_path(
        &self,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
    ) -> Result<Vec<PathBuf>, cached_path::Error> {
        //use cached_path::Options;
        let urls = self.urls_with_dates(start_date, end_date).clone();
        let mut handles = vec![];
        // Limita o número de threads bloqueantes simultâneas
        let semaphore = Arc::new(Semaphore::new(4)); // Limita a 4 tarefas ao mesmo tempo (ajuste conforme necessário)
                                                     // Para cada URL, cria uma tarefa assíncrona separada
        for (url, historical_url) in urls {
            let semaphore_clone = semaphore.clone();
            let handle = tokio::spawn(async move {
                match try_download(
                    url.clone(),
                    "portfolio".to_string(),
                    semaphore_clone.clone(),
                )
                .await
                {
                    Ok(path) => Ok(path),
                    Err(_) => {
                        // Tenta baixar dos dados históricos se falhar
                        try_download(historical_url, "portfolio".to_string(), semaphore_clone).await
                    }
                }
            });
            handles.push(handle);
        }

        // Aguarda todas as tarefas serem concluídas
        let mut paths = Vec::new();
        for handle in handles {
            let path = handle.await.unwrap()?;
            paths.push(path);
        }
        Ok(paths)
    }

    fn generate_patterns(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        url: &str,
        historical_url: &str,
    ) -> Vec<(String, String)> {
        let mut patterns = Vec::new();
        let mut current_date = start_date;
        while current_date <= end_date {
            // Formata o ano e o mês no formato desejado
            let year = current_date.year();
            let month = current_date.month();
            // Substitua os placeholders no template do caminho com o ano e o mês atuais
            let mut pattern = url.to_string();
            pattern = pattern.replace("{year}", &year.to_string());
            pattern = pattern.replace("{month}", &format!("{:02}", month));

            let mut hist = historical_url.to_string();
            hist = hist.replace("{year}", &year.to_string());

            // Adiciona o padrão à lista
            patterns.push((pattern, hist));
            // Avança para o próximo mês
            current_date = current_date
                .with_month(month + 1)
                .unwrap_or(NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap());
        }
        patterns.dedup();

        patterns
    }
}

pub fn load() -> Result<Options, config::ConfigError> {
    get::<Options>(ROOT)
}
