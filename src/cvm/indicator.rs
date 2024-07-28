use std::fs::File;

use chrono::{NaiveDate};
use polars::{
    error::PolarsError,
    frame::DataFrame,
    io::SerReader,
    lazy::dsl::{col, lit, StrptimeOptions},
    prelude::{DataType, IntoLazy, JsonReader, NamedFrom, SortOptions, TakeRandom},
    series::{Series},
};
//https://www.b3.com.br/data/files/A5/56/B2/36/245C5810F534EB48AC094EA8/IBOVDIA.zip

const CDI_PATH: &str = "./dataset/cdi/bcdata.sgs.12.json";
const IBOV_PATH: &str = "./dataset/ibov/ibov.json";

pub fn cdi(start_date: NaiveDate, end_date: NaiveDate) -> Result<DataFrame, PolarsError> {
    let mut file = std::fs::File::open(CDI_PATH)?;
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

pub fn get_ibovespa(start_date: NaiveDate, end_date: NaiveDate) -> Result<DataFrame, PolarsError> {
    let mut file = File::open(IBOV_PATH)?;
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

    println!("DataFrame após a filtragem e ordenação: {:?}", df);
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
