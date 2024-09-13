use chrono::NaiveDate;
use glob::glob;

use options::Options;
use polars::{
    datatypes::DataType,
    error::PolarsError,
    frame::DataFrame,
    lazy::{
        dsl::{col, concat, lit},
        frame::LazyFrame,
    },
    prelude::{IntoLazy, StrptimeOptions, UnionArgs},
};
pub mod options;

use super::{align_and_convert_columns_to_string, get_all_columns, read_csv_lazy};

#[derive(Clone)]
pub struct Portfolio {
    options: Options,
}

impl Portfolio {
    pub fn new() -> Self {
        let options = options::load().unwrap();
        Self { options }
    }

    pub async fn async_read_assets(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<(LazyFrame, LazyFrame), PolarsError> {
        let result = self
            .options
            .async_path(Some(start_date), Some(end_date))
            .await;
        match result {
            Ok(paths) => {
                let mut frames = Vec::new();
                let mut pls = Vec::new();
                for path in paths {
                    let pattern = format!("{}/*", path.display());
                    for path in glob(&pattern).unwrap().filter_map(Result::ok) {
                        let file = path.display().to_string();
                        let res = read_csv_lazy(&file);
                        match res {
                            Ok(lf) => {
                                if file.contains("PL") {
                                    pls.push(lf)
                                } else {
                                    let lf = lf
                                        .with_column(
                                            col("DT_COMPTC")
                                                .str()
                                                .strptime(
                                                    DataType::Date,
                                                    StrptimeOptions {
                                                        format: Some("%Y-%m-%d".into()),
                                                        ..Default::default()
                                                    },
                                                )
                                                .cast(DataType::Date)
                                                .alias("AS_DATE"),
                                        )
                                        .filter(
                                            col("AS_DATE").gt_eq(lit(start_date)).and(
                                                col("AS_DATE")
                                                    .cast(DataType::Date)
                                                    .lt_eq(lit(end_date)),
                                            ),
                                        )
                                        .collect()
                                        .unwrap()
                                        .lazy();
                                    frames.push(lf)
                                }
                            }
                            Err(err) => {
                                log::error!("err {}", err);
                            }
                        }
                    }
                }

                let all_columns = get_all_columns(&frames);
                let aligned_lfs: Vec<LazyFrame> = frames
                    .into_iter()
                    .map(|lf| align_and_convert_columns_to_string(lf, &all_columns))
                    .collect();

                let result = concat(&aligned_lfs, UnionArgs::default());
                match result {
                    Ok(lf) => {
                        if pls.is_empty() {
                            Ok((lf, LazyFrame::default()))
                        } else {
                            let pl = pls.first().unwrap().clone();
                            Ok((lf, pl))
                        }
                    }
                    Err(err) => Err(err),
                }
            }
            Err(err) => Err(PolarsError::NoData(err.to_string().into())),
        }
    }

    fn get_month_start_and_end(
        &self,
        month: String,
        year: String,
    ) -> Result<(NaiveDate, NaiveDate), String> {
        // Tentar converter os valores de string para u32 e i32
        let month: u32 = month.parse().map_err(|_| "Invalid month format")?;
        let year: i32 = year.parse().map_err(|_| "Invalid year format")?;

        // Verificar se o mês está no intervalo válido
        if !(1..=12).contains(&month) {
            return Err("Month must be between 1 and 12".into());
        }

        // Criar a data de início (primeiro dia do mês)
        let start_date = NaiveDate::from_ymd_opt(year, month, 1).unwrap();

        // Calcular o último dia do mês
        let end_date = match month {
            12 => NaiveDate::from_ymd_opt(year + 1, 1, 1)
                .unwrap()
                .pred_opt()
                .unwrap(), // Janeiro do ano seguinte
            _ => NaiveDate::from_ymd_opt(year, month + 1, 1)
                .unwrap()
                .pred_opt()
                .unwrap(), // Último dia do mês atual
        };

        Ok((start_date, end_date))
    }

    pub async fn async_assets(
        &self,
        cnpj: String,
        year: String,
        month: String,
        top: bool,
    ) -> Result<(DataFrame, DataFrame, DataFrame), PolarsError> {
        let (start_date, end_date) = self.get_month_start_and_end(month, year).unwrap();
        let res = self
            .async_read_assets(start_date.to_owned(), end_date.to_owned())
            .await;

        match res {
            Ok((lf, pl)) => {
                let mut valor_pl = 0.0;
                let pl = pl
                    .filter(col("CNPJ_FUNDO").eq(lit(cnpj.clone())))
                    .collect()?;

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
                if top {
                    let res = self.top_assets(assets.clone().lazy(), cnpj.clone());
                    match res {
                        Ok(top_assets) => return Ok((pl.clone(), assets.clone(), top_assets)),
                        Err(_) => return Ok((pl, assets, DataFrame::empty())),
                    }
                };

                Ok((pl, assets, DataFrame::empty()))
            }
            Err(err) => Err(err),
        }
    }

    pub fn top_assets(&self, lf: LazyFrame, cnpj: String) -> Result<DataFrame, PolarsError> {
        let assets = lf
            .filter(col("CNPJ_FUNDO").str().contains(lit(cnpj), false))
            .with_column(col("VL_MERC_POS_FINAL").cast(DataType::Float64))
            .with_column(col("VL_PORCENTAGEM_PL").cast(DataType::Float64))
            //.select(&[
            //      col("TP_APLIC"),
            //      col("VL_MERC_POS_FINAL").cast(DataType::Float64),
            //       col("VL_PORCENTAGEM_PL"),
            //   ])
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
        Ok(top_assets)
    }
}
