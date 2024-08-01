use std::{
    fmt,
    fs::{self, File},
    io::Write,
    path::Path,
};
pub mod options;

use encoding_rs::WINDOWS_1252;
use options::{load, Options};
use polars::{
    error::PolarsError,
    frame::DataFrame,
    lazy::dsl::{col, lit, Expr, GetOutput},
    prelude::{LazyCsvReader, LazyFileListReader, LazyFrame, SortOptions},
    series::IntoSeries,
};
use regex::Regex;
use tokio_util::sync::CancellationToken;
use unicode_normalization::UnicodeNormalization;

use crate::ui::download::Download;

#[derive(Clone)]
pub struct Register {
    options: Options,
    funds: LazyFrame,
}

pub enum Situation {
    Normal,
}

impl Situation {
    pub fn to_string(&self) -> &str {
        match self {
            Situation::Normal => "EM FUNCIONAMENTO NORMAL",
        }
    }
}

#[derive(Clone, PartialEq)]
pub enum Class {
    RendaFixa,
    Acoes,
    Cambial,
    MultiMarket,
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Class::Acoes => write!(f, "Fundo de Ações"),
            Class::RendaFixa => write!(f, "Fundo de Renda Fixa"),
            Class::Cambial => write!(f, "Fundo Cambial"),
            Class::MultiMarket => write!(f, "Fundo Multimercado"),
        }
    }
}

impl Register {
    pub fn new() -> Self {
        let options = load().unwrap();

        Self {
            options,
            funds: LazyFrame::default(),
        }
    }

    pub fn load(&mut self) -> Result<(), PolarsError> {
        match LazyCsvReader::new(&self.options.path)
            .has_header(true)
            .with_infer_schema_length(None)
            .with_delimiter(b';')
            .with_cache(true)
            .finish()
        {
            Ok(lf) => {
                self.funds = lf;
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub fn find(
        &self,
        keyword: Option<String>,
        class: Option<Class>,
        situation: Option<Situation>,
        limit: Option<u32>,
    ) -> Result<DataFrame, PolarsError> {
        let mut filtered = self.funds.clone();

        if let Some(keyword) = keyword {
            filtered = filtered.filter(self.contains_normalized(keyword));
        }

        if let Some(class) = class {
            filtered = filtered.filter(col("CLASSE").eq(lit(class.to_string())));
        }

        let sit = situation.unwrap_or(Situation::Normal);
        filtered = filtered.filter(col("SIT").eq(lit(sit.to_string())));

        if let Some(limit) = limit {
            filtered = filtered.limit(limit);
        }

        filtered
            .sort("DENOM_SOCIAL", SortOptions::default())
            .collect()
    }

    pub fn find_by_cnpj(&self, cnpj: String) -> Result<DataFrame, PolarsError> {
        let filtered = self.funds.clone();
        if cnpj.is_empty() {
            return Err(PolarsError::NoData("CNPJ is empty".into()));
        }
        filtered
            .filter(col("CNPJ_FUNDO").eq(lit(cnpj)))
            .sort("DT_REG", SortOptions::default())
            .collect()
    }

    // Função para normalizar texto removendo acentos
    // NOTE: Egui não suporta Unicode completo, então é necessário normalizar certas palavras.
    // Por ex: "grão" vira "grao", "ações" - "acoes" etc...
    fn contains_normalized(&self, keyword: String) -> Expr {
        let q = format!("(?i){}", keyword);
        let re = Regex::new(r"\p{M}").unwrap();
        col("DENOM_SOCIAL")
            .apply(
                move |s: polars::prelude::Series| {
                    // Assuming "DENOM_SOCIAL" is a Utf8String type column
                    let utf8_series = s.utf8().expect("Expected Utf8String series");
                    // Normalize each string in the series
                    let normalized_series = utf8_series
                        .into_iter()
                        .map(|opt_str| {
                            opt_str.map(|s| {
                                let decomposed = s.nfkd().collect::<String>();
                                let t = re.replace_all(&decomposed, "").into_owned();
                                t
                            })
                        })
                        .collect::<polars::prelude::Utf8Chunked>();
                    // Convert the Utf8Chunked back to a Series
                    let result_series: polars::prelude::Series = normalized_series.into_series();
                    // Wrap the Series in Some and then Ok
                    Ok(Some(result_series))
                },
                GetOutput::default(),
            )
            .str()
            .contains(lit(q.clone()), false)
            .or(col("CNPJ_FUNDO").str().contains(lit(q), false))
    }

    pub fn download(
        &self,
        token: CancellationToken,
        mut on_progress: impl 'static + Send + FnMut(Download),
    ) {
        let options = self.options.clone();
        let request = ehttp::Request::get(options.url);

        on_progress(Download::InProgress("Baixando (0/1)...".to_string()));
        ehttp::fetch(request, move |on_done: Result<ehttp::Response, String>| {
            if token.is_cancelled() {
                on_progress(Download::Cancel);
                return;
            }

            on_progress(Download::InProgress("Baixando (0/1)...".to_string()));

            match on_done {
                Ok(res) => {
                    let (decoded_str, _, had_errors) = WINDOWS_1252.decode(&res.bytes);
                    if had_errors {
                        on_progress(Download::Cancel); // Or another appropriate error handling
                        return;
                    }

                    if Self::create_and_write_csv(&options.path, decoded_str.as_bytes()).is_err() {
                        on_progress(Download::Cancel); // Or another appropriate error handling
                        return;
                    }

                    on_progress(Download::InProgress("Baixando (1/1)...".to_string()));
                    on_progress(Download::Done);
                }
                Err(_) => {
                    on_progress(Download::Cancel); // Or another appropriate error handling
                }
            }
        });
    }

    fn create_and_write_csv<P: AsRef<Path>>(
        path: P,
        buf: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Create the directories if they do not exist
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        // Create the file and write the JSON data
        let mut file = File::create(&path)?;
        file.write_all(buf)?;
        Ok(())
    }
}
