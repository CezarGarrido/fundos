use log::{debug, info};
use polars::prelude::*;

// ── Analytics Structures ─────────────────────────────────────────────────

pub struct AssetAnalytics {
    pub avg_buy_price: f64,
    pub avg_sell_price: f64,
    pub take_profit_trigger: f64,
    pub speed_to_peak: usize,
    /// (date, predicted_qty, estimated_value) — resultado do melhor método
    pub hidden_qty_estimates: Vec<(String, f64, f64)>,
    /// Métricas de qualidade da extrapolação
    pub extrapolation_quality: Option<ExtrapolationQuality>,
}

#[derive(Clone)]
pub struct ExtrapolationQuality {
    pub method: ExtrapolationMethod,
    pub r_squared: Option<f64>,
    pub mae: Option<f64>,
    pub confidence_95: Option<(f64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ExtrapolationMethod {
    Baseline,
    LinearRegression,
    KalmanFilter,
    RandomForest,
}

impl std::fmt::Display for ExtrapolationMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Baseline => write!(f, "Baseline (estático)"),
            Self::LinearRegression => write!(f, "Regressão Linear"),
            Self::KalmanFilter => write!(f, "Filtro de Kalman"),
            Self::RandomForest => write!(f, "Random Forest"),
        }
    }
}

// ── Extrapolation Engine ─────────────────────────────────────────────────

type ExtrapolationResult = (Vec<(String, f64, f64)>, ExtrapolationQuality);

/// Compara todos os métodos de extrapolação e retorna o melhor (maior R²).
pub fn extrapolate_best(
    historical_qtys: &[f64],
    last_qty: f64,
    quotes: &[crate::provider::yahoo::MonthlyQuote],
    last_known_dt: &str,
) -> ExtrapolationResult {
    let methods = [
        ExtrapolationMethod::Baseline,
        ExtrapolationMethod::LinearRegression,
        ExtrapolationMethod::KalmanFilter,
        ExtrapolationMethod::RandomForest,
    ];

    let mut best_result: Option<ExtrapolationResult> = None;
    let mut best_r2: f64 = -1.0;

    info!(
        "Extrapolação: comparando 4 métodos com {} pontos históricos e {} cotações futuras",
        historical_qtys.len(),
        quotes.len()
    );

    for method in &methods {
        let (estimates, quality) = extrapolate(
            method.clone(),
            historical_qtys,
            last_qty,
            quotes,
            last_known_dt,
        );
        let r2 = quality.r_squared.unwrap_or(-1.0);
        debug!(
            "  {} → R²={:.4}, MAE={:?}, estimativas={}",
            method,
            r2,
            quality.mae,
            estimates.len()
        );
        if r2 > best_r2 {
            best_r2 = r2;
            best_result = Some((estimates, quality));
        }
    }

    if let Some((ref estimates, ref quality)) = best_result {
        if best_r2 < 0.0 {
            info!(
                "Extrapolação: nenhum método aprendeu (melhor R²={:.4}) — usando baseline",
                best_r2
            );
            return extrapolate_baseline(quotes, last_known_dt, last_qty);
        }
        info!(
            "Extrapolação: vencedor = {} (R²={:.4}, {} estimativas)",
            quality.method,
            best_r2,
            estimates.len()
        );
    }

    best_result.unwrap_or_else(|| {
        info!("Extrapolação: todos os métodos falharam, usando baseline");
        let (estimates, quality) = extrapolate(
            ExtrapolationMethod::Baseline,
            historical_qtys,
            last_qty,
            quotes,
            last_known_dt,
        );
        (estimates, quality)
    })
}

