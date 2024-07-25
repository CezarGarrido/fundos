use chrono::NaiveDate;
use glob::glob;

use polars::{
    datatypes::{DataType, Float64Chunked},
    error::PolarsError,
    frame::DataFrame,
    lazy::{
        dsl::{col, concat, lit},
        frame::LazyFrame,
    },
    prelude::{ChunkCumAgg, ChunkShift, UnionArgs},
    series::IntoSeries,
};

use crate::cvm::{align_columns, extract_date_from_filename, get_all_columns, read_csv_lazy};

#[derive(Clone)]
pub struct Informe {
    path: String,
}

impl Informe {
    pub fn new() -> Self {
        let path = "./dataset/infdiario/inf_diario_fi_*/*.csv".to_string();
        Self { path }
    }

    pub fn profitability(
        &self,
        cnpj: String,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<DataFrame, PolarsError> {
        let res = self.read_informes(start_date, end_date);
        match res {
            Ok(lf) => {
                let start_date_str = start_date.format("%Y-%m-%d").to_string();
                let end_date_str = end_date.format("%Y-%m-%d").to_string();
                let cotas = lf
                    .filter(col("CNPJ_FUNDO").str().contains(lit(cnpj), false))
                    .filter(col("DT_COMPTC").gt_eq(lit(start_date_str)))
                    .filter(col("DT_COMPTC").lt_eq(lit(end_date_str)))
                    .collect()
                    .unwrap();

                let q = cotas
                    .column("VL_QUOTA")
                    .unwrap()
                    .cast(&DataType::Float64)
                    .unwrap();

                let vl_quota = q.f64()?;
                let shifted = vl_quota.shift(1);

                let rent = vl_quota
                    .into_iter()
                    .zip(shifted.into_iter())
                    .map(|(current, previous)| match (current, previous) {
                        (Some(current), Some(previous)) => Some(current / previous - 1.0),
                        _ => Some(0.0),
                    })
                    .collect::<Float64Chunked>()
                    .into_series();

                let mut rent_acum = rent
                    .clone()
                    .f64()?
                    .cumsum(false)
                    .into_iter()
                    .map(|opt| opt.map(|v| v * 100.0))
                    .collect::<Float64Chunked>()
                    .into_series();

                let mut cotas = cotas.clone();
                //cotas.with_column(rent.rename("RENT").to_owned())?;
                cotas.with_column(rent_acum.rename("RENT_ACUM").to_owned())?;
                Ok(cotas)
            }
            Err(err) => Err(err),
        }
    }

    fn find(&self, start_date: NaiveDate, end_date: NaiveDate) -> Vec<LazyFrame> {
        glob(&self.path)
            .unwrap()
            .filter_map(|entry| match entry {
                Ok(path) => {
                    let file_name = path.file_name()?.to_str()?;
                    let file_date = extract_date_from_filename(file_name)?;
                    if file_date >= start_date && file_date <= end_date {
                        read_csv_lazy(path.to_str()?).ok()
                    } else {
                        None
                    }
                }
                Err(_) => None,
            })
            .collect()
    }

    fn read_informes(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<LazyFrame, PolarsError> {
        let lfs = self.find(start_date, end_date);
        if lfs.is_empty() {
            return Err(PolarsError::NoData(
                "No CSV files found or all failed to read".into(),
            ));
        }

        let all_columns = get_all_columns(&lfs);
        let aligned_lfs: Vec<LazyFrame> = lfs
            .into_iter()
            .map(|lf| align_columns(lf, &all_columns))
            .collect();

        let concatenated_lf = concat(&aligned_lfs, UnionArgs::default())?;
        let concatenated_lf = concatenated_lf.cache();
        Ok(concatenated_lf)
    }
}

/* Calcular a rentabilidade acumulada
 let initial_value = cotas
     .column("VL_QUOTA")
     .unwrap()
     .f64()
     .unwrap()
     .get(0)
     .unwrap();
 let final_value = cotas
     .column("VL_QUOTA")
     .unwrap()
     .f64()
     .unwrap()
     .get(cotas.height() - 1)
     .unwrap();
// let rentabilidade_ac umulada = (final_value / initial_value - 1.0) * 100.0;
// println!("Rentabilidade acumulada no período: {:.2}%", rentabilidade_acumulada);

*/
