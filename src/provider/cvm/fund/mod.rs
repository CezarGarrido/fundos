use std::fmt;
pub mod options;

use options::{load, Options};
use polars::{
    error::PolarsError,
    frame::DataFrame,
    lazy::dsl::{col, lit, Expr, GetOutput, StrptimeOptions},
    prelude::{DataType, LazyCsvReader, LazyFileListReader, LazyFrame, SortOptions},
    series::IntoSeries,
};

use regex::Regex;
use thiserror::Error;
use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Error getting async path: {0}")]
    CachedPathError(#[from] cached_path::Error),

    #[error("Error loading CSV: {0}")]
    PolarsError(#[from] polars::prelude::PolarsError),
}

#[derive(Clone)]
pub struct Register {
    options: Options,
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

        Self { options }
    }

    pub async fn async_find(
        &self,
        keyword: Option<String>,
        class: Option<Class>,
        situation: Option<Situation>,
        limit: Option<u32>,
    ) -> Result<DataFrame, Error> {
        let path = self.options.async_path().await?;
        let lf = LazyCsvReader::new(&path)
            .has_header(true)
            .with_infer_schema_length(None)
            .with_delimiter(b';')
            // .with_cache(true)
            .finish()?;

        let mut filtered = lf.clone();

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

        let res = filtered
            .sort("DENOM_SOCIAL", SortOptions::default())
            .collect()?;
        Ok(res)
    }

    pub async fn async_find_by_cnpj(
        &self,
        cnpj: String,
        offline: bool,
    ) -> Result<DataFrame, Error> {
        let path = if offline {
            self.options.async_path_offline().await?
        } else {
            self.options.async_path().await?
        };

        let lf = LazyCsvReader::new(&path)
            .has_header(true)
            .with_infer_schema_length(None)
            .with_delimiter(b';')
            .with_cache(true)
            .finish()?;

        let res = lf
            .filter(col("CNPJ_FUNDO").eq(lit(cnpj)))
            .sort("DT_REG", SortOptions::default())
            .collect()?;

        Ok(res)
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

    pub async fn async_stats(&self) -> Result<(DataFrame, DataFrame, DataFrame), Error> {
        let path = self.options.async_path().await?;
        let lf = LazyCsvReader::new(&path)
            .has_header(true)
            .with_infer_schema_length(None)
            .with_delimiter(b';')
            .with_cache(true)
            .finish()?;
        // Chama as funções para obter os DataFrames desejados
        let by_year = self.count_funds_by_year(lf.clone())?;
        let by_status = self.count_funds_by_status(lf.clone())?;
        let by_class = self.count_funds_by_class(lf.clone())?;

        Ok((by_year, by_status, by_class))
    }

    pub fn count_funds_by_year(&self, fund_lazyframe: LazyFrame) -> Result<DataFrame, PolarsError> {
        let expr = fund_lazyframe
            .with_column(
                col("DT_CONST")
                    .str()
                    .strptime(
                        DataType::Date,
                        StrptimeOptions {
                            format: Some("%Y-%m-%d".into()),
                            ..Default::default()
                        },
                    )
                    .alias("DT_CONST_DATE"),
            )
            .with_column(col("DT_CONST_DATE").dt().year().alias("Ano"))
            .groupby(vec![col("Ano")])
            .agg(vec![col("Ano").count().alias("Quant")]);

        expr.collect()
    }

    pub fn count_funds_by_status(
        &self,
        fund_lazyframe: LazyFrame,
    ) -> Result<DataFrame, PolarsError> {
        fund_lazyframe
            .groupby(vec![col("SIT")])
            .agg(vec![col("TP_FUNDO").count()])
            .sort("TP_FUNDO", Default::default())
            .collect()
    }

    pub fn count_funds_by_class(
        &self,
        fund_lazyframe: LazyFrame,
    ) -> Result<DataFrame, PolarsError> {
        fund_lazyframe
            .groupby(vec![col("CLASSE")])
            .agg(vec![col("TP_FUNDO").count()])
            .sort("TP_FUNDO", Default::default())
            .collect()
    }
}
