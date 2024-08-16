use chrono::{Datelike, NaiveDate};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::get;

const ROOT: &str = "cvm.fundo.informe";

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
        NaiveDate::parse_from_str(&self.start_date, "%d/%m/%Y").unwrap()
    }
    pub fn end_date(&self) -> NaiveDate {
        NaiveDate::parse_from_str(&self.end_date, "%d/%m/%Y").unwrap()
    }

    pub fn urls(&self) -> Vec<String> {
        let urls = self.generate_patterns(self.start_date(), self.end_date(), &self.url);

        println!("datas informes {}", urls.len());

        urls
    }

    fn generate_patterns(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        path_template: &str,
    ) -> Vec<String> {
        let mut patterns = Vec::new();

        let mut current_date = start_date;
        while current_date <= end_date {
            // Formata o ano e o mês no formato desejado
            let year = current_date.year();
            let month = current_date.month();

            // Substitua os placeholders no template do caminho com o ano e o mês atuais
            let mut pattern = path_template.to_string();
            pattern = pattern.replace("{year}", &year.to_string());
            pattern = pattern.replace("{month}", &format!("{:02}", month));

            // Adiciona o padrão à lista
            patterns.push(pattern);

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
