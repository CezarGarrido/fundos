use chrono::NaiveDate;
use polars::{
    error::PolarsError,
    frame::DataFrame,
    io::SerReader,
    lazy::dsl::{col, lit, StrptimeOptions},
    prelude::{DataType, IntoLazy, JsonReader, SortOptions},
};
const PATH: &str = "./dataset/cdi/bcdata.sgs.12.json";

pub fn cdi(start_date: NaiveDate, end_date: NaiveDate) -> Result<DataFrame, PolarsError> {
    let mut file = std::fs::File::open(PATH)?;
    let res = JsonReader::new(&mut file).finish()?;

    // Calcular a rentabilidade diária acumulada
    let mut rent_acc = (col("cdi_decimal") + lit(1.0)).cumprod(false) - lit(1.0);
    rent_acc = (rent_acc * lit(100.0)).alias("cdi_acumulado");

    let df = res
        .lazy()
        .with_column(
            col("data")
                .str()
                .strptime(
                    DataType::Date,
                    StrptimeOptions {
                        format: Some("%d/%m/%Y".into()),
                        ..Default::default()
                    },
                )
                .alias("data_fmt"),
        )
        .with_column(col("valor").cast(DataType::Float64).alias("valor_float"))
        .filter(
            col("data_fmt")
                .cast(DataType::Date)
                .gt_eq(lit(start_date))
                .and(col("data_fmt").cast(DataType::Date).lt_eq(lit(end_date))),
        )
        .with_column((col("valor_float") / lit(100.0)).alias("cdi_decimal"))
        .with_column(rent_acc)
        .sort("data_fmt", SortOptions::default()) // Ordena por data
        .collect()?;
    Ok(df)
}