/// Executa a extrapolação usando o método especificado.
pub fn extrapolate(
    method: ExtrapolationMethod,
    historical_qtys: &[f64],
    last_qty: f64,
    quotes: &[crate::provider::yahoo::MonthlyQuote],
    last_known_dt: &str,
) -> ExtrapolationResult {
    match method {
        ExtrapolationMethod::Baseline => extrapolate_baseline(quotes, last_known_dt, last_qty),
        ExtrapolationMethod::LinearRegression => {
            extrapolate_linear(historical_qtys, last_qty, quotes, last_known_dt)
        }
        ExtrapolationMethod::KalmanFilter => {
            extrapolate_kalman(historical_qtys, last_qty, quotes, last_known_dt)
        }
        ExtrapolationMethod::RandomForest => {
            extrapolate_random_forest(historical_qtys, last_qty, quotes, last_known_dt)
        }
    }
}

// ── Baseline: qtd estática × preço ─────────────────────────────────────

fn extrapolate_baseline(
    quotes: &[crate::provider::yahoo::MonthlyQuote],
    last_known_dt: &str,
    last_qty: f64,
) -> ExtrapolationResult {
    let mut estimates = Vec::new();
    for q in quotes {
        if q.date.as_str() > last_known_dt && last_qty > 0.0 {
            estimates.push((q.date.clone(), last_qty, last_qty * q.close_price));
        }
    }
    debug!(
        "Baseline: qtd={:.0} estática, {} estimativas geradas",
        last_qty,
        estimates.len()
    );
    let quality = ExtrapolationQuality {
        method: ExtrapolationMethod::Baseline,
        r_squared: None,
        mae: None,
        confidence_95: None,
    };
    (estimates, quality)
}

// ── Regressão Linear (OLS via smartcore) ──────────────────────────────

fn extrapolate_linear(
    historical_qtys: &[f64],
    last_qty: f64,
    quotes: &[crate::provider::yahoo::MonthlyQuote],
    last_known_dt: &str,
) -> ExtrapolationResult {
    let n = historical_qtys.len();
    if n < 3 {
        return extrapolate_baseline(quotes, last_known_dt, last_qty);
    }

    // Features: X = [[t]] para cada ponto, target: y = qtd
    let features: Vec<Vec<f64>> = (0..n).map(|i| vec![i as f64]).collect();
    let targets: Vec<f64> = historical_qtys.to_vec();

    let x = smartcore::linalg::basic::matrix::DenseMatrix::from_2d_vec(&features)
        .expect("DenseMatrix failed");
    let lr = smartcore::linear::linear_regression::LinearRegression::fit(
        &x,
        &targets,
        Default::default(),
    );

    let (intercept, slope, r_squared, mae) = match lr {
        Ok(model) => {
            let preds = model.predict(&x).unwrap_or_else(|_| targets.clone());
            let intercept = *model.intercept();
            let slope = model.coefficients().iter().next().copied().unwrap_or(0.0);

            let y_mean = targets.iter().sum::<f64>() / n as f64;
            let ss_res: f64 = targets
                .iter()
                .zip(preds.iter())
                .map(|(y, p)| (y - p).powi(2))
                .sum();
            let ss_tot: f64 = targets.iter().map(|y| (y - y_mean).powi(2)).sum();
            let r2 = if ss_tot > 0.0 {
                1.0 - ss_res / ss_tot
            } else {
                0.0
            };
            let m: f64 = targets
                .iter()
                .zip(preds.iter())
                .map(|(y, p)| (y - p).abs())
                .sum::<f64>()
                / n as f64;
            (intercept, slope, r2, m)
        }
        Err(_) => return extrapolate_baseline(quotes, last_known_dt, last_qty),
    };

    debug!(
        "Regressão Linear: intercept={:.2}, slope={:.4}, R²={:.4}, MAE={:.2}",
        intercept, slope, r_squared, mae
    );

    // R² < 0.1 → ruído, usa baseline
    if r_squared < 0.1 {
        debug!(
            "Regressão Linear: R² muito baixo ({:.4}), usando baseline",
            r_squared
        );
        let (estimates, _) = extrapolate_baseline(quotes, last_known_dt, last_qty);
        let quality = ExtrapolationQuality {
            method: ExtrapolationMethod::LinearRegression,
            r_squared: Some(r_squared),
            mae: Some(mae),
            confidence_95: None,
        };
        return (estimates, quality);
    }

    let mut estimates = Vec::new();
    for (j, q) in quotes.iter().enumerate() {
        if q.date.as_str() > last_known_dt {
            let t_future = (n + j) as f64;
            let pred_qty = (intercept + slope * t_future).max(0.0);
            estimates.push((q.date.clone(), pred_qty, pred_qty * q.close_price));
        }
    }

    let quality = ExtrapolationQuality {
        method: ExtrapolationMethod::LinearRegression,
        r_squared: Some(r_squared),
        mae: Some(mae),
        confidence_95: None,
    };
    (estimates, quality)
}

