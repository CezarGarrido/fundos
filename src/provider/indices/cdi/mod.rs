use std::{
    fs::{self, File},
    io::Write,
    path::Path,
};

use chrono::NaiveDate;

pub mod options;

use options::load;
use polars::{
    error::PolarsError,
    frame::DataFrame,
    io::SerReader,
    lazy::dsl::{col, lit, StrptimeOptions},
    prelude::{DataType, IntoLazy, JsonReader, SortOptions},
};
use tokio_util::sync::CancellationToken;

use crate::ui::download::Download;

pub fn dataframe(start_date: NaiveDate, end_date: NaiveDate) -> Result<DataFrame, PolarsError> {
    let options = load().unwrap();

    let mut file = std::fs::File::open(options.path.clone())?;
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

pub fn download(token: CancellationToken, mut on_progress: impl 'static + Send + FnMut(Download)) {
    let options = load().unwrap();
    let request = ehttp::Request::get(options.url);
    on_progress(Download::InProgress("Baixando (0/1)...".to_string()));
    ehttp::fetch(request, move |on_done: Result<ehttp::Response, String>| {
        if token.is_cancelled() {
            on_progress(Download::Cancel);
            return;
        }

        on_progress(Download::InProgress("Baixando (0/1)...".to_string()));

        match on_done {
            Ok(res) => {
                if create_and_write_csv(&options.path, &res.bytes).is_err() {
                    on_progress(Download::Cancel); // Or another appropriate error handling
                    return;
                }

                on_progress(Download::InProgress("Baixando (1/1)...".to_string()));
                on_progress(Download::Done);
            }
            Err(_) => {
                on_progress(Download::Cancel); // Or another appropriate error handling
            }
        }
    });
}

fn create_and_write_csv<P: AsRef<Path>>(
    path: P,
    buf: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    // Create the directories if they do not exist
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    // Create the file and write the JSON data
    let mut file = File::create(&path)?;
    file.write_all(buf)?;
    Ok(())
}
