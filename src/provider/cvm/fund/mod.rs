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
use unicode_normalization::UnicodeNormalization;

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

    pub fn stats(&self) -> Result<(DataFrame, DataFrame, DataFrame), PolarsError> {
        let filtered = self.funds.clone();
        // Chama as funções para obter os DataFrames desejados
        let by_year = self.count_funds_by_year(filtered.clone())?;
        let by_status = self.count_funds_by_status(filtered.clone())?;
        let by_class = self.count_funds_by_class(filtered.clone())?;

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