// ── Filtro de Kalman (1D posição + velocidade) ─────────────────────────

fn extrapolate_kalman(
    historical_qtys: &[f64],
    last_qty: f64,
    quotes: &[crate::provider::yahoo::MonthlyQuote],
    last_known_dt: &str,
) -> ExtrapolationResult {
    let n = historical_qtys.len();
    if n < 2 {
        return extrapolate_baseline(quotes, last_known_dt, last_qty);
    }

    let data_scale = historical_qtys
        .iter()
        .cloned()
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let q_pos = data_scale * data_scale * 0.01;
    let q_vel = data_scale * data_scale * 0.001;
    let r_obs = data_scale * data_scale * 0.05;
    let init_var = data_scale * data_scale;

    let mut kf = kalman_filters::KalmanFilterBuilder::<f64>::new(2, 1)
        .initial_state(vec![historical_qtys[0], 0.0])
        .initial_covariance(vec![init_var, 0.0, 0.0, init_var * 0.1])
        .transition_matrix(vec![1.0, 1.0, 0.0, 1.0]) // F = [[1,1],[0,1]]
        .process_noise(vec![q_pos, 0.0, 0.0, q_vel])
        .observation_matrix(vec![1.0, 0.0]) // H = [1, 0]
        .measurement_noise(vec![r_obs])
        .build()
        .expect("KalmanFilter build failed");

    let mut predictions: Vec<f64> = vec![historical_qtys[0]];
    let mut residuals: Vec<f64> = Vec::new();

    for &y_obs in historical_qtys.iter().skip(1) {
        kf.predict();
        let pred_state = kf.state().to_vec();
        predictions.push(pred_state[0]);

        let residual = y_obs - pred_state[0];
        residuals.push(residual);

        kf.update(&[y_obs]).expect("Kalman update failed");
    }

    let final_state = kf.state().to_vec();
    let final_cov = kf.covariance().to_vec();

    // R² e MAE
    let y_mean: f64 = historical_qtys.iter().sum::<f64>() / n as f64;
    let ss_res: f64 = historical_qtys
        .iter()
        .zip(predictions.iter())
        .map(|(y, p)| (y - p).powi(2))
        .sum();
    let ss_tot: f64 = historical_qtys.iter().map(|y| (y - y_mean).powi(2)).sum();
    let r_squared = if ss_tot > 0.0 {
        1.0 - ss_res / ss_tot
    } else {
        0.0
    };
    let mae: f64 = residuals.iter().map(|r| r.abs()).sum::<f64>() / residuals.len().max(1) as f64;

    let std_dev = if final_cov[0] > data_scale * 0.01 {
        final_cov[0].sqrt()
    } else {
        let var_res: f64 =
            residuals.iter().map(|r| r * r).sum::<f64>() / residuals.len().max(1) as f64;
        var_res.sqrt().max(data_scale * 0.05)
    };
    let ci_lower = (final_state[0] - 1.96 * std_dev).max(0.0);
    let ci_upper = final_state[0] + 1.96 * std_dev;

    debug!(
        "Kalman: estado_final=[pos={:.2}, vel={:.4}], IC95=[{:.0}, {:.0}], R²={:.4}, MAE={:.2}",
        final_state[0], final_state[1], ci_lower, ci_upper, r_squared, mae
    );

    // Projeção com suavização
    let mut estimates = Vec::new();
    let mut prev_qty = final_state[0];
    let vel = final_state[1];
    let ema_alpha = 0.25;
    for q in quotes {
        if q.date.as_str() > last_known_dt {
            let raw_qty = prev_qty + vel;
            let max_change = prev_qty * 0.25;
            let clamped = raw_qty.clamp(prev_qty - max_change, prev_qty + max_change);
            let pred_qty = (ema_alpha * clamped + (1.0 - ema_alpha) * prev_qty).max(0.0);
            estimates.push((q.date.clone(), pred_qty, pred_qty * q.close_price));
            prev_qty = pred_qty;
        }
    }

    let quality = ExtrapolationQuality {
        method: ExtrapolationMethod::KalmanFilter,
        r_squared: Some(r_squared),
        mae: Some(mae),
        confidence_95: Some((ci_lower, ci_upper)),
    };
    (estimates, quality)
}

