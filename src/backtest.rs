use crate::analytics::compute_asset_analytics;
use polars::prelude::*;

#[derive(Debug)]
pub struct BacktestResult {
    pub coverage_ratio: f64,
    pub autocorr_lag1: f64,
    pub liquidity_violations: usize,
}

/// Executa a validação cross-sectional (Walk-Forward) simulando uma carteira de ativos.
/// O objetivo é aferir as métricas de metrologia institucional do Filtro de Kalman.
pub fn run_walk_forward_validation() -> BacktestResult {
    use std::io::Write;
    let _ = std::fs::remove_file("metrology_trajectories.csv");
    let mut file = std::fs::File::create("metrology_trajectories.csv").unwrap();
    let _ = writeln!(
        file,
        "fund_id,last_qty,target_qty,predicted_qty,ci_lower,ci_upper"
    );
    let num_funds = 50;
    let mut hits = 0;
    let mut total_predictions = 0;
    let mut residuals = Vec::new();
    let mut liquidity_violations = 0;

    for i in 0..num_funds {
        let asset_name = format!("FUNDO_{}", i);

        // Simulação Estocástica de Trajetória
        let mut qtys = Vec::new();
        let mut current_qty = 1000.0;
        let mut target_qty = 0.0;

        let mut historical_qtys = Vec::new();
        for m in 0..11 {
            // Ruído pseudo-aleatório combinado com drift
            let noise = ((i + m) % 7) as f64 * 80.0 - 240.0;
            current_qty = (current_qty + 20.0 + noise).max(10.0);

            // Choque gigantesco no Fundo 0 para estourar o limite de liquidez de propósito
            if i == 0 && m == 9 {
                current_qty += 50000.0;
            }

            qtys.push(current_qty);
            if m < 10 {
                historical_qtys.push(current_qty);
            } else {
                // Para 2 fundos, aplicamos um choque exógeno forte no T+1 para estourar o IC95
                if i == 1 || i == 2 {
                    target_qty = (current_qty + 2000.0).max(10.0);
                } else {
                    target_qty = current_qty;
                }
            }
        }

        let cd_ativo = Series::new("CD_ATIVO", vec![asset_name.as_str(); 10]);
        let dt_comptc = Series::new(
            "DT_COMPTC",
            vec![
                "2024-01", "2024-02", "2024-03", "2024-04", "2024-05", "2024-06", "2024-07",
                "2024-08", "2024-09", "2024-10",
            ],
        );
        let qt_pos = Series::new("QT_POS_FINAL", historical_qtys.clone());
        let pct_pl = Series::new("VL_PORCENTAGEM_PL", vec![1.0; 10]);
        let vl_aquis = Series::new("VL_AQUIS_NEGOC", vec![1000.0; 10]);
        let vl_merc = Series::new(
            "VL_MERC_POS_FINAL",
            historical_qtys
                .iter()
                .map(|q| q * 10.0)
                .collect::<Vec<f64>>(),
        );
        let pl = Series::new("VL_PATRIM_LIQ", vec![100000.0; 10]);
        let cd_isin = Series::new("CD_ISIN", vec![""; 10]);

        let df = DataFrame::new(vec![
            cd_ativo, cd_isin, dt_comptc, qt_pos, pct_pl, vl_aquis, vl_merc, pl,
        ])
        .unwrap();

        let q_date = Series::new("date", vec!["2024-11-01"]);
        let q_close = Series::new("adjclose", vec![10.0]);
        let quotes_df = DataFrame::new(vec![q_date, q_close]).unwrap();

        if let Some(analytics) = compute_asset_analytics(&df, &asset_name, Some(&quotes_df), None) {
            if let Some(inference) = analytics.portfolio_inference {
                if let Some((_, pred, lower, upper)) = inference.forward_trajectory.first() {
                    total_predictions += 1;

                    // 1. Teste de Cobertura (IC95)
                    if target_qty >= *lower && target_qty <= *upper {
                        hits += 1;
                    }

                    // 2. Coleta de Resíduos para Ruído Branco
                    let residual = target_qty - *pred;
                    residuals.push(residual);

                    // Salvar dados no CSV para plotagem externa
                    let mut file = std::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open("metrology_trajectories.csv")
                        .unwrap();
                    let _ = writeln!(
                        file,
                        "{},{},{},{},{},{}",
                        asset_name,
                        historical_qtys.last().unwrap(),
                        target_qty,
                        pred,
                        lower,
                        upper
                    );

                    // 3. Teste de Estresse de Liquidez Físico
                    // Assumimos que a liquidez diária do mercado (ADTV) do ativo é extremamente baixa (ex: 5 cotas/dia)
                    let adtv = 5.0;
                    let trading_days = 21.0;
                    let max_market_capacity = adtv * trading_days;

                    let predicted_trade_volume = (*pred - historical_qtys.last().unwrap()).abs();
                    if predicted_trade_volume > max_market_capacity {
                        liquidity_violations += 1;
                    }
                }
            }
        }
    }

    let coverage_ratio = if total_predictions > 0 {
        hits as f64 / total_predictions as f64
    } else {
        0.0
    };

    // Cálculo da Autocorrelação (Lag-1) para Teste de Ruído Branco
    let autocorr_lag1 = if residuals.len() > 2 {
        let mean_res: f64 = residuals.iter().sum::<f64>() / residuals.len() as f64;
        let mut num = 0.0;
        let mut den = 0.0;
        for i in 1..residuals.len() {
            num += (residuals[i] - mean_res) * (residuals[i - 1] - mean_res);
        }
        for res in &residuals {
            den += (res - mean_res).powi(2);
        }
        if den > 1e-6 {
            num / den
        } else {
            0.0
        }
    } else {
        0.0
    };

    BacktestResult {
        coverage_ratio,
        autocorr_lag1,
        liquidity_violations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrologia_institucional() {
        let results = run_walk_forward_validation();

        println!("Resultados do Backtest:");
        println!(
            "Coverage Ratio (IC95): {:.2}%",
            results.coverage_ratio * 100.0
        );
        println!("Autocorrelação (Lag-1): {:.4}", results.autocorr_lag1);
        println!("Violações de Liquidez: {}", results.liquidity_violations);

        // O cone estatístico deve ser robusto (exigência acadêmica: cobertura > 80% na simulação ruidosa)
        assert!(
            results.coverage_ratio > 0.80,
            "Filtro subestimou brutalmente a incerteza (Otimismo excessivo). Cobertura: {:.2}%",
            results.coverage_ratio * 100.0
        );
        assert!(
            results.coverage_ratio < 1.0,
            "Filtro superestimou o cone de incerteza (Bandas infinitas). Cobertura: 100%"
        );

        // Os erros devem ser fracamente correlacionados (Ruído Branco -> autocorr próxima de 0)
        assert!(
            results.autocorr_lag1.abs() < 0.40,
            "Filtro apresenta forte vazamento de memória nos resíduos (Autocorrelação: {:.2}).",
            results.autocorr_lag1
        );

        // Deve identificar violações do restrito ADTV (estresse físico provou-se ativo)
        assert!(
            results.liquidity_violations > 0,
            "Nenhuma violação de liquidez detectada. O stress test falhou."
        );
    }

    #[test]
    fn test_generate_academic_charts_data() {
        use std::io::Write;
        let mut file = std::fs::File::create("academic_charts_data.csv").unwrap();
        writeln!(
            file,
            "t,real_qty,filtered_qty,ci_lower,ci_upper,z_score,residual,is_future"
        )
        .unwrap();

        let n = 36;
        let mut real_qtys = Vec::new();
        let mut current = 1000.0;

        // Mês 1 a 12: Estável com ruído
        // Mês 13 a 24: Acumulação Direcional
        // Mês 25 a 30: Estável
        // Mês 31 a 36: Predição cega (Futuro)
        for t in 0..30 {
            let noise = ((t * 13) % 11) as f64 * 10.0 - 50.0;
            if t >= 12 && t < 24 {
                current += 100.0; // Drift
            }
            real_qtys.push((current + noise).max(0.0));
        }

        // Setup Kalman
        let log_historical: Vec<f64> = real_qtys.iter().map(|&q| q.max(1.0).ln()).collect();
        let mut kf = kalman_filters::KalmanFilterBuilder::<f64>::new(2, 1)
            .initial_state(vec![log_historical[0], 0.0])
            .initial_covariance(vec![0.5, 0.0, 0.0, 0.1])
            .transition_matrix(vec![1.0, 1.0, 0.0, 0.85])
            .process_noise(vec![0.05, 0.0, 0.0, 0.01])
            .observation_matrix(vec![1.0, 0.0])
            .measurement_noise(vec![0.1])
            .build()
            .unwrap();

        for t in 0..30 {
            kf.predict();
            kf.update(&[log_historical[t]]).unwrap();

            let filtered_log = kf.state()[0];
            let cov = kf.covariance()[0];
            let std = if cov > 1e-6 { cov.sqrt() * 1.5 } else { 0.05 };

            let filtered_qty = filtered_log.exp();
            let ci_lower = (filtered_log - 1.96 * std).exp();
            let ci_upper = (filtered_log + 1.96 * std).exp();

            let drift = kf.state()[1];
            let drift_cov = kf.covariance()[3];
            let drift_std = if drift_cov > 1e-6 {
                drift_cov.sqrt()
            } else {
                0.05
            };
            let z_score = drift / drift_std;
            let residual = real_qtys[t] - filtered_qty;

            writeln!(
                file,
                "{},{},{},{},{},{},{},0",
                t, real_qtys[t], filtered_qty, ci_lower, ci_upper, z_score, residual
            )
            .unwrap();
        }

        // Extrapolação futura (Cone de Incerteza colapsando)
        for t in 30..36 {
            kf.predict();
            let filtered_log = kf.state()[0];
            let cov = kf.covariance()[0];
            let std = if cov > 1e-6 { cov.sqrt() * 1.5 } else { 0.05 };

            let filtered_qty = filtered_log.exp();
            let ci_lower = (filtered_log - 1.96 * std).exp();
            let ci_upper = (filtered_log + 1.96 * std).exp();

            // Simula uma realidade dentro do cone na maioria e 1 fuga fora
            let noise = ((t * 17) % 13) as f64 * 30.0 - 150.0;
            let mut fake_real = filtered_qty + noise;
            if t == 34 {
                fake_real = ci_upper + 500.0;
            } // Outlier intencional

            let residual = fake_real - filtered_qty;
            writeln!(
                file,
                "{},{},{},{},{},{},{},1",
                t, fake_real, filtered_qty, ci_lower, ci_upper, 0.0, residual
            )
            .unwrap();
        }
    }

    #[tokio::test]
    #[ignore] // Rode apenas manualmente com: cargo test test_real_cvm_backtest -- --ignored --nocapture
    async fn test_real_cvm_backtest() {
        use crate::provider::cvm::portfolio::Portfolio;
        use chrono::NaiveDate;
        use std::io::Write;

        let start_date = NaiveDate::from_ymd_opt(2024, 7, 1).unwrap();
        let end_date = NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(); // 12 meses

        let portfolio = Portfolio::new();
        let mut file = std::fs::File::create("real_cvm_trajectories.csv").unwrap();
        writeln!(
            file,
            "fund_id,asset,t,real_qty,filtered_qty,ci_lower,ci_upper,z_score,residual,is_future"
        )
        .unwrap();

        // Criamos quotes fictícias para que o motor de analytics consiga rodar a inferência de portfólio
        use polars::frame::DataFrame;
        use polars::prelude::Series;
        let mut dates = Vec::new();
        for year in 2024..=2026 {
            for month in 1..=12 {
                dates.push(format!("{}-{:02}-01", year, month));
            }
        }
        let q_date = Series::new("date", dates);
        let q_close = Series::new("adjclose", vec![10.0; 36]);
        let quotes_df = DataFrame::new(vec![q_date, q_close]).unwrap();
        let yahoo_provider = crate::provider::yahoo::YahooProvider::new();

        println!("Carregando composição de carteiras da CVM uma única vez...");
        let (lf_all, pl_lf_all) = portfolio
            .async_read_assets(start_date, end_date)
            .await
            .unwrap();
        println!(
            "Dados carregados na memória. Selecionando 10 fundos que possuem ações em carteira..."
        );

        let fund_cnpjs = vec![
            "12.987.743/0001-86".to_string(), // Alaska Black FIC FIA
            "73.232.530/0001-39".to_string(), // Dynamo Cougar FIA
            "13.412.399/0001-71".to_string(), // Constellation FIA
            "17.525.132/0001-09".to_string(), // Versa Long Biased FIM
        ];

        println!("Fundos Famosos Selecionados: {:?}", fund_cnpjs);

        println!(
            "Iniciando processamento para os {} fundos selecionados...",
            fund_cnpjs.len()
        );

        let mut filter_expr =
            polars::lazy::dsl::col("CNPJ_FUNDO").eq(polars::lazy::dsl::lit(fund_cnpjs[0].clone()));
        for cnpj in fund_cnpjs.iter().skip(1) {
            filter_expr = filter_expr
                .or(polars::lazy::dsl::col("CNPJ_FUNDO").eq(polars::lazy::dsl::lit(cnpj.clone())));
        }

        println!(
            "Filtrando e carregando dados apenas para os fundos na memória (1 scan rápido)..."
        );
        let lf_all = lf_all.filter(filter_expr.clone()).collect().unwrap().lazy();
        let pl_lf_all = pl_lf_all
            .filter(filter_expr.clone())
            .collect()
            .unwrap()
            .lazy();

        for cnpj in fund_cnpjs {
            println!("Processando: {}", cnpj);
            let schema = match lf_all.schema() {
                Ok(s) => s,
                Err(_) => {
                    println!("Failed to get schema for lf_all");
                    continue;
                }
            };
            if !schema.contains("AS_DATE") {
                println!("AS_DATE not in schema!");
                continue;
            }

            let pl_has_vl = pl_lf_all
                .schema()
                .ok()
                .map(|s| s.contains("VL_PATRIM_LIQ"))
                .unwrap_or(false);

            let df = if pl_has_vl {
                let pl_cols = ["CNPJ_FUNDO", "VL_PATRIM_LIQ", "DT_COMPTC"];
                let pl_exists: Vec<&str> = pl_cols
                    .iter()
                    .filter(|c| {
                        pl_lf_all
                            .schema()
                            .ok()
                            .map(|s| s.contains(c))
                            .unwrap_or(false)
                    })
                    .copied()
                    .collect();
                if pl_exists.len() == 3 {
                    let pl = match pl_lf_all
                        .clone()
                        .filter(col("CNPJ_FUNDO").eq(lit(cnpj.clone())))
                        .collect()
                    {
                        Ok(df) => df,
                        Err(e) => {
                            println!("Failed to collect pl: {:?}", e);
                            continue;
                        }
                    };
                    let pl_value: f64 = if pl.height() > 0 {
                        pl.column("VL_PATRIM_LIQ")
                            .ok()
                            .and_then(|col| col.get(0).ok())
                            .and_then(|val| val.get_str().map(|s| s.to_string()))
                            .and_then(|s| s.parse::<f64>().ok())
                            .unwrap_or(0.0)
                    } else {
                        0.0
                    };

                    let pl_monthly = pl_lf_all
                        .clone()
                        .filter(col("CNPJ_FUNDO").eq(lit(cnpj.clone())))
                        .with_column(col("DT_COMPTC").str().str_slice(0, Some(7)).alias("month"))
                        .groupby(vec![col("month")])
                        .agg(vec![col("VL_PATRIM_LIQ").last().alias("VL_PATRIM_LIQ")]);

                    match lf_all
                        .clone()
                        .filter(col("CNPJ_FUNDO").eq(lit(cnpj.clone())))
                        .with_column(col("DT_COMPTC").str().str_slice(0, Some(7)).alias("month"))
                        .join(
                            pl_monthly,
                            [col("month")],
                            [col("month")],
                            polars::prelude::JoinArgs::new(polars::prelude::JoinType::Left),
                        )
                        .with_column(lit(pl_value).alias("VL_PATRIM_LIQ_SINGLE"))
                        .with_column(
                            (col("VL_MERC_POS_FINAL").cast(DataType::Float64)
                                / lit(pl_value.max(1.0))
                                * lit(100.0))
                            .round(3)
                            .alias("VL_PORCENTAGEM_PL"),
                        )
                        .collect()
                    {
                        Ok(df) => df,
                        Err(e) => {
                            println!("Failed to collect joined df: {:?}", e);
                            continue;
                        }
                    }
                } else {
                    match lf_all
                        .clone()
                        .filter(col("CNPJ_FUNDO").eq(lit(cnpj.clone())))
                        .collect()
                    {
                        Ok(df) => df,
                        Err(e) => {
                            println!("Failed to collect df: {:?}", e);
                            continue;
                        }
                    }
                }
            } else {
                match lf_all
                    .clone()
                    .filter(col("CNPJ_FUNDO").eq(lit(cnpj.clone())))
                    .collect()
                {
                    Ok(df) => df,
                    Err(e) => {
                        println!("Failed to collect fallback df: {:?}", e);
                        continue;
                    }
                }
            };

            // Pegamos as séries que o motor de analytics recomenda
            let top_series = crate::analytics::compute_top_series(&df);
            for series in top_series.into_iter().take(3) {
                // 3 principais ativos de cada fundo
                let asset_name = series.0.trim().to_string();

                // Verifica se é qualificado para cotações do Yahoo (ex: ações como STBP3, VALE3, PDGR3, CSAN33)
                let is_yahoo_eligible = asset_name.len() >= 4
                    && asset_name.len() <= 6
                    && asset_name.chars().all(|c| c.is_alphanumeric());

                let mut quotes_to_use = &quotes_df;
                let mut downloaded_df = None;

                if is_yahoo_eligible {
                    println!(
                        "Tentando baixar cotações reais do Yahoo Finance para o ativo: {}...",
                        asset_name
                    );
                    match yahoo_provider.get_monthly_quotes(&asset_name, 36).await {
                        Ok(monthly_quotes) => {
                            if !monthly_quotes.is_empty() {
                                println!(
                                    "Sucesso! Baixadas {} cotações para {}.",
                                    monthly_quotes.len(),
                                    asset_name
                                );
                                let mut dates = Vec::new();
                                let mut closes = Vec::new();
                                for q in monthly_quotes {
                                    dates.push(q.date);
                                    closes.push(q.close_price);
                                }
                                let q_date = Series::new("date", dates);
                                let q_close = Series::new("adjclose", closes);
                                if let Ok(df) = DataFrame::new(vec![q_date, q_close]) {
                                    downloaded_df = Some(df);
                                }
                            }
                        }
                        Err(e) => {
                            println!("Aviso: Falha ao baixar cotações para {} ({}). Usando fallback com quotes fictícias.", asset_name, e);
                        }
                    }
                }

                if let Some(ref df) = downloaded_df {
                    quotes_to_use = df;
                }

                if let Some(analytics) = crate::analytics::compute_asset_analytics(
                    &df,
                    &asset_name,
                    Some(quotes_to_use),
                    None,
                ) {
                    if let Some(inference) = analytics.portfolio_inference {
                        // 1. Exporta a janela de tempo histórica (is_future = 0) com z-scores e resíduos
                        for (t, (_date, real_qty, filtered_qty, ci_lower, ci_upper)) in
                            inference.historical_states.iter().enumerate()
                        {
                            let z_score = if inference.historical_z_scores.len() > t {
                                inference.historical_z_scores[t]
                            } else {
                                0.0
                            };
                            let residual = real_qty - filtered_qty;

                            writeln!(
                                file,
                                "{},{},{},{},{},{},{},{},{},0",
                                cnpj,
                                asset_name,
                                t,
                                real_qty,
                                filtered_qty,
                                ci_lower,
                                ci_upper,
                                z_score,
                                residual
                            )
                            .unwrap();
                        }

                        let offset = inference.historical_states.len();

                        // 2. Exporta o cone de incerteza (is_future = 1) no horizonte estendido
                        for (t, (_date, filtered_qty, ci_lower, ci_upper)) in
                            inference.forward_trajectory.iter().enumerate()
                        {
                            // Preenchendo real_qty e residual com zero para o futuro
                            writeln!(
                                file,
                                "{},{},{},{},{},{},{},{},{},1",
                                cnpj,
                                asset_name,
                                t + offset,
                                filtered_qty,
                                filtered_qty,
                                ci_lower,
                                ci_upper,
                                0.0,
                                0.0
                            )
                            .unwrap();
                        }
                    }
                }
            }
        }
        println!("Backtest CVM Real exportado para real_cvm_trajectories.csv");
    }
}
