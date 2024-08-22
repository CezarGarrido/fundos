use chrono::{Datelike, NaiveDate};
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
    prelude::{IntoLazy, NamedFrom, SortOptions, UnionArgs},
    series::Series,
};

use super::{align_columns, get_all_columns, read_csv_lazy};

#[derive(Clone)]
pub struct Informe {
    path: String,
    options: Options,
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
                let mut frames = Vec::new();
                for path in paths {
                    let pattern = format!("{}/*", path.display());
                    //    println!("pattern {}", pattern.clone());
                    for path in glob(&pattern).unwrap().filter_map(Result::ok) {
                        let file = path.display().to_string();
                        println!("file {}", file.clone());

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
                                    )).collect().unwrap().lazy();

                                frames.push(lf)
                            }
                            Err(err) => {
                                println!("err {}", err);
                            }
                        }
                    }
                }
                let all_columns = get_all_columns(&frames);

                let aligned_lfs: Vec<LazyFrame> = frames
                    .into_iter()
                    .map(|lf| align_columns(lf, &all_columns))
                    .collect();

                println!("all {:?}", all_columns);

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
                    Ok(lf) => {
                        println!("ok");
                        Ok(lf)
                    }
                    Err(err) => {
                        println!("concat error aqui {}", err);
                        Err(err)
                    }
                }
            }
            Err(err) => {
                println!("errors  {}", err);
                return Err(PolarsError::NoData(
                    "No CSV files found or all failed to read".into(),
                ));
            }
        }
    }

    pub async fn async_profit1(
        &self,
        cnpj: String,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<DataFrame, PolarsError> {
        let result_informes = self.async_informes(start_date, end_date).await;

        match result_informes {
            Ok(informes) => {
                println!("Calculando....");
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
                println!("Fim Calculando....");

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

    pub async fn async_profit(
        &self,
        cnpj: String,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<DataFrame, PolarsError> {
        let res = self.async_informes(start_date, end_date).await.unwrap();
        // Ajustar a rentabilidade acumulada

        let mut cotas = res
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
        // Calcular rentabilidade diária e acumulada
        let df_with_rent_acc = cotas
            .lazy()
            .with_column(
                (col("valor_float") / col("valor_float").shift(1) - lit(1.0))
                    .fill_null(lit(0.0)) // Preencher Nones com 0.0 para o primeiro valor
                    .fill_nan(lit(0.0)) // Preencher Nones com 0.0 para o primeiro valor
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

        println!("Fim Calculando....");
        Ok(df_with_rent_acc)
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
}