// ── Random Forest (via smartcore) ─────────────────────────────────────

fn extrapolate_random_forest(
    historical_qtys: &[f64],
    last_qty: f64,
    quotes: &[crate::provider::yahoo::MonthlyQuote],
    last_known_dt: &str,
) -> ExtrapolationResult {
    let n = historical_qtys.len();
    if n < 6 {
        return extrapolate_linear(historical_qtys, last_qty, quotes, last_known_dt);
    }

    // Features: [t, y_lag1]
    let mut features: Vec<Vec<f64>> = Vec::with_capacity(n - 1);
    let mut targets: Vec<f64> = Vec::with_capacity(n - 1);
    for i in 1..n {
        features.push(vec![i as f64, historical_qtys[i - 1]]);
        targets.push(historical_qtys[i]);
    }
    let m = features.len();

    let max_depth: u16 = if m < 20 {
        3
    } else if m < 40 {
        4
    } else {
        5
    };
    let n_trees: usize = if m < 20 {
        15
    } else if m < 40 {
        30
    } else {
        50
    };

    let x = smartcore::linalg::basic::matrix::DenseMatrix::from_2d_vec(&features)
        .expect("DenseMatrix failed");
    let rf_params = smartcore::ensemble::random_forest_regressor::RandomForestRegressorParameters {
        n_trees,
        max_depth: Some(max_depth),
        seed: 42,
        ..Default::default()
    };

    let (r_squared, mae, model) =
        match smartcore::ensemble::random_forest_regressor::RandomForestRegressor::fit(
            &x, &targets, rf_params,
        ) {
            Ok(model) => {
                let preds = model.predict(&x).unwrap_or_else(|_| targets.clone());
                let y_mean = targets.iter().sum::<f64>() / m as f64;
                let ss_res: f64 = targets
                    .iter()
                    .zip(preds.iter())
                    .map(|(y, p)| (y - p).powi(2))
                    .sum();
                let ss_tot: f64 = targets.iter().map(|y| (y - y_mean).powi(2)).sum();
                let r2 = if ss_tot > 0.0 {
                    1.0 - ss_res / ss_tot
                } else {
                    0.0
                };
                let err: f64 = targets
                    .iter()
                    .zip(preds.iter())
                    .map(|(y, p)| (y - p).abs())
                    .sum::<f64>()
                    / m as f64;
                (r2, err, Some(model))
            }
            Err(_) => return extrapolate_linear(historical_qtys, last_qty, quotes, last_known_dt),
        };

    debug!(
        "Random Forest: {} árvores (depth={}), {} amostras, R²={:.4}, MAE={:.2}",
        n_trees, max_depth, m, r_squared, mae
    );

    // Projeção recursiva com suavização
    let mut estimates = Vec::new();
    let mut prev_qty = historical_qtys[n - 1];
    let ema_alpha = 0.3;
    for (j, q) in quotes.iter().enumerate() {
        if q.date.as_str() > last_known_dt {
            let feat = vec![(n + j) as f64, prev_qty];
            let raw_pred = if let Some(ref model) = model {
                let fx_vec = vec![feat];
                let fx = smartcore::linalg::basic::matrix::DenseMatrix::from_2d_vec(&fx_vec)
                    .expect("DenseMatrix failed");
                model.predict(&fx).unwrap_or_else(|_| vec![prev_qty])[0].max(0.0)
            } else {
                prev_qty
            };
            let max_change = prev_qty * 0.30;
            let clamped = raw_pred.clamp(prev_qty - max_change, prev_qty + max_change);
            let pred_qty = (ema_alpha * clamped + (1.0 - ema_alpha) * prev_qty).max(0.0);
            estimates.push((q.date.clone(), pred_qty, pred_qty * q.close_price));
            prev_qty = pred_qty;
        }
    }

    let quality = ExtrapolationQuality {
        method: ExtrapolationMethod::RandomForest,
        r_squared: Some(r_squared),
        mae: Some(mae),
        confidence_95: None,
    };
    (estimates, quality)
}

