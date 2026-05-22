use log::{debug, info};
use polars::prelude::*;

// ── Analytics Structures ─────────────────────────────────────────────────

#[derive(Clone)]
pub struct AssetAnalytics {
    pub avg_buy_price: f64,
    pub avg_sell_price: f64,
    pub take_profit_trigger: f64,
    pub speed_to_peak: usize,
    pub hidden_qty_estimates: Vec<(String, f64, f64)>,
    pub portfolio_inference: Option<PortfolioInference>,
}

/// Inferência de estado da carteira (Holding Persistence Estimator)
#[derive(Clone, Debug)]
pub struct PortfolioInference {
    /// Último dado real observado no balanço (âncora)
    pub baseline_qty: f64,
    /// Estimativa suavizada do filtro após consumir o histórico
    pub filtered_qty: f64,
    /// Score de inferibilidade: 1.0 = perfeitamente previsível, → 0 se caótico
    pub stability_score: f64,
    /// Z-Score da previsão: quantos desvios-padrão a previsão está da baseline
    pub z_score: f64,
    /// Direção do viés estatisticamente validada (> 2.0 sigma)
    pub bias_direction: TradeBias,
    /// Projeções multi-horizon com incerteza crescente (data, qtd, ci_lower, ci_upper)
    pub forward_trajectory: Vec<(String, f64, f64, f64)>,
    /// Histórico de estados filtrados (data, real_qty, filtered_qty, ci_lower, ci_upper)
    pub historical_states: Vec<(String, f64, f64, f64, f64)>,
    /// Histórico de Z-Scores
    pub historical_z_scores: Vec<f64>,
}

