use chrono::NaiveDate;
pub mod options;

use options::load;
use polars::{
    error::PolarsError,
    frame::DataFrame,
    io::SerReader,
    lazy::dsl::{col, lit, StrptimeOptions},
    prelude::{DataType, IntoLazy, JsonReader, NamedFrom, SortOptions, TakeRandom},
    series::Series,
};
use std::fs::File;

use serde::{Deserialize, Serialize};

pub async fn async_dataframe(
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<DataFrame, PolarsError> {
    let opts = load().unwrap();

    let path = opts.async_path(start_date, end_date).await.unwrap();

    let mut file = File::open(path)?;
    let df = JsonReader::new(&mut file).finish()?;
    // Adicionar coluna de datas formatadas e filtrar por data
    let mut df = df
        .lazy()
        .with_column(col("adjclose").cast(DataType::Float64).alias("adjclose"))
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
                .cast(DataType::Date)
                .alias("AS_DATE"),
        )
        .filter(
            col("AS_DATE")
                .gt_eq(lit(start_date))
                .and(col("AS_DATE").cast(DataType::Date).lt_eq(lit(end_date))),
        )
        .sort("AS_DATE", SortOptions::default())
        .collect()?;

    // Adicionando coluna de rentabilidade percentual
    let close_series = df.column("adjclose")?.f64()?;
    let mut rentabilidade_percentual = Vec::new();

    for i in 1..close_series.len() {
        let rentabilidade = (close_series.get(i).unwrap() - close_series.get(i - 1).unwrap())
            / close_series.get(i - 1).unwrap()
            * 100.0;
        rentabilidade_percentual.push(rentabilidade);
    }
    rentabilidade_percentual.insert(0, 0.0);
    let rentabilidade_series = Series::new("rentabilidade", &rentabilidade_percentual);
    df.with_column(rentabilidade_series)?;

    let mut rentabilidade_acumulada = Vec::new();
    let mut acumulado = 1.0;

    for &valor in &rentabilidade_percentual {
        acumulado *= 1.0 + valor / 100.0;
        rentabilidade_acumulada.push((acumulado - 1.0) * 100.0);
    }
    // Adicionando a coluna de rentabilidade acumulada ao DataFrame
    let acumulada_series = Series::new("value", &rentabilidade_acumulada);
    df.with_column(acumulada_series)?;
    Ok(df)
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Deserialize, Serialize)]
pub struct Ibov {
    pub date: String,
    pub timestamp: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub volume: u64,
    pub close: f64,
    pub adjclose: f64,
}