// ── Original analytics (RF09, RF10, RF11) ───────────────────────────────

pub fn compute_asset_analytics_v1(df: &DataFrame, asset_code: &str) -> Option<AssetAnalytics> {
    let filtered_df = df
        .clone()
        .lazy()
        .filter(
            col("CD_ISIN")
                .eq(lit(asset_code))
                .or(col("CD_ATIVO").eq(lit(asset_code)))
                .or(col("TP_ATIVO").eq(lit(asset_code))),
        )
        .sort(
            "DT_COMPTC",
            SortOptions {
                descending: false,
                ..Default::default()
            },
        )
        .collect()
        .ok()?;

    if filtered_df.height() == 0 {
        return None;
    }

    let pct_pl_col = filtered_df.column("VL_PORCENTAGEM_PL").ok()?;
    let mut max_pct_pl = 0.0;
    let mut months_to_peak = 0;
    let mut current_streak = 0;
    let mut take_profit = 0.0;
    let mut reached_peak = false;

    for i in 0..filtered_df.height() {
        let pct = get_f64(pct_pl_col, i);
        if pct > max_pct_pl {
            max_pct_pl = pct;
            current_streak += 1;
        } else if pct < max_pct_pl && !reached_peak && current_streak > 0 {
            reached_peak = true;
            months_to_peak = current_streak;
            take_profit = max_pct_pl;
        }
    }

    Some(AssetAnalytics {
        avg_buy_price: 0.0,
        avg_sell_price: 0.0,
        take_profit_trigger: take_profit,
        speed_to_peak: months_to_peak,
        hidden_qty_estimates: Vec::new(),
        extrapolation_quality: None,
    })
}