impl PortfolioInference {
    /// Exporta o ciclo de vida completo da inferência metrológica para CSV (Passado + Futuro).
    /// Os arquivos serão salvos seguindo o padrão acadêmico: results/FUNDO/ATIVO/metrology.csv
    pub fn export_to_csv(&self, cnpj: &str, asset: &str) -> std::io::Result<()> {
        use std::io::Write;

        let safe_cnpj = cnpj.replace(&['/', '.', '-'][..], "");
        let dir_path = format!("results/{}/{}", safe_cnpj, asset);
        std::fs::create_dir_all(&dir_path)?;

        let file_path = format!("{}/metrology.csv", dir_path);
        let mut file = std::fs::File::create(file_path)?;

        writeln!(
            file,
            "fund_id,asset,t_dt,real_qty,filtered_qty,ci_lower,ci_upper,z_score,residual,is_future"
        )?;

        for (i, (dt, real, filtered, lower, upper)) in self.historical_states.iter().enumerate() {
            let z = self.historical_z_scores.get(i).copied().unwrap_or(0.0);
            let residual = real - filtered;
            writeln!(
                file,
                "{},{},{},{:.4},{:.4},{:.4},{:.4},{:.4},{:.4},0",
                safe_cnpj, asset, dt, real, filtered, lower, upper, z, residual
            )?;
        }

        for (dt, filtered, lower, upper) in &self.forward_trajectory {
            // Em dados futuros, o real_qty, residual e z_score não existem (ficam vazios/zerados)
            writeln!(
                file,
                "{},{},{},,{:.4},{:.4},{:.4},0.0,0.0,1",
                safe_cnpj, asset, dt, filtered, lower, upper
            )?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TradeBias {
    Acumulando,
    Distribuindo,
    Consistente,
}

impl std::fmt::Display for TradeBias {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Acumulando => write!(f, "Comprando"),
            Self::Distribuindo => write!(f, "Vendendo"),
            Self::Consistente => write!(f, "Estável"),
        }
    }
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

/// Compara todos os métodos de extrapolação e retorna o melhor (maior R² legítimo).
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
    let mut best_r2: f64 = f64::MIN;

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

        // Define o Baseline como marco zero (0.0). Os outros precisam vencê-lo.
        let r2 = match quality.method {
            ExtrapolationMethod::Baseline => 0.0,
            _ => quality.r_squared.unwrap_or(-100.0),
        };

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

    if let Some((estimates, quality)) = best_result {
        // Se nenhum modelo dinâmico superou o marco zero com segurança, força o Baseline
        if quality.method != ExtrapolationMethod::Baseline && best_r2 < 0.01 {
            info!(
                "Extrapolação: nenhum método dinâmico aprendeu o padrão (melhor R²={:.4}) — forçando fallback para baseline",
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
        return (estimates, quality);
    }

    info!("Extrapolação: todos os métodos falharam criticamente, gerando baseline");
    extrapolate_baseline(quotes, last_known_dt, last_qty)
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

// ── Kalman MLE (Maximum Likelihood Estimation) ────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct KalmanParams {
    pub q_pos: f64,
    pub r_obs: f64,
    pub r_obs_aux: f64,
    pub gamma: f64,
}

fn compute_log_likelihood(
    log_historical: &[f64],
    log_implied: &[f64],
    p: &KalmanParams,
    has_aux: bool,
) -> f64 {
    let meas_dim = if has_aux { 2 } else { 1 };
    let obs_matrix = if has_aux {
        vec![1.0, 0.0, 1.0, 0.0]
    } else {
        vec![1.0, 0.0]
    };
    let meas_noise = if has_aux {
        vec![p.r_obs, 0.0, 0.0, p.r_obs_aux]
    } else {
        vec![p.r_obs]
    };

    let mut kf = kalman_filters::KalmanFilterBuilder::<f64>::new(2, meas_dim)
        .initial_state(vec![log_historical[0], 0.0])
        .initial_covariance(vec![0.5, 0.0, 0.0, 0.1])
        .transition_matrix(vec![1.0, 1.0, 0.0, p.gamma])
        .process_noise(vec![p.q_pos, 0.0, 0.0, 0.01])
        .observation_matrix(obs_matrix)
        .measurement_noise(meas_noise)
        .build()
        .unwrap_or_else(|_| panic!("MLE build failed: {:?}", p));

    let mut log_likelihood = 0.0;

    for i in 1..log_historical.len() {
        kf.predict();

        let y_obs = log_historical[i];
        let pred_y = kf.state()[0];
        let innovation = y_obs - pred_y;

        let innovation_variance = kf.covariance()[0] + p.r_obs;
        let safe_variance = innovation_variance.max(1e-9);

        let step_ll = -0.5
            * ((2.0 * std::f64::consts::PI * safe_variance).ln()
                + (innovation.powi(2) / safe_variance));
        log_likelihood += step_ll;

        // Garante que o update respeite a dimensão do filtro
        if has_aux {
            let _ = kf.update(&[y_obs, log_implied[i]]);
        } else {
            let _ = kf.update(&[y_obs]);
        }
    }
    log_likelihood
}

fn optimize_kalman_parameters(
    log_historical: &[f64],
    log_implied: &[f64],
    has_aux: bool,
) -> KalmanParams {
    let q_grid = [0.01, 0.05, 0.10, 0.15, 0.25];
    let r_grid = [0.01, 0.05, 0.10, 0.20, 0.40];
    let r_aux_grid = [0.10, 0.20, 0.30, 0.50];
    let gamma_grid = [0.50, 0.70, 0.85, 0.95];

    let mut best = KalmanParams {
        q_pos: 0.15,
        r_obs: 0.10,
        r_obs_aux: 0.30,
        gamma: 0.85,
    };
    let mut max_ll = f64::NEG_INFINITY;

    for &q in &q_grid {
        for &r in &r_grid {
            for &g in &gamma_grid {
                if has_aux {
                    for &ra in &r_aux_grid {
                        let p = KalmanParams {
                            q_pos: q,
                            r_obs: r,
                            r_obs_aux: ra,
                            gamma: g,
                        };
                        let ll = compute_log_likelihood(log_historical, log_implied, &p, true);
                        if ll > max_ll {
                            max_ll = ll;
                            best = p;
                        }
                    }
                } else {
                    let p = KalmanParams {
                        q_pos: q,
                        r_obs: r,
                        r_obs_aux: 0.30,
                        gamma: g,
                    };
                    let ll = compute_log_likelihood(log_historical, log_implied, &p, false);
                    if ll > max_ll {
                        max_ll = ll;
                        best = p;
                    }
                }
            }
        }
    }

    debug!(
        "Kalman MLE: Q={:.3} R={:.3} γ={:.3} LL={:.2}",
        best.q_pos, best.r_obs, best.gamma, max_ll
    );
    best
}

// ── Motor de Inferência de Portfólio (Holding Persistence Estimator) ──

pub fn infer_portfolio_state(
    historical_qtys: &[f64],
    historical_implied: &[f64],
    quotes: &[crate::provider::yahoo::MonthlyQuote],
    last_known_dt: &str,
    historical_dates: &[String],
) -> PortfolioInference {
    let n = historical_qtys.len();
    let baseline_qty = historical_qtys.last().copied().unwrap_or(0.0);

    let qty_min = historical_qtys
        .iter()
        .cloned()
        .fold(f64::INFINITY, f64::min);
    let qty_max = historical_qtys.iter().cloned().fold(0.0_f64, f64::max);
    info!(
        "Inferência iniciada: {} meses, qtd=[{:.0}..{:.0}], baseline={:.0}, quotes_futuras={}",
        n,
        qty_min,
        qty_max,
        baseline_qty,
        quotes.len()
    );

    if n < 3 {
        info!(
            "Inferência: poucos dados ({}), usando trajetória estática",
            n
        );
        return PortfolioInference {
            baseline_qty,
            filtered_qty: baseline_qty,
            stability_score: 1.0,
            z_score: 0.0,
            bias_direction: TradeBias::Consistente,
            forward_trajectory: generate_static_trajectory(baseline_qty, quotes, last_known_dt),
            historical_states: Vec::new(),
            historical_z_scores: Vec::new(),
        };
    }

    // 1. Log-Transform
    let log_historical: Vec<f64> = historical_qtys.iter().map(|&q| q.max(1.0).ln()).collect();

    // Canal dual: exige integridade total (100% dos meses com PL + Yahoo)
    let has_aux =
        historical_implied.len() == n && historical_implied.iter().all(|&v| v > 0.0 && !v.is_nan());
    info!(
        "Inferência: canal dual {} ({} observações implícitas)",
        if has_aux { "ATIVO" } else { "desligado" },
        if has_aux { n } else { 0 }
    );
    let log_implied: Vec<f64> = if has_aux {
        historical_implied
            .iter()
            .map(|&q| q.max(1.0).ln())
            .collect()
    } else {
        vec![]
    };

    // MLE
    let optimal = optimize_kalman_parameters(&log_historical, &log_implied, has_aux);

    // Filtro 2D [pos, vel] com 1 ou 2 canais de observação
    let meas_dim = if has_aux { 2_usize } else { 1_usize };
    let obs_matrix = if has_aux {
        vec![1.0, 0.0, 1.0, 0.0]
    } else {
        vec![1.0, 0.0]
    };
    let meas_noise = if has_aux {
        vec![optimal.r_obs, 0.0, 0.0, optimal.r_obs_aux]
    } else {
        vec![optimal.r_obs]
    };

    let mut kf = kalman_filters::KalmanFilterBuilder::<f64>::new(2, meas_dim)
        .initial_state(vec![log_historical[0], 0.0])
        .initial_covariance(vec![0.5, 0.0, 0.0, 0.1])
        .transition_matrix(vec![1.0, 1.0, 0.0, optimal.gamma])
        .process_noise(vec![optimal.q_pos, 0.0, 0.0, 0.01])
        .observation_matrix(obs_matrix)
        .measurement_noise(meas_noise)
        .build()
        .expect("Inference Kalman build failed");

    let mut log_predictions = Vec::with_capacity(n - 1);
    let mut log_actuals = Vec::with_capacity(n - 1);

    let mut historical_states = Vec::new();
    let mut historical_z_scores = Vec::new();

    let initial_cov = kf.covariance()[0];
    let initial_std = if initial_cov > 1e-6 {
        initial_cov.sqrt() * 1.5
    } else {
        0.05
    };
    historical_states.push((
        historical_dates[0].clone(),
        log_historical[0].exp(),
        kf.state()[0].exp(),
        (kf.state()[0] - 1.96 * initial_std).exp().max(0.0),
        (kf.state()[0] + 1.96 * initial_std).exp(),
    ));
    historical_z_scores.push(0.0);

    for i in 1..n {
        kf.predict();
        log_predictions.push(kf.state()[0]);
        log_actuals.push(log_historical[i]);

        // Se has_aux, log_implied[i] existe garantido (.all() validou)
        if has_aux {
            kf.update(&[log_historical[i], log_implied[i]])
                .expect("Kalman update dual failed");
        } else {
            kf.update(&[log_historical[i]])
                .expect("Kalman update failed");
        }

        let filtered_log = kf.state()[0];
        let cov = kf.covariance()[0];
        let std = if cov > 1e-6 { cov.sqrt() * 1.5 } else { 0.05 };

        let drift = kf.state()[1];
        let drift_cov = kf.covariance()[3];
        let drift_std = if drift_cov > 1e-6 {
            drift_cov.sqrt()
        } else {
            0.05
        };
        let z_score = drift / drift_std;

        historical_states.push((
            historical_dates[i].clone(),
            log_historical[i].exp(),
            filtered_log.exp(),
            (filtered_log - 1.96 * std).exp().max(0.0),
            (filtered_log + 1.96 * std).exp(),
        ));
        historical_z_scores.push(z_score);
    }

    let filtered_log_qty = kf.state()[0];

    // 2. Stability Score
    let m = log_actuals.len();
    let mut relative_error_sum = 0.0;
    for i in 0..m {
        let actual = log_actuals[i].exp();
        let pred = log_predictions[i].exp();
        relative_error_sum += ((actual - pred) / actual.max(1.0)).abs();
    }
    let discrepancy = relative_error_sum / m as f64;
    let stability_score = (-2.0 * discrepancy).exp();

    // 3. Multi-Horizon Forecast
    let mut forward_trajectory = Vec::new();
    let inflation_factor = 1.5;

    kf.predict();

    // Z-Score calc baseado no Drift (Velocidade) ao invés do delta posicional
    let drift = kf.state()[1];
    let drift_cov = kf.covariance()[3];
    let drift_std = if drift_cov > 1e-6 {
        drift_cov.sqrt()
    } else {
        0.05
    };
    let z_score = drift / drift_std;

    let bias_direction = if z_score > 2.0 {
        TradeBias::Acumulando
    } else if z_score < -2.0 {
        TradeBias::Distribuindo
    } else {
        TradeBias::Consistente
    };

    for q in quotes {
        if q.date.as_str() > last_known_dt {
            let pred_nat = kf.state()[0].exp();
            let current_cov = kf.covariance()[0];
            let current_std = if current_cov > 1e-6 {
                current_cov.sqrt() * inflation_factor
            } else {
                0.05
            };
            let ci_lower = (kf.state()[0] - 1.96 * current_std).exp().max(0.0);
            let ci_upper = (kf.state()[0] + 1.96 * current_std).exp();
            forward_trajectory.push((q.date.clone(), pred_nat, ci_lower, ci_upper));
            kf.predict();
        }
    }

    let first_ci = forward_trajectory.first().map(|f| (f.2, f.3));
    let last_ci = forward_trajectory.last().map(|f| (f.2, f.3));
    info!(
        "Inferência concluída: filtrado={:.0}, estab={:.1}%, viés={:?}, Z={:+.2}, traj={} passos, IC95_inicial={:?}, IC95_final={:?}",
        filtered_log_qty.exp(),
        stability_score * 100.0,
        bias_direction,
        z_score,
        forward_trajectory.len(),
        first_ci,
        last_ci
    );

    PortfolioInference {
        baseline_qty,
        filtered_qty: filtered_log_qty.exp(),
        stability_score,
        z_score,
        bias_direction,
        forward_trajectory,
        historical_states,
        historical_z_scores,
    }
}

fn generate_static_trajectory(
    baseline_qty: f64,
    quotes: &[crate::provider::yahoo::MonthlyQuote],
    last_known_dt: &str,
) -> Vec<(String, f64, f64, f64)> {
    let mut traj = Vec::new();
    for q in quotes {
        if q.date.as_str() > last_known_dt {
            traj.push((
                q.date.clone(),
                baseline_qty,
                baseline_qty * 0.90,
                baseline_qty * 1.10,
            ));
        }
    }
    traj
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

    if r_squared < 0.1 {
        debug!(
            "Regressão Linear: R² muito baixo ({:.4}), usando baseline",
            r_squared
        );
        let (estimates, _) = extrapolate_baseline(quotes, last_known_dt, last_qty);
        let quality = ExtrapolationQuality {
            method: ExtrapolationMethod::Baseline,
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

// ── Filtro de Kalman Local Level (1D em escala Log) ─────────────────────────

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

    // 1. Aplicação do Log-Transform
    let log_historical: Vec<f64> = historical_qtys.iter().map(|&q| q.max(1.0).ln()).collect();

    let q_pos = 0.15;
    let r_obs = 0.10;
    let init_var = 0.5;

    let mut kf = kalman_filters::KalmanFilterBuilder::<f64>::new(1, 1)
        .initial_state(vec![log_historical[0]])
        .initial_covariance(vec![init_var])
        .transition_matrix(vec![1.0])
        .process_noise(vec![q_pos])
        .observation_matrix(vec![1.0])
        .measurement_noise(vec![r_obs])
        .build()
        .expect("KalmanFilter 1D build failed");

    let mut prior_predictions_native = Vec::with_capacity(n - 1);
    let mut actual_evaluated_native = Vec::with_capacity(n - 1);

    for &y_log_obs in log_historical.iter().skip(1) {
        kf.predict();

        // Extrai o estado e garante a conversão nativa de forma explícita aqui
        let pred_log = kf.state()[0];

        // Se kf.state() por algum motivo bizarro já estiver retornando nativo,
        // checamos o tamanho para evitar distorção de escala
        let final_pred_native = if pred_log > 50.0 {
            pred_log
        } else {
            pred_log.exp()
        };

        prior_predictions_native.push(final_pred_native);
        actual_evaluated_native.push(y_log_obs.exp());

        kf.update(&[y_log_obs]).expect("Kalman update failed");
    }

    let final_state = kf.state().to_vec();
    let final_cov = kf.covariance().to_vec();

    // 2. Cálculo do R² Geométrico/Relativo (Calculado de forma direta e blindada)
    let m = actual_evaluated_native.len();

    // Vamos calcular o erro com base na variação percentual para neutralizar o efeito dos picos de 10x
    let mut ss_res_rel = 0.0;
    let mut ss_tot_rel = 0.0;

    let y_mean_native = actual_evaluated_native.iter().sum::<f64>() / m as f64;

    for i in 0..m {
        // Erro relativo proporcional ao tamanho do ativo
        let actual = actual_evaluated_native[i];
        let pred = prior_predictions_native[i];

        ss_res_rel += ((actual - pred) / actual.max(1.0)).powi(2);
        ss_tot_rel += ((actual - y_mean_native) / actual.max(1.0)).powi(2);
    }

    // R² proporcional geométrico. Protege contra o Viés de Jensen no exp()
    let kf_r_squared = if ss_tot_rel > 0.0 {
        (1.0 - (ss_res_rel / ss_tot_rel)).clamp(-1.0, 1.0)
    } else {
        0.0
    };

    // MAE nativo tradicional
    let mut total_absolute_error = 0.0;
    for i in 0..m {
        total_absolute_error += (actual_evaluated_native[i] - prior_predictions_native[i]).abs();
    }
    let kf_mae = total_absolute_error / m.max(1) as f64;

    // Intervalo de confiança
    let std_dev_log = if !final_cov.is_empty() && final_cov[0] > 0.001 {
        final_cov[0].sqrt()
    } else {
        0.20 // Fallback estável de desvio log
    };

    let final_pos_log = final_state[0];
    let final_pos_native = if final_pos_log > 50.0 {
        final_pos_log
    } else {
        final_pos_log.exp()
    };

    let ci_lower = if final_pos_log > 50.0 {
        final_pos_log * 0.7
    } else {
        (final_pos_log - 1.96 * std_dev_log).exp().max(0.0)
    };
    let ci_upper = if final_pos_log > 50.0 {
        final_pos_log * 1.3
    } else {
        (final_pos_log + 1.96 * std_dev_log).exp()
    };

    debug!(
        "Kalman 1D: nível_final={:.2}, IC95=[{:.0}, {:.0}], R²={:.4}, MAE={:.2}",
        final_pos_native, ci_lower, ci_upper, kf_r_squared, kf_mae
    );

    let mut estimates = Vec::new();
    let pred_qty_native = final_pos_native;

    for q in quotes {
        if q.date.as_str() > last_known_dt {
            estimates.push((
                q.date.clone(),
                pred_qty_native,
                pred_qty_native * q.close_price,
            ));
        }
    }

    let quality = ExtrapolationQuality {
        method: ExtrapolationMethod::KalmanFilter,
        r_squared: Some(kf_r_squared),
        mae: Some(kf_mae),
        confidence_95: Some((ci_lower, ci_upper)),
    };
    (estimates, quality)
}

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
        keep_samples: true,
        ..Default::default()
    };

    let (r_squared, mae, model) =
        match smartcore::ensemble::random_forest_regressor::RandomForestRegressor::fit(
            &x, &targets, rf_params,
        ) {
            Ok(model) => {
                let preds = model
                    .predict_oob(&x)
                    .or_else(|_| model.predict(&x))
                    .unwrap_or_else(|_| targets.clone());
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
        portfolio_inference: None,
    })
}

pub fn compute_asset_analytics(
    df: &DataFrame,
    asset_code: &str,
    yahoo_prices: Option<&DataFrame>,
    cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Option<AssetAnalytics> {
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

    if let Some(c) = &cancel {
        if c.load(std::sync::atomic::Ordering::Relaxed) {
            return None;
        }
    }

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
    let mut total_buy_value = 0.0;
    let mut total_buy_qty = 0.0;
    let mut total_sell_value = 0.0;
    let mut total_sell_qty = 0.0;

    let mut historical_qtys: Vec<f64> = Vec::with_capacity(height);
    let mut historical_pcts: Vec<f64> = Vec::with_capacity(height);
    let mut historical_pl: Vec<f64> = Vec::with_capacity(height);
    let pl_col = filtered_df.column("VL_PATRIM_LIQ").ok();

    for i in 0..height {
        if let Some(c) = &cancel {
            if c.load(std::sync::atomic::Ordering::Relaxed) {
                return None;
            }
        }
        let current_qty = get_f64(qt_regis_col, i);
        let pct = get_f64(pct_pl_col, i);
        let current_aquis = vl_aquis_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0);
        let current_merc = vl_merc_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0);

        historical_qtys.push(current_qty);
        historical_pcts.push(pct);
        let est_price = if current_qty > 0.0 {
            current_merc / current_qty
        } else {
            0.0
        };
        historical_pl.push(pl_col.as_ref().map(|c| get_f64(c, i)).unwrap_or(0.0));

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
            if delta_qty > 0.0 {
                active_buying_months += 1;
                total_buy_qty += delta_qty;
                total_buy_value += if current_aquis > 0.0 {
                    current_aquis
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

    // ── Inferência de Portfólio ─────────────────────────────────────
    let (hidden_qty_estimates, portfolio_inference) = if let Some(ref quotes) = quotes {
        if historical_qtys.len() >= 3 {
            debug!("Analytics: quotes disponíveis ({} cotações)", quotes.len());
            if let Ok(dt_col) = filtered_df.column("DT_COMPTC") {
                if let Some(last_known_dt) = dt_col
                    .get(height - 1)
                    .ok()
                    .and_then(|v| v.get_str().map(|s| s.to_string()))
                {
                    info!(
                        "Analytics: inferindo estado para {} ({} meses, qtd={:.0})",
                        asset_code,
                        historical_qtys.len(),
                        last_qty
                    );
                    // Baseline: valor estimado hoje (âncora de persistência)
                    let mut estimates = Vec::new();
                    for q in quotes {
                        if q.date.as_str() > last_known_dt.as_str() && last_qty > 0.0 {
                            estimates.push((q.date.clone(), last_qty, last_qty * q.close_price));
                        }
                    }
                    // Inferência de estado com observação dual (PL mensal + Yahoo)
                    // Alinha Yahoo mensal para quebrar circularidade
                    let has_aux = historical_pl.iter().any(|&pl| pl > 0.0) && !quotes.is_empty();
                    let yahoo_monthly: std::collections::HashMap<String, f64> = if has_aux {
                        let mut map = std::collections::HashMap::new();
                        for q in quotes {
                            if q.date.len() >= 7 {
                                let month = &q.date[..7];
                                map.insert(month.to_string(), q.close_price);
                            }
                        }
                        map
                    } else {
                        std::collections::HashMap::new()
                    };
                    // Extrai meses do histórico (mesmo formato YYYY-MM do DT_COMPTC)
                    let dt_col = filtered_df.column("DT_COMPTC").ok();
                    let historical_implied: Vec<f64> = if has_aux {
                        historical_pcts
                            .iter()
                            .enumerate()
                            .map(|(i, &pct)| {
                                let pl = historical_pl[i];
                                let month = dt_col
                                    .and_then(|c| c.get(i).ok())
                                    .and_then(|v| v.get_str().map(|s| s.to_string()))
                                    .unwrap_or_default();
                                let month_key = if month.len() >= 7 { &month[..7] } else { "" };
                                let yahoo_price =
                                    yahoo_monthly.get(month_key).copied().unwrap_or(0.0);
                                if pct > 0.0 && pl > 0.0 && yahoo_price > 0.0 {
                                    (pct / 100.0 * pl / yahoo_price).max(1.0)
                                } else {
                                    f64::NAN
                                }
                            })
                            .collect()
                    } else {
                        vec![]
                    };

                    let mut historical_dates: Vec<String> =
                        Vec::with_capacity(historical_qtys.len());
                    for i in 0..historical_qtys.len() {
                        let month = dt_col
                            .and_then(|c| c.get(i).ok())
                            .and_then(|v| v.get_str().map(|s| s.to_string()))
                            .unwrap_or_default();
                        historical_dates.push(month);
                    }

                    let implied_count = historical_implied.iter().filter(|v| !v.is_nan()).count();
                    info!(
                        "Analytics: observações implícitas geradas: {}/{} meses",
                        implied_count,
                        historical_implied.len()
                    );

                    let inference = infer_portfolio_state(
                        &historical_qtys,
                        &historical_implied,
                        quotes,
                        &last_known_dt,
                        &historical_dates,
                    );
                    debug!(
                        "Portfolio Inference: estabilidade={:.2}, viés={:?}",
                        inference.stability_score, inference.bias_direction
                    );
                    (estimates, Some(inference))
                } else {
                    (Vec::new(), None)
                }
            } else {
                (Vec::new(), None)
            }
        } else {
            debug!(
                "Analytics: poucos dados ({}) — pulando inferência",
                historical_qtys.len()
            );
            (Vec::new(), None)
        }
    } else {
        debug!("Analytics: sem quotes — pulando inferência");
        (Vec::new(), None)
    };

    let cnpj = df
        .column("CNPJ_FUNDO")
        .ok()
        .and_then(|c| c.get(0).ok())
        .and_then(|v| v.get_str().map(|s| s.to_string()))
        .unwrap_or_else(|| "UNKNOWN_FUND".to_string());

    if let Some(ref inf) = portfolio_inference {
        if let Err(e) = inf.export_to_csv(&cnpj, asset_code) {
            log::error!(
                "Erro ao exportar CSV metrológico para {}: {}",
                asset_code,
                e
            );
        } else {
            info!(
                "CSV metrológico exportado com sucesso para {}/{}",
                cnpj, asset_code
            );
        }
    }

    Some(AssetAnalytics {
        avg_buy_price,
        avg_sell_price,
        take_profit_trigger: take_profit,
        speed_to_peak: months_counting_to_peak,
        hidden_qty_estimates,
        portfolio_inference,
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

pub type TopSeriesRow = (String, String, Vec<(f64, f64)>);

pub fn compute_top_series(
    data: &DataFrame,
    cancel: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
) -> Vec<TopSeriesRow> {
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
        if let Some(c) = &cancel {
            if c.load(std::sync::atomic::Ordering::Relaxed) {
                return Vec::new();
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use polars::prelude::*;

    #[test]
    fn test_submarino_em_linha_reta() {
        // Cenário: Submarino em linha reta logarítmica (Acumulação estrita)
        // A quantidade dobra a cada mês (crescimento exponencial = rampa linear no espaço log)
        let cd_ativo = Series::new("CD_ATIVO", vec!["SUBMARINO"; 10]);
        let dt_comptc = Series::new(
            "DT_COMPTC",
            vec![
                "2024-01", "2024-02", "2024-03", "2024-04", "2024-05", "2024-06", "2024-07",
                "2024-08", "2024-09", "2024-10",
            ],
        );
        let qt_pos = Series::new(
            "QT_POS_FINAL",
            vec![
                100.0, 200.0, 400.0, 800.0, 1600.0, 3200.0, 6400.0, 12800.0, 25600.0, 51200.0,
            ],
        );
        let pct_pl = Series::new("VL_PORCENTAGEM_PL", vec![1.0; 10]);
        let vl_aquis = Series::new("VL_AQUIS_NEGOC", vec![1000.0; 10]);
        let vl_merc = Series::new(
            "VL_MERC_POS_FINAL",
            vec![
                1000.0, 2000.0, 3000.0, 4000.0, 5000.0, 6000.0, 7000.0, 8000.0, 9000.0, 10000.0,
            ],
        );
        let pl = Series::new("VL_PATRIM_LIQ", vec![100000.0; 10]);
        let cd_isin = Series::new("CD_ISIN", vec![""; 10]);

        let df = DataFrame::new(vec![
            cd_ativo, cd_isin, dt_comptc, qt_pos, pct_pl, vl_aquis, vl_merc, pl,
        ])
        .unwrap();

        // 3 cotações no "futuro" para inferência multi-horizonte e 10 de histórico
        let q_date = Series::new(
            "date",
            vec![
                "2024-01-01",
                "2024-02-01",
                "2024-03-01",
                "2024-04-01",
                "2024-05-01",
                "2024-06-01",
                "2024-07-01",
                "2024-08-01",
                "2024-09-01",
                "2024-10-01",
                "2024-11-01",
                "2024-12-01",
                "2025-01-01",
            ],
        );
        let q_close = Series::new("adjclose", vec![10.0; 13]);
        let quotes_df = DataFrame::new(vec![q_date, q_close]).unwrap();

        let analytics = compute_asset_analytics(&df, "SUBMARINO", Some(&quotes_df), None)
            .expect("Analytics failed");

        let inference = analytics
            .portfolio_inference
            .expect("Sem PortfolioInference!");

        println!("DEBUG Z-SCORE: {}", inference.z_score);
        println!("DEBUG BIAS: {:?}", inference.bias_direction);

        assert_eq!(
            inference.bias_direction,
            TradeBias::Acumulando,
            "Filtro falhou em detectar acumulação direcional forte."
        );
        assert!(
            inference.z_score > 2.0,
            "O Z-Score ({:.2}) deveria ser altamente significativo (> 2.0) para uma rampa limpa.",
            inference.z_score
        );
    }

    #[test]
    fn test_log_transform_zero() {
        // Teste de Log-Transform: 0.0 não causa NaN ou -inf
        let cd_ativo = Series::new("CD_ATIVO", vec!["SUBMARINO"; 5]);
        let dt_comptc = Series::new(
            "DT_COMPTC",
            vec!["2024-01", "2024-02", "2024-03", "2024-04", "2024-05"],
        );
        let qt_pos = Series::new("QT_POS_FINAL", vec![0.0; 5]);
        let pct_pl = Series::new("VL_PORCENTAGEM_PL", vec![0.0; 5]);
        let vl_aquis = Series::new("VL_AQUIS_NEGOC", vec![0.0; 5]);
        let vl_merc = Series::new("VL_MERC_POS_FINAL", vec![0.0; 5]);
        let pl = Series::new("VL_PATRIM_LIQ", vec![100000.0; 5]);
        let cd_isin = Series::new("CD_ISIN", vec![""; 5]);

        let df = DataFrame::new(vec![
            cd_ativo, cd_isin, dt_comptc, qt_pos, pct_pl, vl_aquis, vl_merc, pl,
        ])
        .unwrap();

        let q_date = Series::new("date", vec!["2024-06-01"]);
        let q_close = Series::new("adjclose", vec![10.0]);
        let quotes_df = DataFrame::new(vec![q_date, q_close]).unwrap();

        let analytics = compute_asset_analytics(&df, "SUBMARINO", Some(&quotes_df), None)
            .expect("Analytics failed");
        let inference = analytics
            .portfolio_inference
            .expect("No inference generated");

        assert_eq!(
            inference.bias_direction,
            TradeBias::Consistente,
            "Viés de posições zeradas deve ser Consistente."
        );
        for (_, qty, _, _) in inference.forward_trajectory {
            assert!(
                !qty.is_nan(),
                "Trajetória gerou NaN em quantidades zeradas."
            );
        }
    }

    #[test]
    fn test_invariancia_de_escala() {
        // Mesmo cenário log-linear, mas quantidades multiplicadas por 1000
        let cd_ativo = Series::new("CD_ATIVO", vec!["ESCALA"; 10]);
        let dt_comptc = Series::new(
            "DT_COMPTC",
            vec![
                "2024-01", "2024-02", "2024-03", "2024-04", "2024-05", "2024-06", "2024-07",
                "2024-08", "2024-09", "2024-10",
            ],
        );
        // 100k, 200k, 400k...
        let qt_pos = Series::new(
            "QT_POS_FINAL",
            vec![
                1e5, 2e5, 4e5, 8e5, 1.6e6, 3.2e6, 6.4e6, 1.28e7, 2.56e7, 5.12e7,
            ],
        );
        let pct_pl = Series::new("VL_PORCENTAGEM_PL", vec![1.0; 10]);
        let vl_aquis = Series::new("VL_AQUIS_NEGOC", vec![1000.0; 10]);
        let vl_merc = Series::new("VL_MERC_POS_FINAL", vec![1000.0; 10]);
        let pl = Series::new("VL_PATRIM_LIQ", vec![100000.0; 10]);
        let cd_isin = Series::new("CD_ISIN", vec![""; 10]);

        let df = DataFrame::new(vec![
            cd_ativo, cd_isin, dt_comptc, qt_pos, pct_pl, vl_aquis, vl_merc, pl,
        ])
        .unwrap();

        let q_date = Series::new(
            "date",
            vec![
                "2024-01-01",
                "2024-02-01",
                "2024-03-01",
                "2024-04-01",
                "2024-05-01",
                "2024-06-01",
                "2024-07-01",
                "2024-08-01",
                "2024-09-01",
                "2024-10-01",
                "2024-11-01",
            ],
        );
        let q_close = Series::new("adjclose", vec![10.0; 11]);
        let quotes_df = DataFrame::new(vec![q_date, q_close]).unwrap();

        let analytics = compute_asset_analytics(&df, "ESCALA", Some(&quotes_df), None)
            .expect("Analytics failed");
        let inference = analytics
            .portfolio_inference
            .expect("Sem PortfolioInference!");

        assert_eq!(
            inference.bias_direction,
            TradeBias::Acumulando,
            "A invariância de escala não foi respeitada."
        );
    }

    #[test]
    fn test_choque_exogeno() {
        // Choque Exógeno: estabilidade -> salto súbito -> estabilidade. O estado deve retornar a Consistente após estabilizar.
        let cd_ativo = Series::new("CD_ATIVO", vec!["CHOQUE"; 12]);
        let dt_comptc = Series::new(
            "DT_COMPTC",
            vec![
                "2024-01", "2024-02", "2024-03", "2024-04", "2024-05", "2024-06", "2024-07",
                "2024-08", "2024-09", "2024-10", "2024-11", "2024-12",
            ],
        );
        let qt_pos = Series::new(
            "QT_POS_FINAL",
            vec![
                1000.0, 1000.0, 1000.0, 1000.0, 1000.0, 1000.0, 50000.0, 50000.0, 50000.0, 50000.0,
                50000.0, 50000.0,
            ],
        );
        let pct_pl = Series::new("VL_PORCENTAGEM_PL", vec![1.0; 12]);
        let vl_aquis = Series::new("VL_AQUIS_NEGOC", vec![0.0; 12]);
        let vl_merc = Series::new("VL_MERC_POS_FINAL", vec![1000.0; 12]);
        let pl = Series::new("VL_PATRIM_LIQ", vec![100000.0; 12]);
        let cd_isin = Series::new("CD_ISIN", vec![""; 12]);

        let df = DataFrame::new(vec![
            cd_ativo, cd_isin, dt_comptc, qt_pos, pct_pl, vl_aquis, vl_merc, pl,
        ])
        .unwrap();

        let q_date = Series::new(
            "date",
            vec![
                "2024-01-01",
                "2024-02-01",
                "2024-03-01",
                "2024-04-01",
                "2024-05-01",
                "2024-06-01",
                "2024-07-01",
                "2024-08-01",
                "2024-09-01",
                "2024-10-01",
                "2024-11-01",
                "2024-12-01",
                "2025-01-01",
            ],
        );
        let q_close = Series::new("adjclose", vec![10.0; 13]);
        let quotes_df = DataFrame::new(vec![q_date, q_close]).unwrap();

        let analytics = compute_asset_analytics(&df, "CHOQUE", Some(&quotes_df), None)
            .expect("Analytics failed");
        let inference = analytics
            .portfolio_inference
            .expect("Sem PortfolioInference!");

        assert_eq!(
            inference.bias_direction,
            TradeBias::Consistente,
            "Filtro não estabilizou após o choque exógeno prolongado."
        );
    }

    #[test]
    fn test_integridade_canal_dual() {
        // Passando NaN no PL para desativar o canal dual, o filtro deve sobreviver usando só o canal histórico primário.
        let cd_ativo = Series::new("CD_ATIVO", vec!["CANAL"; 5]);
        let dt_comptc = Series::new(
            "DT_COMPTC",
            vec!["2024-01", "2024-02", "2024-03", "2024-04", "2024-05"],
        );
        let qt_pos = Series::new("QT_POS_FINAL", vec![100.0, 200.0, 300.0, 400.0, 500.0]);
        let pct_pl = Series::new("VL_PORCENTAGEM_PL", vec![1.0; 5]);
        let vl_aquis = Series::new("VL_AQUIS_NEGOC", vec![0.0; 5]);
        let vl_merc = Series::new("VL_MERC_POS_FINAL", vec![1000.0; 5]);
        let pl = Series::new("VL_PATRIM_LIQ", vec![f64::NAN; 5]); // NaN intencional
        let cd_isin = Series::new("CD_ISIN", vec![""; 5]);

        let df = DataFrame::new(vec![
            cd_ativo, cd_isin, dt_comptc, qt_pos, pct_pl, vl_aquis, vl_merc, pl,
        ])
        .unwrap();

        let q_date = Series::new(
            "date",
            vec![
                "2024-01-01",
                "2024-02-01",
                "2024-03-01",
                "2024-04-01",
                "2024-05-01",
                "2024-06-01",
            ],
        );
        let q_close = Series::new("adjclose", vec![10.0; 6]);
        let quotes_df = DataFrame::new(vec![q_date, q_close]).unwrap();

        let analytics = compute_asset_analytics(&df, "CANAL", Some(&quotes_df), None)
            .expect("Analytics failed");
        let _inference = analytics
            .portfolio_inference
            .expect("Filtro falhou em fallback dimensional (NaN).");
    }
}
