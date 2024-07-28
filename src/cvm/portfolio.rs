use crate::cvm::{align_and_convert_columns_to_string, get_all_columns, read_csv_lazy};
use glob::glob;
use polars::{
    datatypes::DataType,
    error::PolarsError,
    frame::DataFrame,
    lazy::{
        dsl::{col, concat, lit},
        frame::LazyFrame,
    },
    prelude::{IntoLazy, UnionArgs},
};

const PORTFOLIO_PATH: &str = "./dataset/cda/";

#[derive(Clone)]
pub struct Portfolio {
    path: String,
}

impl Portfolio {
    pub fn new() -> Self {
        let path = format!("{}{}", PORTFOLIO_PATH, "cda_fi_{year}{month}/cda*.csv");
        Self { path }
    }

    fn read_assets(&self, year: String, month: String) -> Result<LazyFrame, PolarsError> {
        let pattern = self
            .path
            .replace("{year}", &year.to_string())
            .replace("{month}", &month.to_string());

        let mut lfs = Vec::new();
        let mut errs = Vec::new();
        for path in glob(&pattern).unwrap().filter_map(Result::ok) {
            if path.is_file() {
                let file = path.display().to_string();
                if !file.contains("PL") {
                    let res = read_csv_lazy(&file);
                    match res {
                        Ok(lf) => lfs.push(lf),
                        Err(err) => errs.push(err),
                    }
                }
            }
        }

        if lfs.is_empty() {
            return Err(PolarsError::NoData(
                "No CSV files found or all failed to read".into(),
            ));
        }

        let all_columns = get_all_columns(&lfs);
        let aligned_lfs: Vec<LazyFrame> = lfs
            .into_iter()
            .map(|lf| align_and_convert_columns_to_string(lf, &all_columns))
            .collect();

        let concatenated_lf = concat(&aligned_lfs, UnionArgs::default())?;
        let concatenated_lf = concatenated_lf.cache();
        Ok(concatenated_lf)
    }

    pub fn patrimonio_liquido(
        &self,
        cnpj: String,
        year: String,
        month: String,
    ) -> Result<DataFrame, PolarsError> {
        let pattern = format!("./dataset/cda/cda_fi_{}{}/cda_fi_PL_*.csv", year, month);

        let lfs: Vec<LazyFrame> = glob(&pattern)
            .unwrap()
            .filter_map(|entry| match entry {
                Ok(path) => {
                    if let Some(str) = path.to_str() {
                        if path.is_file() {
                            log::info!("Reading file: {:?}", path.display());
                            let res = read_csv_lazy(str);
                            match res {
                                Ok(lf) => Some(lf),
                                Err(_) => None,
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                Err(e) => {
                    eprintln!("Error accessing entry: {:?}", e);
                    None
                }
            })
            .collect();

        if lfs.is_empty() {
            return Err(PolarsError::NoData(
                "No CSV files found or all failed to read".into(),
            ));
        }

        let lf = lfs.first().unwrap().clone();

        let df = lf
            .filter(col("CNPJ_FUNDO").str().contains(lit(cnpj), false))
            .collect()
            .unwrap();

        Ok(df)
    }

    pub fn assets(
        &self,
        pl: DataFrame,
        cnpj: String,
        year: String,
        month: String,
        top: bool,
    ) -> Result<(DataFrame, DataFrame), PolarsError> {
        let res = self.read_assets(year.to_owned(), month.to_owned());

        match res {
            Ok(lf) => {
                let mut valor_pl = 0.0;
                if let Some(parsed_value) = pl
                    .column("VL_PATRIM_LIQ")
                    .ok()
                    .and_then(|col| col.get(0).ok())
                    .and_then(|val| val.get_str().map(|s| s.to_string()))
                    .and_then(|value_str| value_str.parse::<f64>().ok())
                {
                    valor_pl = parsed_value;
                }

                let assets = lf
                    .filter(col("CNPJ_FUNDO").eq(lit(cnpj.clone())))
                    .with_column(
                        (col("VL_MERC_POS_FINAL").cast(DataType::Float64) / lit(valor_pl)
                            * lit(100.0))
                        .round(3)
                        .alias("VL_PORCENTAGEM_PL"),
                    )
                    .collect()
                    .unwrap();
                println!("assets {:?}", assets);

                if top {
                    let res = self.top_assets(assets.clone().lazy(), cnpj.clone());
                    match res {
                        Ok(top_assets) => return Ok((assets.clone(), top_assets)),
                        Err(_) => return Ok((assets, DataFrame::empty())),
                    }
                };

                Ok((assets, DataFrame::empty()))
            }
            Err(err) => Err(err),
        }
    }

    pub fn top_assets(&self, lf: LazyFrame, cnpj: String) -> Result<DataFrame, PolarsError> {
        let assets = lf
            .filter(col("CNPJ_FUNDO").str().contains(lit(cnpj), false))
            .select(&[
                col("TP_APLIC"),
                col("VL_MERC_POS_FINAL").cast(DataType::Float64),
                col("VL_PORCENTAGEM_PL"),
            ])
            .groupby(vec![col("TP_APLIC")]);

        let top_assets = assets
            .agg([
                col("VL_MERC_POS_FINAL").sum(),
                col("VL_PORCENTAGEM_PL").sum(),
            ])
            .sort(
                "VL_MERC_POS_FINAL",
                polars::prelude::SortOptions {
                    descending: true,
                    ..Default::default()
                },
            )
            .collect()
            .unwrap();

        println!("{:?}", top_assets);
        Ok(top_assets)
    }
}