pub fn compute_asset_analytics(
    df: &DataFrame,
    asset_code: &str,
    yahoo_prices: Option<&DataFrame>,
) -> Option<AssetAnalytics> {
    // Converter DataFrame da Yahoo para Vec<MonthlyQuote>
    let quotes: Option<Vec<crate::provider::yahoo::MonthlyQuote>> =
        yahoo_prices.and_then(|prices_df| {
            let date_col = prices_df.column("date").ok()?;
            let close_col = prices_df.column("adjclose").ok()?;
            let n = prices_df.height();
            let mut quotes = Vec::with_capacity(n);
            for i in 0..n {
                let date = date_col
                    .get(i)
                    .ok()
                    .and_then(|v| v.get_str().map(|s| s.to_string()))
                    .unwrap_or_default();
                let close = close_col
                    .get(i)
                    .ok()
                    .and_then(|v| v.try_extract::<f64>().ok())
                    .unwrap_or(0.0);
                if !date.is_empty() && close > 0.0 {
                    quotes.push(crate::provider::yahoo::MonthlyQuote {
                        date,
                        close_price: close,
                    });
                }
            }
            if quotes.is_empty() {
                None
            } else {
                Some(quotes)
            }
        });

    info!(
        "Analytics iniciado para '{}': {} linhas, quotes={}",
        asset_code,
        df.height(),
        quotes.as_ref().map(|q| q.len()).unwrap_or(0)
    );
    let filtered_df = df
        .clone()
        .lazy()
        .filter(
            col("CD_ISIN")
                .eq(lit(asset_code))
                .or(col("CD_ATIVO").eq(lit(asset_code))),
        )
        .sort(
            "DT_COMPTC",
            SortOptions {
                descending: false,
                ..Default::default()
            },
        )
        .collect()
        .ok()?;

    if filtered_df.height() == 0 {
        return None;
    }

    let qt_regis_col = filtered_df
        .column("QT_POS_FINAL")
        .ok()
        .or_else(|| filtered_df.column("QT_REGIS").ok())
        .or_else(|| filtered_df.column("QT_VENDA").ok())?;
    let pct_pl_col = filtered_df.column("VL_PORCENTAGEM_PL").ok()?;
    let vl_aquis_col = filtered_df.column("VL_AQUIS_NEGOC").ok();
    let vl_merc_col = filtered_df.column("VL_MERC_POS_FINAL").ok();

    let mut absolute_max_pct = 0.0;
    let mut index_of_peak = 0;
    let mut active_buying_months = 0;
    let mut months_counting_to_peak = 0;

    let height = filtered_df.height();
    let mut last_qty = 0.0;
    let mut last_aquis = 0.0;
    let mut total_buy_value = 0.0;
    let mut total_buy_qty = 0.0;
    let mut total_sell_value = 0.0;
    let mut total_sell_qty = 0.0;

    // Coletar quantidades históricas para extrapolação
    let mut historical_qtys: Vec<f64> = Vec::with_capacity(height);

    for i in 0..height {
        let current_qty = get_f64(qt_regis_col, i);
        let pct = get_f64(pct_pl_col, i);
        let current_aquis = vl_aquis_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0);
        let current_merc = vl_merc_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0);

        historical_qtys.push(current_qty);

        let est_price = if current_qty > 0.0 {
            current_merc / current_qty
        } else {
            0.0
        };

        if i == 0 {
            if current_qty > 0.0 {
                active_buying_months += 1;
                total_buy_qty += current_qty;
                total_buy_value += if current_aquis > 0.0 {
                    current_aquis
                } else {
                    current_qty * est_price
                };
            }
        } else {
            let delta_qty = current_qty - last_qty;
            let delta_aquis = current_aquis - last_aquis;
            if delta_qty > 0.0 {
                active_buying_months += 1;
                total_buy_qty += delta_qty;
                total_buy_value += if delta_aquis > 0.0 {
                    delta_aquis
                } else {
                    delta_qty * est_price
                };
            } else if delta_qty < 0.0 {
                total_sell_qty += delta_qty.abs();
                total_sell_value += delta_qty.abs() * est_price;
            }
        }

        if pct > absolute_max_pct {
            absolute_max_pct = pct;
            index_of_peak = i;
            months_counting_to_peak = active_buying_months;
        }

        last_qty = current_qty;
        last_aquis = current_aquis;
    }

    let mut take_profit = 0.0;
    if index_of_peak < height - 1 {
        let mut last_qty_post = get_f64(qt_regis_col, index_of_peak);
        for i in (index_of_peak + 1)..height {
            let current_qty = get_f64(qt_regis_col, i);
            if current_qty < last_qty_post {
                take_profit = absolute_max_pct;
                break;
            }
            last_qty_post = current_qty;
        }
    }

    let avg_buy_price = if total_buy_qty > 0.0 {
        total_buy_value / total_buy_qty
    } else {
        0.0
    };
    let avg_sell_price = if total_sell_qty > 0.0 {
        total_sell_value / total_sell_qty
    } else {
        0.0
    };

    // ── Extrapolação: testa todos os métodos e escolhe o melhor ─────
    let (hidden_qty_estimates, extrapolation_quality) = if let Some(ref quotes) = quotes {
        if historical_qtys.len() < 3 {
            debug!(
                "Analytics: poucos dados ({}) — pulando extrapolação",
                historical_qtys.len()
            );
            (
                Vec::new(),
                ExtrapolationQuality {
                    method: ExtrapolationMethod::Baseline,
                    r_squared: None,
                    mae: None,
                    confidence_95: None,
                },
            )
        } else {
            debug!("Analytics: quotes disponíveis ({} cotações)", quotes.len());
            if let Ok(dt_col) = filtered_df.column("DT_COMPTC") {
                if let Some(last_known_dt) = dt_col
                    .get(height - 1)
                    .ok()
                    .and_then(|v| v.get_str().map(|s| s.to_string()))
                {
                    info!(
                        "Analytics: executando extrapolação para {} ({} meses históricos, última qtd={:.0})",
                        asset_code, historical_qtys.len(), last_qty
                    );
                    extrapolate_best(&historical_qtys, last_qty, quotes, &last_known_dt)
                } else {
                    (
                        Vec::new(),
                        ExtrapolationQuality {
                            method: ExtrapolationMethod::Baseline,
                            r_squared: None,
                            mae: None,
                            confidence_95: None,
                        },
                    )
                }
            } else {
                (
                    Vec::new(),
                    ExtrapolationQuality {
                        method: ExtrapolationMethod::Baseline,
                        r_squared: None,
                        mae: None,
                        confidence_95: None,
                    },
                )
            }
        }
    } else {
        debug!("Analytics: sem quotes disponíveis — pulando extrapolação");
        (
            Vec::new(),
            ExtrapolationQuality {
                method: ExtrapolationMethod::Baseline,
                r_squared: None,
                mae: None,
                confidence_95: None,
            },
        )
    };

    Some(AssetAnalytics {
        avg_buy_price,
        avg_sell_price,
        take_profit_trigger: take_profit,
        speed_to_peak: months_counting_to_peak,
        hidden_qty_estimates,
        extrapolation_quality: Some(extrapolation_quality),
    })
}

