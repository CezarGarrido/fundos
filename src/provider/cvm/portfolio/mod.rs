use chrono::NaiveDate;
use glob::glob;

use options::Options;
use polars::{
    datatypes::DataType,
    error::PolarsError,
    frame::DataFrame,
    lazy::{
        dsl::{col, concat, lit},
        frame::LazyFrame,
    },
    prelude::{IntoLazy, SortOptions, StrptimeOptions, UnionArgs},
};
pub mod options;

use super::{align_and_convert_columns_to_string, get_all_columns, read_csv_lazy};

#[derive(Clone)]
pub struct Portfolio {
    options: Options,
}

impl Portfolio {
    pub fn new() -> Self {
        let options = options::load().unwrap();
        Self { options }
    }

    pub async fn async_read_assets(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<(LazyFrame, LazyFrame), PolarsError> {
        crate::provider::cvm::fund::set_status("Baixando composição de carteiras da CVM...");
        let result = self
            .options
            .async_path(Some(start_date), Some(end_date))
            .await;
        match result {
            Ok(paths) => {
                let mut frames = Vec::new();
                let mut pls = Vec::new();
                for path in paths {
                    let pattern = format!("{}/*", path.display());
                    for path in glob(&pattern).unwrap().filter_map(Result::ok) {
                        let file = path.display().to_string();
                        crate::provider::cvm::fund::set_status(&format!(
                            "Carregando arquivo de carteira: {}",
                            path.file_name().unwrap().to_string_lossy()
                        ));
                        let res = read_csv_lazy(&file);
                        match res {
                            Ok(mut lf) => {
                                let schema = match lf.schema() {
                                    Ok(s) => s,
                                    Err(err) => {
                                        log::error!("Erro ao obter schema do portfólio: {}", err);
                                        continue;
                                    }
                                };
                                if schema.contains("CNPJ_FUNDO_CLASSE") {
                                    lf = lf
                                        .with_column(col("CNPJ_FUNDO_CLASSE").alias("CNPJ_FUNDO"));
                                }

                                if file.contains("PL") {
                                    pls.push(lf)
                                } else {
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
                                        .filter(
                                            col("AS_DATE").gt_eq(lit(start_date)).and(
                                                col("AS_DATE")
                                                    .cast(DataType::Date)
                                                    .lt_eq(lit(end_date)),
                                            ),
                                        )
                                        .collect()
                                        .unwrap()
                                        .lazy();
                                    frames.push(lf)
                                }
                            }
                            Err(err) => {
                                log::error!("err {}", err);
                            }
                        }
                    }
                }

                let all_columns = get_all_columns(&frames);
                let aligned_lfs: Vec<LazyFrame> = frames
                    .into_iter()
                    .map(|lf| align_and_convert_columns_to_string(lf, &all_columns))
                    .collect();

                // Se nenhum mês foi carregado (ex: período todo no futuro), retorna vazio
                if aligned_lfs.is_empty() {
                    return Ok((DataFrame::empty().lazy(), LazyFrame::default()));
                }

                let result = concat(&aligned_lfs, UnionArgs::default());
                match result {
                    Ok(lf) => {
                        if pls.is_empty() {
                            Ok((lf, LazyFrame::default()))
                        } else {
                            let pl = pls.first().unwrap().clone();
                            Ok((lf, pl))
                        }
                    }
                    Err(err) => Err(err),
                }
            }
            Err(err) => Err(PolarsError::NoData(err.to_string().into())),
        }
    }

    fn get_month_start_and_end(
        &self,
        month: String,
        year: String,
    ) -> Result<(NaiveDate, NaiveDate), String> {
        // Tentar converter os valores de string para u32 e i32
        let month: u32 = month.parse().map_err(|_| "Invalid month format")?;
        let year: i32 = year.parse().map_err(|_| "Invalid year format")?;

        // Verificar se o mês está no intervalo válido
        if !(1..=12).contains(&month) {
            return Err("Month must be between 1 and 12".into());
        }

        // Criar a data de início (primeiro dia do mês)
        let start_date = NaiveDate::from_ymd_opt(year, month, 1).unwrap();

        // Calcular o último dia do mês
        let end_date = match month {
            12 => NaiveDate::from_ymd_opt(year + 1, 1, 1)
                .unwrap()
                .pred_opt()
                .unwrap(), // Janeiro do ano seguinte
            _ => NaiveDate::from_ymd_opt(year, month + 1, 1)
                .unwrap()
                .pred_opt()
                .unwrap(), // Último dia do mês atual
        };

        Ok((start_date, end_date))
    }

    pub async fn async_assets(
        &self,
        cnpj: String,
        year: String,
        month: String,
        top: bool,
    ) -> Result<(DataFrame, DataFrame, DataFrame), PolarsError> {
        let (start_date, end_date) = self.get_month_start_and_end(month, year).unwrap();
        let res = self
            .async_read_assets(start_date.to_owned(), end_date.to_owned())
            .await;

        match res {
            Ok((lf, pl)) => {
                let mut valor_pl = 0.0;
                let pl = pl
                    .filter(col("CNPJ_FUNDO").eq(lit(cnpj.clone())))
                    .collect()?;

                if let Some(parsed_value) = pl
                    .column("VL_PATRIM_LIQ")
                    .ok()
                    .and_then(|col| col.get(0).ok())
                    .and_then(|val| val.get_str().map(|s| s.to_string()))
                    .and_then(|value_str| value_str.parse::<f64>().ok())
                {
                    valor_pl = parsed_value;
                }

                let assets = lf
                    .filter(col("CNPJ_FUNDO").eq(lit(cnpj.clone())))
                    .with_column(
                        (col("VL_MERC_POS_FINAL").cast(DataType::Float64) / lit(valor_pl)
                            * lit(100.0))
                        .round(3)
                        .alias("VL_PORCENTAGEM_PL"),
                    )
                    .collect()
                    .unwrap();
                if top {
                    let res = self.top_assets(assets.clone().lazy(), cnpj.clone());
                    match res {
                        Ok(top_assets) => return Ok((pl.clone(), assets.clone(), top_assets)),
                        Err(_) => return Ok((pl, assets, DataFrame::empty())),
                    }
                };

                Ok((pl, assets, DataFrame::empty()))
            }
            Err(err) => Err(err),
        }
    }

    pub fn top_assets(&self, lf: LazyFrame, cnpj: String) -> Result<DataFrame, PolarsError> {
        let assets = lf
            .filter(col("CNPJ_FUNDO").str().contains(lit(cnpj), false))
            .with_column(col("VL_MERC_POS_FINAL").cast(DataType::Float64))
            .with_column(col("VL_PORCENTAGEM_PL").cast(DataType::Float64))
            //.select(&[
            //      col("TP_APLIC"),
            //      col("VL_MERC_POS_FINAL").cast(DataType::Float64),
            //       col("VL_PORCENTAGEM_PL"),
            //   ])
            .groupby(vec![col("TP_APLIC")]);

        let top_assets = assets
            .agg([
                col("VL_MERC_POS_FINAL").sum(),
                col("VL_PORCENTAGEM_PL").sum(),
            ])
            .sort(
                "VL_MERC_POS_FINAL",
                polars::prelude::SortOptions {
                    descending: true,
                    ..Default::default()
                },
            )
            .collect()
            .unwrap();
        Ok(top_assets)
    }

    /// Carrega histórico mensal de ativos de um fundo específico
    pub async fn async_historical_assets(
        &self,
        cnpj: String,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<DataFrame, PolarsError> {
        let (lf, pl_lf) = self
            .async_read_assets(start_date, end_date)
            .await
            .map_err(|e| PolarsError::NoData(e.to_string().into()))?;

        let schema = lf.schema()?;
        if !schema.contains("AS_DATE") {
            return Ok(DataFrame::empty());
        }

        // Join PL data with positions
        let with_pl = if pl_lf
            .schema()
            .ok()
            .map(|s| s.contains("VL_PATRIM_LIQ"))
            .unwrap_or(false)
        {
            // Align PL schema
            let pl_cols = ["CNPJ_FUNDO", "VL_PATRIM_LIQ", "DT_COMPTC"];
            let pl_exists: Vec<&str> = pl_cols
                .iter()
                .filter(|c| pl_lf.schema().ok().map(|s| s.contains(c)).unwrap_or(false))
                .copied()
                .collect();
            if pl_exists.len() == 3 {
                let pl = pl_lf
                    .filter(col("CNPJ_FUNDO").eq(lit(cnpj.clone())))
                    .collect()?;
                if pl.height() > 0 {
                    let _pl_date = pl
                        .column("DT_COMPTC")
                        .ok()
                        .and_then(|c| c.get(0).ok())
                        .and_then(|v| v.get_str().map(|s| s.to_string()))
                        .unwrap_or_default();
                    let pl_value: f64 = pl
                        .column("VL_PATRIM_LIQ")
                        .ok()
                        .and_then(|col| col.get(0).ok())
                        .and_then(|val| val.get_str().map(|s| s.to_string()))
                        .and_then(|s| s.parse::<f64>().ok())
                        .unwrap_or(0.0);

                    if pl_value > 0.0 {
                        let result = lf
                            .clone()
                            .filter(col("CNPJ_FUNDO").eq(lit(cnpj.clone())))
                            .with_column(
                                (col("VL_MERC_POS_FINAL").cast(DataType::Float64) / lit(pl_value)
                                    * lit(100.0))
                                .round(3)
                                .alias("VL_PORCENTAGEM_PL"),
                            )
                            .collect()?;
                        return Ok(result);
                    }
                }
            }
            // Fallback: no PL join
            lf.clone()
                .filter(col("CNPJ_FUNDO").eq(lit(cnpj.clone())))
                .collect()?
        } else {
            lf.clone()
                .filter(col("CNPJ_FUNDO").eq(lit(cnpj.clone())))
                .collect()?
        };

        Ok(with_pl)
    }

    /// Agrega todos os ativos de todas as carteiras no período, gerando ranking de mercado.
    pub async fn async_market_assets(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<DataFrame, PolarsError> {
        crate::provider::cvm::fund::set_status("Baixando carteiras do mercado para análise...");
        let (lf, _pl) = self
            .async_read_assets(start_date, end_date)
            .await
            .map_err(|e| PolarsError::NoData(e.to_string().into()))?;

        crate::provider::cvm::fund::set_status("Calculando ranking de ativos do mercado...");

        // Group by asset identity: ISIN + application type + asset type
        // TP_TITPUB and DT_VENC are aggregated as first-occurrence metadata
        let schema = lf.schema()?;
        let has_col = |name: &str| schema.contains(name);

        let mut agg_exprs = vec![
            // Distinct funds holding a position
            col("CNPJ_FUNDO").n_unique().alias("N_FUNDOS"),
            // Total market value of the position across all funds
            col("VL_MERC_POS_FINAL")
                .cast(DataType::Float64)
                .sum()
                .alias("VL_MERC_TOTAL"),
            // Total acquisition value
            col("VL_AQUIS_NEGOC")
                .cast(DataType::Float64)
                .sum()
                .alias("VL_COMPRADO"),
            // Total sale value
            col("VL_VENDA_NEGOC")
                .cast(DataType::Float64)
                .sum()
                .alias("VL_VENDIDO"),
            // Count of buy transactions (rows where acquisition > 0)
            col("VL_AQUIS_NEGOC")
                .cast(DataType::Float64)
                .gt(lit(0.0_f64))
                .cast(DataType::UInt32)
                .sum()
                .alias("N_COMPRADORES"),
            // Count of sell transactions (rows where venda > 0)
            col("VL_VENDA_NEGOC")
                .cast(DataType::Float64)
                .gt(lit(0.0_f64))
                .cast(DataType::UInt32)
                .sum()
                .alias("N_VENDEDORES"),
            // Individual position stats (min/max/mean)
            col("VL_MERC_POS_FINAL")
                .cast(DataType::Float64)
                .min()
                .alias("VL_MIN"),
            col("VL_MERC_POS_FINAL")
                .cast(DataType::Float64)
                .max()
                .alias("VL_MAX"),
            col("VL_MERC_POS_FINAL")
                .cast(DataType::Float64)
                .mean()
                .alias("VL_MEDIO"),
            // Individual acquisition stats (min/max/mean)
            col("VL_AQUIS_NEGOC")
                .cast(DataType::Float64)
                .filter(
                    col("VL_AQUIS_NEGOC")
                        .cast(DataType::Float64)
                        .gt(lit(0.0_f64)),
                )
                .min()
                .alias("VL_COMPRA_MIN"),
            col("VL_AQUIS_NEGOC")
                .cast(DataType::Float64)
                .max()
                .alias("VL_COMPRA_MAX"),
            col("VL_AQUIS_NEGOC")
                .cast(DataType::Float64)
                .filter(
                    col("VL_AQUIS_NEGOC")
                        .cast(DataType::Float64)
                        .gt(lit(0.0_f64)),
                )
                .mean()
                .alias("VL_COMPRA_MEDIO"),
            // Metadata columns (first occurrence)
            col("TP_TITPUB").first().alias("TP_TITPUB"),
            col("DT_VENC").first().alias("DT_VENC"),
        ];

        // Optional identity columns — may not exist in all CVM files
        for col_name in &["CD_ATIVO", "DS_ATIVO", "NM_FUNDO_COTA", "CD_SELIC"] {
            if has_col(col_name) {
                agg_exprs.push(col(col_name).first().alias(col_name));
            }
        }

        let grouped = lf
            .groupby(vec![col("CD_ISIN"), col("TP_APLIC"), col("TP_ATIVO")])
            .agg(agg_exprs)
            .sort(
                "VL_MERC_TOTAL",
                SortOptions {
                    descending: true,
                    ..Default::default()
                },
            );

        // Add PL_MEDIO = total market value / number of funds holding the position
        let result = grouped
            .with_column(
                (col("VL_MERC_TOTAL") / col("N_FUNDOS").cast(DataType::Float64)).alias("PL_MEDIO"),
            )
            .collect()?;

        Ok(result)
    }

    /// Fetches all funds holding a specific asset (by ISIN or CD_ATIVO) in the given date range.
    pub async fn async_asset_holders(
        &self,
        asset_id: String,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<DataFrame, PolarsError> {
        let (lf, pl_lf) = self
            .async_read_assets(start_date, end_date)
            .await
            .map_err(|e| PolarsError::NoData(e.to_string().into()))?;

        let schema = lf.schema()?;
        let has_nm_fundo = schema.contains("NM_FUNDO");

        // Filter by the specific asset
        let filtered = lf.filter(
            col("CD_ISIN")
                .eq(lit(asset_id.clone()))
                .or(col("CD_ATIVO").eq(lit(asset_id.clone()))),
        );

        let mut agg_exprs = vec![
            col("DT_COMPTC").last().alias("DT_COMPTC"),
            col("VL_MERC_POS_FINAL")
                .cast(DataType::Float64)
                .last()
                .alias("VL_MERC_POS_FINAL"),
        ];

        if has_nm_fundo {
            agg_exprs.push(col("NM_FUNDO").last().alias("NM_FUNDO"));
        }

        // Group by CNPJ_FUNDO to get the latest position for each fund in this period
        let holders = filtered.groupby(vec![col("CNPJ_FUNDO")]).agg(agg_exprs);

        // If PL is available, join to calculate percentage
        let final_lf = if pl_lf
            .schema()
            .map(|s| s.contains("VL_PATRIM_LIQ"))
            .unwrap_or(false)
        {
            let pl_grouped = pl_lf
                .groupby(vec![col("CNPJ_FUNDO")])
                .agg(vec![col("VL_PATRIM_LIQ")
                    .cast(DataType::Float64)
                    .last()
                    .alias("VL_PATRIM_LIQ")]);

            holders
                .left_join(pl_grouped, col("CNPJ_FUNDO"), col("CNPJ_FUNDO"))
                .with_column(
                    (col("VL_MERC_POS_FINAL") / col("VL_PATRIM_LIQ") * lit(100.0))
                        .round(3)
                        .alias("VL_PORCENTAGEM_PL"),
                )
        } else {
            holders.with_column(lit(0.0_f64).alias("VL_PORCENTAGEM_PL"))
        };

        let result = final_lf
            .sort(
                "VL_MERC_POS_FINAL",
                SortOptions {
                    descending: true,
                    ..Default::default()
                },
            )
            .collect()?;

        Ok(result)
    }
}
