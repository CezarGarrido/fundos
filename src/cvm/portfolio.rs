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

use crate::cvm::{align_and_convert_columns_to_string, get_all_columns, read_csv_lazy};

#[derive(Clone)]
pub struct Portfolio {
    path: String,
}

impl Portfolio {
    pub fn new() -> Self {
        let path = "./dataset/cda/cda_fi_{year}{month}/cda*.csv".to_string();

        Self { path }
    }

    fn read_assets(&self, year: String, month: String) -> Result<LazyFrame, PolarsError> {
        let pattern = self
            .path
            .replace("{year}", &year.to_string())
            .replace("{month}", &month.to_string());

        let mut lfs = Vec::new();
        for path in glob(&pattern).unwrap().filter_map(Result::ok) {
            if path.is_file() {
                let file = path.display().to_string();
                if !file.contains("PL") {
                    log::info!("Reading file: {:?}", path.display());
                    let res = read_csv_lazy(&file);
                    match res {
                        Ok(lf) => lfs.push(lf),
                        Err(err) => {
                            log::info!("Error reading file: {:?} {}", path.display(), err);
                        }
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
                if let Ok(s) = pl.column("VL_PATRIM_LIQ") {
                    valor_pl = s.get(0).unwrap().get_str().unwrap().parse::<f64>().unwrap();
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
