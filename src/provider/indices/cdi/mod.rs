use std::{
    fs::{self, File},
    io::BufWriter,
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
    let mut file = std::fs::File::open(options.path)?;
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
    println!("url {}", options.url);
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
                if res.ok {
                    if token.is_cancelled() {
                        on_progress(Download::Cancel);
                        return;
                    }

                    let text = res.text().unwrap_or_default();
                    if create_and_write_json(&options.path, text).is_err() {
                        on_progress(Download::Cancel); // Or another appropriate error handling
                        return;
                    }
                    on_progress(Download::InProgress("Baixando (1/1)...".to_string()));
                } else {
                    log::error!("Falha {}", res.status_text);
                }
            }
            Err(msg) => {
                log::error!("Falha {}", msg);
            }
        }

        on_progress(Download::Done);
    });
}

fn create_and_write_json<P: AsRef<Path>>(
    path: P,
    data: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Cria os diretórios se não existirem
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }
    // Cria o arquivo e escreve o JSON
    let file = File::create(&path)?;
    let writer = BufWriter::new(file);
    let json_value: serde_json::Value = serde_json::from_str(data)?;
    serde_json::to_writer_pretty(writer, &json_value)?;
    Ok(())
}
