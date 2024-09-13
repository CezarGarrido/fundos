use chrono::NaiveDate;
pub mod options;

use options::load;
use polars::{
    frame::DataFrame,
    io::SerReader,
    lazy::dsl::{col, lit, StrptimeOptions},
    prelude::{DataType, IntoLazy, JsonReader, SortOptions},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Error getting async path: {0}")]
    CachedPath(#[from] cached_path::Error),

    #[error("Error loading JsonReader: {0}")]
    Polars(#[from] polars::prelude::PolarsError),

    #[error("Error loading JSON file: {0}")]
    Io(#[from] std::io::Error),
}

pub async fn async_dataframe(
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<DataFrame, Error> {
    let options = load().unwrap();
    let path = options.async_path(start_date, end_date).await?;
    let mut file = std::fs::File::open(path)?;
    let res = JsonReader::new(&mut file).finish()?;
    // Calcular a rentabilidade diária acumulada
    let mut rent_acc = (col("cdi_decimal") + lit(1.0)).cumprod(false) - lit(1.0);
    rent_acc = (rent_acc * lit(100.0)).alias("value");

    let df = res
        .lazy()
        .with_column(col("data").alias("date"))
        .with_column(
            col("date")
                .str()
                .strptime(
                    DataType::Date,
                    StrptimeOptions {
                        format: Some("%d/%m/%Y".into()),
                        ..Default::default()
                    },
                )
                .alias("as_date"),
        )
        .with_column(col("valor").cast(DataType::Float64).alias("valor_float"))
        .filter(
            col("as_date")
                .cast(DataType::Date)
                .gt_eq(lit(start_date))
                .and(col("as_date").cast(DataType::Date).lt_eq(lit(end_date))),
        )
        .with_column((col("valor_float") / lit(100.0)).alias("cdi_decimal"))
        .with_column(rent_acc)
        .sort("as_date", SortOptions::default()) // Ordena por data
        .collect()?;
    Ok(df)
}
