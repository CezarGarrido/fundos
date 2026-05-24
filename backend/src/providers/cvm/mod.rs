use std::{
    fs::{self, remove_file, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use cached_path::Cache;
use encoding_rs::WINDOWS_1252;

use polars::{
    error::PolarsError,
    lazy::dsl::{col, lit, Expr},
    prelude::{DataType, LazyCsvReader, LazyFileListReader, LazyFrame, NULL},
};

use tokio::sync::Semaphore;

pub mod fund;
pub mod informe;
pub mod portfolio;

fn read_csv_lazy(file_path: &str) -> Result<LazyFrame, PolarsError> {
    LazyCsvReader::new(file_path)
        .has_header(true)
        .with_infer_schema_length(Some(0))
        .with_delimiter(b';')
        .with_ignore_errors(true)
        .with_cache(false)
        .with_missing_is_null(true)
        .finish()
}

fn get_all_columns(lfs: &[LazyFrame]) -> Vec<String> {
    let mut columns = std::collections::HashSet::new();
    for lf in lfs {
        if let Ok(schema) = lf.schema() {
            for field in schema.iter_fields() {
                columns.insert(field.name().clone());
            }
        }
    }
    columns.into_iter().map(|s| s.to_string()).collect()
}

fn align_columns(lf: LazyFrame, all_columns: &[String]) -> LazyFrame {
    let mut new_lf = lf;
    for column in all_columns {
        if !new_lf
            .schema()
            .unwrap()
            .iter_fields()
            .any(|f| f.name() == column)
        {
            new_lf = new_lf.with_column(lit(NULL).alias(column));
        }
    }
    new_lf
}

fn align_and_convert_columns_to_string(lf: LazyFrame, all_columns: &[String]) -> LazyFrame {
    let mut aligned_columns: Vec<Expr> = Vec::new();
    for co in all_columns {
        if lf.schema().unwrap().get_field(co).is_some() {
            aligned_columns.push(col(co));
        } else {
            aligned_columns.push(lit(NULL).alias(co));
        }
    }

    let aligned_lf = lf.select(&aligned_columns);
    let aligned_lf = aligned_lf.with_columns(
        all_columns
            .iter()
            .map(|co| col(co).cast(DataType::Utf8))
            .collect::<Vec<_>>(),
    );

    aligned_lf
}

pub async fn try_download(
    url: String,
    subdir: String,
    semaphore: Arc<Semaphore>,
) -> Result<PathBuf, cached_path::Error> {
    let permit = semaphore.clone().acquire_owned().await.unwrap();

    tokio::task::spawn_blocking(move || {
        let cache = Cache::builder()
            .progress_bar(Some(cached_path::ProgressBar::Full))
            .build()?;

        let res = cache.cached_path_with_options(
            url.as_str(),
            &cached_path::Options::default().extract().subdir(&subdir),
        );

        let result = match res {
            Ok(path) => Ok(path),
            Err(_err) => {
                let cache = Cache::builder()
                    .progress_bar(Some(cached_path::ProgressBar::Full))
                    .offline(true)
                    .build()?;
                cache.cached_path_with_options(
                    url.as_str(),
                    &cached_path::Options::default().extract().subdir(&subdir),
                )
            }
        }?;

        // Verifica se `result` é um arquivo ou diretório
        if result.is_file() {
            let utf8_path = PathBuf::from(format!("{}.utf8", result.display()));
            // Verifica se o arquivo já foi convertido para UTF-8
            if utf8_path.exists() {
                drop(permit); // Libera a permissão ao terminar a tarefa
                return Ok(utf8_path);
            }

            // Converte o arquivo para UTF-8
            convert_file_to_utf8(&result, &utf8_path)?;
            drop(permit); // Libera a permissão ao terminar a tarefa
            return Ok(utf8_path);
        } else if result.is_dir() {
            let utf8_path = PathBuf::from(format!("{}-utf8", result.display()));
            // Verifica se o arquivo já foi convertido para UTF-8
            if utf8_path.exists() {
                drop(permit); // Libera a permissão ao terminar a tarefa
                return Ok(utf8_path);
            }
            // Processa todos os arquivos no diretório
            for entry in fs::read_dir(&result)? {
                let entry = entry?;
                let file_path = entry.path();

                if file_path.is_file() {
                    if let Some(name) = file_path.file_name().to_owned() {
                        let utf8_file_path =
                            format!("{}/{}", utf8_path.display(), name.to_string_lossy());
                        let path = PathBuf::from(utf8_file_path.to_string());
                        // Converte o arquivo para UTF-8
                        convert_file_to_utf8(&file_path, &path)?;
                    }
                }
            }
            drop(permit); // Libera a permissão ao terminar a tarefa
            return Ok(utf8_path);
        }

        drop(permit); // Libera a permissão ao terminar a tarefa
        Ok(result)
    })
    .await
    .unwrap()
}

// Função que converte um único arquivo para UTF-8 e trunca o arquivo original
fn convert_file_to_utf8(file_path: &Path, utf8_file_path: &Path) -> Result<(), std::io::Error> {
    if utf8_file_path.exists() {
        return Ok(());
    }

    if let Some(p) = utf8_file_path.parent() {
        if !p.exists() {
            fs::create_dir_all(p)?;
        }
    }

    let mut file = File::open(file_path)?;
    let mut bytes = Vec::new();
    io::copy(&mut file, &mut bytes)?;
    let (decoded_str, _, had_errors) = WINDOWS_1252.decode(&bytes);
    if had_errors {
        log::error!("Erro ao decodificar CSV: {:?}", utf8_file_path);
    }
    let mut outfile = File::create(utf8_file_path)?;
    outfile.write_all(decoded_str.as_bytes())?;

    remove_file(file_path)?;
    // Limpa o arquivo original (trunca seu conteúdo)
    //  File::create(file_path)?; // Trunca o arquivo original

    Ok(())
}
