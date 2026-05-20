use std::fmt;
pub mod options;

use once_cell::sync::Lazy;
use options::{load, Options};
use polars::{
    error::PolarsError,
    frame::DataFrame,
    lazy::dsl::{col, lit, Expr, GetOutput, StrptimeOptions},
    prelude::{DataType, LazyCsvReader, LazyFileListReader, LazyFrame, SortOptions},
    series::IntoSeries,
};
use std::sync::RwLock;

pub static LOADING_STATUS: Lazy<RwLock<String>> =
    Lazy::new(|| RwLock::new("Aguardando...".to_string()));

pub fn set_status(s: &str) {
    if let Ok(mut guard) = LOADING_STATUS.write() {
        *guard = s.to_string();
    }
}

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
            Class::Acoes => write!(f, "Ações"),
            Class::RendaFixa => write!(f, "Renda Fixa"),
            Class::Cambial => write!(f, "Cambial"),
            Class::MultiMarket => write!(f, "Multimercado"),
        }
    }
}

impl Register {
    pub fn new() -> Self {
        let options = load().unwrap();

        Self { options }
    }

    fn load_cadastro(&self, path: std::path::PathBuf) -> Result<LazyFrame, Error> {
        if path.is_dir() {
            set_status("Processando e indexando tabelas CVM 175 com Polars...");
            let path_fundo = path.join("registro_fundo.csv");
            let path_classe = path.join("registro_classe.csv");

            let lf_fundo = LazyCsvReader::new(&path_fundo)
                .has_header(true)
                .with_infer_schema_length(None)
                .with_delimiter(b';')
                .with_ignore_errors(true)
                .finish()?;

            let lf_classe = LazyCsvReader::new(&path_classe)
                .has_header(true)
                .with_infer_schema_length(None)
                .with_delimiter(b';')
                .with_ignore_errors(true)
                .finish()?;

            // Formatar CNPJ_Classe para o formato XX.XXX.XXX/XXXX-XX
            let cnpj_raw = col("CNPJ_Classe").cast(DataType::Utf8);
            let cnpj_str = polars::lazy::dsl::when(cnpj_raw.clone().str().lengths().eq(lit(13)))
                .then(lit("0") + cnpj_raw.clone())
                .otherwise(cnpj_raw.clone());

            let cnpj_formatted = cnpj_str.clone().str().str_slice(0, Some(2))
                + lit(".")
                + cnpj_str.clone().str().str_slice(2, Some(3))
                + lit(".")
                + cnpj_str.clone().str().str_slice(5, Some(3))
                + lit("/")
                + cnpj_str.clone().str().str_slice(8, Some(4))
                + lit("-")
                + cnpj_str.clone().str().str_slice(12, Some(2));

            // Seleciona APENAS as colunas necessárias de cada tabela para evitar conflitos no join
            // Da tabela classe: ID_Registro_Fundo (chave), CNPJ_Classe, Classificacao, e colunas auxiliares
            let classe_selected = lf_classe.clone().select([
                col("ID_Registro_Fundo"),
                col("CNPJ_Classe"),
                col("Classificacao").alias("CLASSE"),
                col("Classificacao_Anbima").alias("CLASSE_ANBIMA"),
                col("Data_Inicio").alias("DT_INI_ATIV"),
                col("Auditor").alias("AUDITOR"),
                col("CNPJ_Auditor").alias("CNPJ_AUDITOR"),
                col("Custodiante").alias("CUSTODIANTE"),
                col("Exclusivo").alias("FUNDO_EXCLUSIVO"),
                col("Publico_Alvo").alias("PUBLICO_ALVO"),
                col("Entidade_Investimento").alias("ENTID_INVEST"),
                col("Tributacao_Longo_Prazo").alias("TRIB_LPRAZO"),
                col("Permitido_Aplicacao_CemPorCento_Exterior").alias("INVEST_CEMPR_EXTER"),
            ]);

            // Da tabela fundo: ID_Registro_Fundo (chave) + dados que queremos exibir
            let fundo_selected = lf_fundo.clone().select([
                col("ID_Registro_Fundo"),
                col("Denominacao_Social").alias("DENOM_SOCIAL"),
                col("Situacao").str().to_uppercase().alias("SIT"),
                col("Data_Constituicao").alias("DT_CONST"),
                col("Tipo_Fundo").alias("TP_FUNDO"),
                col("Data_Registro").alias("DT_REG"),
                col("Codigo_CVM").alias("CD_CVM"),
                col("Data_Inicio_Situacao").alias("DT_INI_SIT"),
                col("Data_Cancelamento").alias("DT_CANCEL"),
                col("Diretor").alias("DIRETOR"),
                col("Administrador").alias("ADMIN"),
                col("CNPJ_Administrador").alias("CNPJ_ADMIN"),
                col("Gestor").alias("GESTOR"),
                col("CPF_CNPJ_Gestor").alias("CPF_CNPJ_GESTOR"),
                col("Tipo_Pessoa_Gestor").alias("PF_PJ_GESTOR"),
            ]);

            // Join usando as tabelas sem conflito de nomes
            let joined = classe_selected.left_join(
                fundo_selected,
                col("ID_Registro_Fundo"),
                col("ID_Registro_Fundo"),
            );

            // Mapeia o CNPJ da classe para o formato exibido
            let mapped = joined.with_column(cnpj_formatted.alias("CNPJ_FUNDO"));

            Ok(mapped)
        } else {
            set_status("Lendo cadastro clássico (cad_fi.csv)...");
            let lf = LazyCsvReader::new(&path)
                .has_header(true)
                .with_infer_schema_length(None)
                .with_delimiter(b';')
                .with_ignore_errors(true)
                .finish()?;
            Ok(lf)
        }
    }

    pub async fn async_find(
        &self,
        keyword: Option<String>,
        class: Option<Class>,
        situation: Option<Situation>,
        limit: Option<u32>,
    ) -> Result<DataFrame, Error> {
        let path = self.options.async_path().await?;
        let lf = self.load_cadastro(path)?;

        let mut filtered = lf.clone();

        if let Some(keyword) = keyword {
            let trimmed = keyword.trim();
            if !trimmed.is_empty() {
                filtered = filtered.filter(self.contains_normalized(trimmed.to_string()));
            }
        }

        if let Some(class) = class {
            let class_expr = match class {
                Class::RendaFixa => col("CLASSE")
                    .eq(lit("Renda Fixa"))
                    .or(col("CLASSE").eq(lit("Curto Prazo")))
                    .or(col("CLASSE").eq(lit("Referenciado")))
                    .or(col("CLASSE").eq(lit("Dívida Externa"))),
                Class::Acoes => col("CLASSE")
                    .eq(lit("Ações"))
                    .or(col("CLASSE").eq(lit("FMP-FGTS"))),
                Class::Cambial => col("CLASSE").eq(lit("Cambial")),
                Class::MultiMarket => col("CLASSE")
                    .eq(lit("Multimercado"))
                    .or(col("CLASSE").eq(lit("FIP Multi"))),
            };
            filtered = filtered.filter(class_expr);
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

        let lf = self.load_cadastro(path)?;

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
        let lf = self.load_cadastro(path)?;
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
