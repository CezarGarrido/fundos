use chrono::NaiveDate;
use glob::glob;
pub mod options;
use options::{load, Options};
use polars::{
    datatypes::DataType,
    error::PolarsError,
    frame::DataFrame,
    lazy::{
        dsl::{col, concat, lit, StrptimeOptions},
        frame::LazyFrame,
    },
    prelude::{when, IntoLazy, SortOptions, UnionArgs},
};

use super::{align_columns, get_all_columns, read_csv_lazy};

#[derive(Clone)]
pub struct Informe {
    options: Options,
}

impl Informe {
    pub fn new() -> Self {
        let options = load().unwrap();
        Self { options }
    }

    pub async fn async_informes(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<LazyFrame, PolarsError> {
        let result = self
            .options
            .async_path(Some(start_date), Some(end_date))
            .await;
        match result {
            Ok(paths) => {
                let mut frames: Vec<LazyFrame> = Vec::new();
                for path in paths {
                    let pattern = format!("{}/*", path.display());
                    for path in glob(&pattern).unwrap().filter_map(Result::ok) {
                        let file = path.display().to_string();
                        let res = read_csv_lazy(&file);
                        match res {
                            Ok(mut lf) => {
                                lf = lf.select(&[
                                    col("CNPJ_FUNDO"),
                                    col("DT_COMPTC"),
                                    col("VL_QUOTA"),
                                ]);

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
                                    .filter(col("AS_DATE").gt_eq(lit(start_date)).and(
                                        col("AS_DATE").cast(DataType::Date).lt_eq(lit(end_date)),
                                    ))
                                    .collect()
                                    .unwrap()
                                    .lazy();

                                frames.push(lf)
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
                    .map(|lf| align_columns(lf, &all_columns))
                    .collect();

                // Concatenar em blocos se o número de LazyFrames for grande
                let result = if aligned_lfs.len() > 10 {
                    let mut concatenated = aligned_lfs[0].clone();
                    for chunk in aligned_lfs.chunks(10) {
                        let partial_concat = concat(chunk, UnionArgs::default())?;
                        concatenated =
                            concat(&[concatenated, partial_concat], UnionArgs::default())?;
                    }
                    Ok(concatenated)
                } else {
                    concat(&aligned_lfs, UnionArgs::default())
                };

                match result {
                    Ok(lf) => Ok(lf),
                    Err(err) => Err(err),
                }
            }
            Err(err) => Err(PolarsError::NoData(err.to_string().into())),
        }
    }

    pub async fn async_profit(
        &self,
        cnpj: String,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<DataFrame, PolarsError> {
        let res = self.async_informes(start_date, end_date).await.unwrap();
        // Ajustar a rentabilidade acumulada
        let cotas = res
            .filter(col("CNPJ_FUNDO").str().contains(lit(cnpj), false))
            .with_column(col("VL_QUOTA").cast(DataType::Float64).alias("valor_float"))
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
                    .alias("AS_DATE"),
            )
            .filter(
                col("AS_DATE")
                    .gt_eq(lit(start_date))
                    .and(col("AS_DATE").lt_eq(lit(end_date))),
            )
            .sort("AS_DATE", SortOptions::default())
            .collect()?;
        // Verificar se há dados suficientes para cálculo
        if cotas.height() == 0 {
            return Err(PolarsError::NoData(
                "Nenhum dado encontrado no intervalo".into(),
            ));
        }
        // Calcular rentabilidade diária e acumulada
        let df_with_rent_acc = cotas
            .lazy()
            .with_column(
                // Verificar se o valor anterior é diferente de zero antes de calcular a rentabilidade
                when(col("valor_float").shift(1).gt(lit(0.0)))
                    .then(col("valor_float") / col("valor_float").shift(1) - lit(1.0))
                    .otherwise(lit(0.0))
                    .fill_null(lit(0.0)) // Preencher valores nulos com 0
                    .alias("DAILY_RETURN"),
            )
            .with_column(
                (col("DAILY_RETURN") + lit(1.0))
                    .cumprod(false) // Produto acumulado
                    .alias("CUMULATIVE_PRODUCT"),
            )
            .with_column(((col("CUMULATIVE_PRODUCT") - lit(1.0)) * lit(100.0)).alias("RENT_ACUM"))
            //.drop("DAILY_RETURN") // Remover coluna intermediária, se desejado
            .sort("AS_DATE", SortOptions::default()) // Ordena por data
            .collect()?;

        log::info!("{:?}", df_with_rent_acc);
        Ok(df_with_rent_acc)
    }
}
