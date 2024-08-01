use chrono::{Datelike, NaiveDate};
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
use std::{
    fs::{self, File},
    io::BufWriter,
    path::Path,
};
use tokio::time::sleep;
use yahoo_finance_api::{
    time::{Date, Month, OffsetDateTime, Time},
    YahooConnector,
};

use serde::{Deserialize, Serialize};

use tokio_util::sync::CancellationToken;

use crate::ui::download::Download;

pub fn dataframe(start_date: NaiveDate, end_date: NaiveDate) -> Result<DataFrame, PolarsError> {
    let opts = load().unwrap();
    let mut file = File::open(opts.path)?;
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

pub fn download(token: CancellationToken, mut on_progress: impl 'static + Send + FnMut(Download)) {
    let options = load().unwrap();
    let provider = YahooConnector::new().unwrap();

    let start_date = options.start_date();
    let end_date = options.end_date();

    let start = OffsetDateTime::new_utc(
        Date::from_calendar_date(
            start_date.year(),
            Month::try_from(start_date.month() as u8).unwrap(),
            start_date.day() as u8,
        )
        .unwrap(),
        Time::from_hms_nano(0, 0, 0, 0).unwrap(),
    );
    let end = OffsetDateTime::new_utc(
        Date::from_calendar_date(end_date.year(), Month::December, end_date.day() as u8).unwrap(),
        Time::from_hms_nano(0, 0, 0, 0).unwrap(),
    );

    let path = options.path.clone();
    let h = async move {
        if token.is_cancelled() {
            on_progress(Download::Cancel);
            return;
        }

        let resp = provider
            .get_quote_history("^BVSP", start, end)
            .await
            .unwrap();

        let quotes: Vec<yahoo_finance_api::Quote> = resp.quotes().unwrap();
        let total_quotes = quotes.len() as f64;
        let mut ibovs = Vec::new();

        on_progress(Download::InProgress(format!(
            "Baixando (0/{})",
            total_quotes
        )));

        for (i, q) in quotes.into_iter().enumerate() {
            // Adiciona um delay para simular carga de trabalho e permitir o cancelamento
            sleep(tokio::time::Duration::from_millis(50)).await;
            if token.is_cancelled() {
                on_progress(Download::Cancel);
                return;
            }
            let ibov = Ibov {
                timestamp: q.timestamp,
                adjclose: q.adjclose,
                date: chrono::DateTime::from_timestamp(q.timestamp as i64, 0)
                    .unwrap()
                    .format("%d/%m/%Y")
                    .to_string(),
                open: q.open,
                high: q.high,
                low: q.low,
                volume: q.volume,
                close: q.close,
            };
            ibovs.push(ibov);

            let progress = i as f64 + 1.0;

            if token.is_cancelled() {
                on_progress(Download::Cancel);
                return;
            }

            on_progress(Download::InProgress(format!(
                "Baixando ({}/{})",
                progress, total_quotes
            )));
        }
        if token.is_cancelled() {
            on_progress(Download::Cancel);
            return;
        }

        create_and_write_json(&path, &ibovs).unwrap();
        on_progress(Download::Done);
    };
    tokio::spawn(h);
}

fn create_and_write_json<P: AsRef<Path>, T: serde::Serialize>(
    path: P,
    data: &T,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create the directories if they do not exist
    if let Some(parent) = path.as_ref().parent() {
        fs::create_dir_all(parent)?;
    }

    // Create the file and write the JSON data
    let file = File::create(&path)?;
    let writer = BufWriter::new(file);
    serde_json::to_writer_pretty(writer, data)?;
    Ok(())
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
