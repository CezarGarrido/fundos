use std::sync::{Arc, Mutex};

use chrono::{Datelike, NaiveDate};
use ehttp::Request;

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
    prelude::{NamedFrom, SortOptions, UnionArgs},
    series::Series,
};

use tokio_util::sync::CancellationToken;

use crate::ui::download::Download;

use super::{align_columns, get_all_columns, read_csv_lazy, unzip_and_save};

#[derive(Clone)]
pub struct Informe {
    options: Options,
    path: String,
}

impl Informe {
    pub fn new() -> Self {
        let options = load().unwrap();
        let path = format!(
            "{}/{}",
            options.path.to_string_lossy(),
            "inf_diario_fi_{year}{month}.csv"
        );
        Self { path, options }
    }

    pub fn profitability(
        &self,
        cnpj: String,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<DataFrame, PolarsError> {
        let result_informes = self.read_informes(start_date, end_date);
        match result_informes {
            Ok(informes) => {
                let mut cotas = informes
                    .filter(col("CNPJ_FUNDO").str().contains(lit(cnpj), false))
                    .with_column(col("VL_QUOTA").cast(DataType::Float64))
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
                        col("AS_DATE")
                            .gt_eq(lit(start_date))
                            .and(col("AS_DATE").cast(DataType::Date).lt_eq(lit(end_date))),
                    )
                    .sort("AS_DATE", SortOptions::default())
                    .collect()?;

                // Obter a coluna de cotas
                let quotas = cotas.column("VL_QUOTA")?.f64()?;
                let mut rentabilidades_diarias = Vec::new();
                let mut prev_quota = None;
                let mut rentabilidade_acumulada = 1.0;
                let mut rentabilidades_acumuladas = Vec::new();

                for quota in quotas.into_iter() {
                    if let Some(prev) = prev_quota {
                        if let Some(current) = quota {
                            let rentabilidade_diaria = (current - prev) / prev;
                            rentabilidade_acumulada *= 1.0 + rentabilidade_diaria;
                            rentabilidades_diarias.push(rentabilidade_diaria);
                        }
                    } else {
                        rentabilidades_diarias.push(0.0); // Inicial para o primeiro valor
                    }
                    rentabilidades_acumuladas.push(rentabilidade_acumulada - 1.0); // Rentabilidade acumulada progressiva

                    prev_quota = quota;
                }
                let rentabilidades_acumuladas_percent: Vec<f64> = rentabilidades_acumuladas
                    .iter()
                    .map(|&r| r * 100.0)
                    .collect();

                // Criar Series para rentabilidade diária e acumulada
                let series_acumulada = Series::new("RENT_ACUM", rentabilidades_acumuladas_percent);
                cotas.with_column(series_acumulada)?;
                Ok(cotas)
            }
            Err(err) => {
                println!("errrror {}", err);
                Err(err)
            }
        }
    }

    fn files_glob(&self, start_date: NaiveDate, end_date: NaiveDate) -> Vec<LazyFrame> {
        let mut lfs = Vec::new();
        let mut errs = Vec::new();
        let patterns = self.generate_patterns(start_date, end_date, self.path.as_str());
        for pattern in patterns {
            for path in glob(&pattern).unwrap().filter_map(Result::ok) {
                if path.is_file() {
                    let file = path.display().to_string();
                    println!("file {}", file);
                    let res = read_csv_lazy(&file);
                    match res {
                        Ok(lf) => lfs.push(lf),
                        Err(err) => errs.push(err),
                    }
                }
            }
        }
        lfs
    }

    fn generate_patterns(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        path_template: &str,
    ) -> Vec<String> {
        let mut patterns = Vec::new();

        let mut current_date = start_date;
        while current_date <= end_date {
            // Formata o ano e o mês no formato desejado
            let year = current_date.year();
            let month = current_date.month();

            // Substitua os placeholders no template do caminho com o ano e o mês atuais
            let mut pattern = path_template.to_string();
            pattern = pattern.replace("{year}", &year.to_string());
            pattern = pattern.replace("{month}", &format!("{:02}", month));

            // Adiciona o padrão à lista
            patterns.push(pattern);

            // Avança para o próximo mês
            current_date = current_date
                .with_month(month + 1)
                .unwrap_or(NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap());
        }
        patterns.dedup();

        patterns
    }

    fn read_informes(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<LazyFrame, PolarsError> {
        let lfs = self.files_glob(start_date, end_date);
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

    pub fn download(
        &self,
        token: CancellationToken,
        on_progress: impl 'static + Send + FnMut(Download),
    ) {
        let options = self.options.clone();
        let urls = self.generate_patterns(options.start_date(), options.end_date(), &options.url);
        let total = urls.len();
        let on_progress = Arc::new(Mutex::new(on_progress));
        let completed_count = Arc::new(Mutex::new(0));
        let f: String = format!("Baixando (0/{})...", total);
        let mut on_progress1 = on_progress.lock().unwrap();
        on_progress1(Download::InProgress(f));

        for url in urls.iter() {
            let request = Request::get(url.to_string());
            let on_progress_clone = Arc::clone(&on_progress);
            let completed_count_clone = Arc::clone(&completed_count);
            let path = options.path.clone();
            let tk = token.clone();
            ehttp::fetch(request, move |on_done| {
                let mut completed = completed_count_clone.lock().unwrap();
                match on_done {
                    Ok(response) if response.ok => {
                        if tk.is_cancelled() {
                            let mut on_progress = on_progress_clone.lock().unwrap();
                            on_progress(Download::Cancel);
                            return;
                        }
                        if let Err(e) = unzip_and_save(&response.bytes, path.clone()) {
                            log::error!(
                                "Erro ao extrair arquivos CSV: {} {}",
                                e,
                                path.to_string_lossy()
                            );
                        }
                    }
                    Ok(response) => log::error!("Falha na requisição {:?}", response.status),
                    Err(err) => log::error!("Erro: {}", err),
                }

                *completed += 1;
                let progress_message = format!("Baixando ({}/{})", *completed, total);
                let mut on_progress = on_progress_clone.lock().unwrap();
                on_progress(Download::InProgress(progress_message));

                if *completed == total {
                    on_progress(Download::Done);
                }
            });
        }
    }
}