fn get_f64(col: &Series, row: usize) -> f64 {
    col.get(row)
        .ok()
        .and_then(|v| {
            v.try_extract::<f64>().ok().or_else(|| {
                v.get_str()
                    .and_then(|s| s.replace(',', ".").parse::<f64>().ok())
            })
        })
        .unwrap_or(0.0)
}

// ── Top Assets Time Series ────────────────────────────────────────────

/// (key, display_name, points[(month_idx, pct)])
pub type TopSeriesRow = (String, String, Vec<(f64, f64)>);

/// Retorna os top 12 ativos do DataFrame CVM como (key, display_name, points[(month_idx, pct)])
pub fn compute_top_series(data: &DataFrame) -> Vec<TopSeriesRow> {
    let mut series = Vec::new();
    let height = data.height();
    if height == 0 {
        return series;
    }

    let df = data
        .clone()
        .lazy()
        .with_column(col("CD_ATIVO").fill_null(lit("")))
        .with_column(col("CD_ISIN").fill_null(lit("")))
        .with_column(
            when(col("CD_ATIVO").neq(lit("")))
                .then(col("CD_ATIVO"))
                .otherwise(col("CD_ISIN"))
                .alias("asset_key"),
        )
        .with_column(col("DT_COMPTC").str().str_slice(0, Some(7)).alias("month"))
        .filter(col("asset_key").neq(lit("")))
        .groupby(vec![col("month"), col("asset_key")])
        .agg(vec![
            col("VL_MERC_POS_FINAL").sum().alias("total_merc"),
            col("VL_PORCENTAGEM_PL").sum().alias("total_pct"),
        ])
        .sort("month", SortOptions::default())
        .collect();

    let grouped = match df {
        Ok(g) => g,
        Err(_) => return series,
    };
    if grouped.height() == 0 {
        return series;
    }

    let mut asset_totals: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    let merc_col_g = grouped.column("total_merc").ok();
    let key_col_g = grouped.column("asset_key").ok();
    if let (Some(mc), Some(kc)) = (merc_col_g, key_col_g) {
        for i in 0..grouped.height() {
            let key = get_str(kc, i);
            let val = get_f64(mc, i);
            *asset_totals.entry(key).or_default() += val;
        }
    }
    let mut sorted: Vec<(String, f64)> = asset_totals.into_iter().collect();
    sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_keys: Vec<String> = sorted.into_iter().take(12).map(|(k, _)| k).collect();

    let asset_names = build_asset_names(data, &top_keys);

    let merc_col = grouped.column("total_merc").ok();
    let pct_col = grouped.column("total_pct").ok();
    let akey_col = grouped.column("asset_key").ok();

    for key in &top_keys {
        let name = asset_names.get(key).cloned().unwrap_or_else(|| key.clone());
        let mut points: Vec<(f64, f64)> = vec![];
        for mi in 0..grouped.height() {
            if let (Some(mc), Some(ac)) = (merc_col, akey_col) {
                if get_str(ac, mi) != *key {
                    continue;
                }
                let month_idx = mi as f64;
                let pct = if let Some(pc) = pct_col {
                    get_f64(pc, mi)
                } else {
                    get_f64(mc, mi)
                };
                points.push((month_idx, pct));
            }
        }
        points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        series.push((key.clone(), name, points));
    }
    series
}

