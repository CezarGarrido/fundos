use std::{fs::File, io::{self, Cursor, Write}, path::{Path, PathBuf}};

use encoding_rs::WINDOWS_1252;
use fund::Register;
use glob::glob;
use informe::Informe;
use polars::{
    error::PolarsError,
    lazy::dsl::{col, lit, Expr},
    prelude::{DataType, LazyCsvReader, LazyFileListReader, LazyFrame, NULL},
};
use portfolio::Portfolio;
use regex::Regex;
use zip::ZipArchive;

use crate::ui::download::Download;

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

pub fn portfolio_available_dates() -> Vec<String> {
    // Define a expressão regular para extrair ano e mês
    let re = Regex::new(r"_(\d{6})\.csv$").unwrap();
    let mut year_month_list = Vec::new();
    // Busca os arquivos usando o padrão
    for path in glob("./dataset/carteira/*.csv")
        .expect("Failed to read glob pattern")
        .flatten()
    {
        if let Some(path_str) = path.to_str() {
            if let Some(caps) = re.captures(path_str) {
                // Extrai o ano e mês do capture
                let year_month = &caps[1];
                let formatted = format!("{}/{}", &year_month[..4], &year_month[4..6]);
                year_month_list.push(formatted);
            }
        }
    }
    // Remove duplicatas e reverte a lista
    year_month_list.sort();
    year_month_list.dedup();
    year_month_list.reverse();
    year_month_list
}

fn unzip_and_save(zip_bytes: &[u8], destination_dir: PathBuf) -> io::Result<()> {
    let reader = Cursor::new(zip_bytes);
    let mut archive = ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let file_name = file.name().to_string();

        if file_name.ends_with(".csv") {
            let out_path = Path::new(&destination_dir).join(file_name);

            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut bytes = Vec::new();
            io::copy(&mut file, &mut bytes)?;

            let (decoded_str, _, had_errors) = WINDOWS_1252.decode(&bytes);
            if had_errors {
                eprintln!("Erro ao decodificar CSV: {:?}", out_path);
                continue;
            }

            let mut out_file = File::create(out_path)?;
            out_file.write_all(decoded_str.as_bytes())?;
        }
    }

    Ok(())
}

pub fn download(
    token: tokio_util::sync::CancellationToken,
    name: String,
    on_progress: impl 'static + Send + FnMut(Download),
) {
    if name == "cad" {
        return Register::new().download(token, on_progress);
    }

    if name == "informe" {
        return Informe::new().download(token, on_progress);
    }

    if name == "carteira" {
        Portfolio::new().download(token, on_progress)
    }
}
