use chrono::NaiveDate;
use polars::{
    error::PolarsError,
    lazy::dsl::{col, lit, Expr},
    prelude::{DataType, LazyCsvReader, LazyFileListReader, LazyFrame, NULL},
};
use regex::Regex;
use glob::glob;

pub mod fund;
pub mod indicator;
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

fn extract_date_from_filename(file_name: &str) -> Option<NaiveDate> {
    let parts: Vec<&str> = file_name.split('_').collect();
    if parts.len() < 4 {
        return None;
    }
    let year = parts[3][..4].parse().ok()?;
    let month = parts[3][4..6].parse().ok()?;
    Some(NaiveDate::from_ymd_opt(year, month, 1).unwrap())
}

pub fn portfolio_available_dates() -> Vec<String> {
    // Define a expressão regular para extrair ano e mês
    let re = Regex::new(r"cda_fi_(\d{6})").unwrap();
    let mut year_month_list = Vec::new();
    // Busca os arquivos usando o padrão
    for entry in glob("./dataset/cda/cda_fi_*/cda_fi_*.csv").expect("Failed to read glob pattern") {
        if let Ok(path) = entry {
            if let Some(path_str) = path.to_str() {
                if let Some(caps) = re.captures(path_str) {
                    // Extrai o ano e mês do capture
                    let year_month = &caps[1];
                    let formatted = format!("{}/{}", &year_month[..4], &year_month[4..6]);
                    year_month_list.push(formatted);
                }
            }
        }
    }
    // Remove duplicatas e reverte a lista
    year_month_list.reverse();
    year_month_list.dedup();
    year_month_list
}