fn build_asset_names(
    data: &DataFrame,
    top_keys: &[String],
) -> std::collections::HashMap<String, String> {
    let mut names = std::collections::HashMap::new();
    let cd_ativo_col = data.column("CD_ATIVO").ok();
    let cd_isin_col = data.column("CD_ISIN").ok();
    let ds_ativo_col = data.column("DS_ATIVO").ok();
    let nm_fundo_col = data.column("NM_FUNDO_COTA").ok();
    let titpub_col = data.column("TP_TITPUB").ok();

    for i in 0..data.height() {
        let cd_ativo = cd_ativo_col
            .as_ref()
            .map(|c| get_str(c, i))
            .unwrap_or_default();
        let cd_isin = cd_isin_col
            .as_ref()
            .map(|c| get_str(c, i))
            .unwrap_or_default();
        let key = if !cd_ativo.is_empty() {
            cd_ativo.clone()
        } else {
            cd_isin.clone()
        };
        if key.is_empty() || !top_keys.contains(&key) || names.contains_key(&key) {
            continue;
        }
        let ds = ds_ativo_col
            .as_ref()
            .map(|c| get_str(c, i))
            .unwrap_or_default();
        let nf = nm_fundo_col
            .as_ref()
            .map(|c| get_str(c, i))
            .unwrap_or_default();
        let tp = titpub_col
            .as_ref()
            .map(|c| get_str(c, i))
            .unwrap_or_default();
        let mut name = if !tp.is_empty() {
            tp
        } else if !ds.is_empty() {
            ds
        } else if !nf.is_empty() {
            nf
        } else {
            key.clone()
        };
        if !cd_ativo.is_empty() && name != cd_ativo && !name.starts_with(&cd_ativo) {
            name = format!("{} - {}", cd_ativo, name);
        }
        names.insert(key, name);
    }
    names
}

fn get_str(col: &Series, row: usize) -> String {
    col.get(row)
        .ok()
        .and_then(|v| v.get_str().map(|s| s.to_string()))
        .unwrap_or_default()
}
