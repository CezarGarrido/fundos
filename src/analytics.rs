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

// ── Regressão Linear (OLS) ─────────────────────────────────────────────

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

    // X = [1, t]  —  intercepto + tendência linear (t = 0..n-1)
    let t: Vec<f64> = (0..n).map(|i| i as f64).collect();
    let y: Vec<f64> = historical_qtys.to_vec();

    // OLS: β = (XᵀX)⁻¹ Xᵀy
    let sum_t: f64 = t.iter().sum();
    let sum_t2: f64 = t.iter().map(|ti| ti * ti).sum();
    let sum_y: f64 = y.iter().sum();
    let sum_ty: f64 = t.iter().zip(y.iter()).map(|(ti, yi)| ti * yi).sum();

    let denom = n as f64 * sum_t2 - sum_t * sum_t;
    if denom.abs() < 1e-10 {
        return extrapolate_baseline(quotes, last_known_dt, last_qty);
    }

    let intercept = (sum_t2 * sum_y - sum_t * sum_ty) / denom;
    let slope = (n as f64 * sum_ty - sum_t * sum_y) / denom;

    // R² e MAE
    let y_mean = sum_y / n as f64;
    let ss_res: f64 = t
        .iter()
        .zip(y.iter())
        .map(|(ti, yi)| {
            let pred = intercept + slope * ti;
            (yi - pred).powi(2)
        })
        .sum();
    let ss_tot: f64 = y.iter().map(|yi| (yi - y_mean).powi(2)).sum();
    let r_squared = if ss_tot > 0.0 {
        1.0 - ss_res / ss_tot
    } else {
        0.0
    };

    let mae: f64 = t
        .iter()
        .zip(y.iter())
        .map(|(ti, yi)| (yi - (intercept + slope * ti)).abs())
        .sum::<f64>()
        / n as f64;

    debug!(
        "Regressão Linear: intercept={:.2}, slope={:.4}, R²={:.4}, MAE={:.2}",
        intercept, slope, r_squared, mae
    );

    // Projetar para os meses ocultos
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

#[allow(clippy::needless_range_loop)]
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

    // Estado: x = [posição, velocidade]
    // Modelo de velocidade constante:
    //   F = [[1, 1], [0, 1]]
    //   H = [1, 0]  (observamos apenas posição)

    // Ruídos
    let q_pos = 0.01; // variância do processo (posição)
    let q_vel = 0.001; // variância do processo (velocidade)
    let r_obs = 0.1; // variância da observação

    // Estado inicial
    let mut x = [historical_qtys[0], 0.0_f64];
    let mut p = [[1.0, 0.0], [0.0, 1.0]]; // covariância inicial

    let f = [[1.0, 1.0], [0.0, 1.0]];
    let h = [1.0, 0.0];
    let proc_noise = [[q_pos, 0.0], [0.0, q_vel]];

    let mut predictions: Vec<f64> = vec![x[0]];
    let mut residuals: Vec<f64> = Vec::new();

    for i in 1..n {
        // Predict
        let x_pred = [
            f[0][0] * x[0] + f[0][1] * x[1],
            f[1][0] * x[0] + f[1][1] * x[1],
        ];
        let p_pred_00 = f[0][0] * (f[0][0] * p[0][0] + f[0][1] * p[1][0])
            + f[0][1] * (f[0][0] * p[0][1] + f[0][1] * p[1][1])
            + proc_noise[0][0];
        let p_pred_01 = f[0][0] * (f[1][0] * p[0][0] + f[1][1] * p[1][0])
            + f[0][1] * (f[1][0] * p[0][1] + f[1][1] * p[1][1])
            + proc_noise[0][1];
        let p_pred_10 = f[1][0] * (f[0][0] * p[0][0] + f[0][1] * p[1][0])
            + f[1][1] * (f[0][0] * p[0][1] + f[0][1] * p[1][1])
            + proc_noise[1][0];
        let p_pred_11 = f[1][0] * (f[1][0] * p[0][0] + f[1][1] * p[1][0])
            + f[1][1] * (f[1][0] * p[0][1] + f[1][1] * p[1][1])
            + proc_noise[1][1];

        // Update
        let y_obs = historical_qtys[i];
        let y_pred = h[0] * x_pred[0] + h[1] * x_pred[1];
        let residual = y_obs - y_pred;
        residuals.push(residual);

        let s = h[0] * (h[0] * p_pred_00 + h[1] * p_pred_10)
            + h[1] * (h[0] * p_pred_01 + h[1] * p_pred_11)
            + r_obs;
        let k0 = (p_pred_00 * h[0] + p_pred_01 * h[1]) / s;
        let k1 = (p_pred_10 * h[0] + p_pred_11 * h[1]) / s;

        x = [x_pred[0] + k0 * residual, x_pred[1] + k1 * residual];
        p = [
            [
                (1.0 - k0 * h[0]) * p_pred_00 - k0 * h[1] * p_pred_10,
                (1.0 - k0 * h[0]) * p_pred_01 - k0 * h[1] * p_pred_11,
            ],
            [
                -k1 * h[0] * p_pred_00 + (1.0 - k1 * h[1]) * p_pred_10,
                -k1 * h[0] * p_pred_01 + (1.0 - k1 * h[1]) * p_pred_11,
            ],
        ];

        predictions.push(x[0]);
    }

    // R² e MAE nos dados históricos
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

    // Intervalo de confiança 95% (baseado na covariância final)
    let std_dev = p[0][0].sqrt();
    let ci_lower = (x[0] - 1.96 * std_dev).max(0.0);
    let ci_upper = x[0] + 1.96 * std_dev;

    debug!(
        "Kalman: estado_final=[pos={:.2}, vel={:.4}], IC95=[{:.0}, {:.0}], R²={:.4}, MAE={:.2}",
        x[0], x[1], ci_lower, ci_upper, r_squared, mae
    );

    // Projetar para frente usando o modelo de velocidade constante
    let mut estimates = Vec::new();
    let mut future_x = x;
    let mut future_p = p;
    for q in quotes {
        if q.date.as_str() > last_known_dt {
            // Predict one step
            future_x = [
                f[0][0] * future_x[0] + f[0][1] * future_x[1],
                f[1][0] * future_x[0] + f[1][1] * future_x[1],
            ];
            future_p = [
                [
                    f[0][0] * (f[0][0] * future_p[0][0] + f[0][1] * future_p[1][0])
                        + f[0][1] * (f[0][0] * future_p[0][1] + f[0][1] * future_p[1][1])
                        + proc_noise[0][0],
                    f[0][0] * (f[1][0] * future_p[0][0] + f[1][1] * future_p[1][0])
                        + f[0][1] * (f[1][0] * future_p[0][1] + f[1][1] * future_p[1][1])
                        + proc_noise[0][1],
                ],
                [
                    f[1][0] * (f[0][0] * future_p[0][0] + f[0][1] * future_p[1][0])
                        + f[1][1] * (f[0][0] * future_p[0][1] + f[0][1] * future_p[1][1])
                        + proc_noise[1][0],
                    f[1][0] * (f[1][0] * future_p[0][0] + f[1][1] * future_p[1][0])
                        + f[1][1] * (f[1][0] * future_p[0][1] + f[1][1] * future_p[1][1])
                        + proc_noise[1][1],
                ],
            ];
            let pred_qty = future_x[0].max(0.0);
            estimates.push((q.date.clone(), pred_qty, pred_qty * q.close_price));
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

// ── Random Forest (implementação simplificada) ──────────────────────────

fn extrapolate_random_forest(
    historical_qtys: &[f64],
    last_qty: f64,
    quotes: &[crate::provider::yahoo::MonthlyQuote],
    last_known_dt: &str,
) -> ExtrapolationResult {
    let n = historical_qtys.len();
    if n < 6 {
        // Poucos dados, fallback para regressão linear
        return extrapolate_linear(historical_qtys, last_qty, quotes, last_known_dt);
    }

    // Features: [t, y_lag1] — tempo + qtd do mês anterior
    let mut features: Vec<[f64; 2]> = Vec::with_capacity(n - 1);
    let mut targets: Vec<f64> = Vec::with_capacity(n - 1);
    for i in 1..n {
        features.push([i as f64, historical_qtys[i - 1]]);
        targets.push(historical_qtys[i]);
    }
    let m = features.len();

    // Hiperparâmetros
    let n_trees = 50_usize;
    let max_depth = 5_usize;

    // Treinar ensemble
    let forest = train_random_forest(&features, &targets, n_trees, max_depth);

    // R² e MAE nos dados de treino
    let train_preds: Vec<f64> = features
        .iter()
        .map(|f| forest_predict(&forest, *f))
        .collect();
    let y_mean: f64 = targets.iter().sum::<f64>() / m as f64;
    let ss_res: f64 = targets
        .iter()
        .zip(train_preds.iter())
        .map(|(y, p)| (y - p).powi(2))
        .sum();
    let ss_tot: f64 = targets.iter().map(|y| (y - y_mean).powi(2)).sum();
    let r_squared = if ss_tot > 0.0 {
        1.0 - ss_res / ss_tot
    } else {
        0.0
    };
    let mae: f64 = targets
        .iter()
        .zip(train_preds.iter())
        .map(|(y, p)| (y - p).abs())
        .sum::<f64>()
        / m as f64;

    debug!(
        "Random Forest: {} árvores (depth={}), {} amostras, R²={:.4}, MAE={:.2}",
        n_trees, max_depth, m, r_squared, mae
    );

    // Projeção recursiva: cada mês futuro usa a predição do mês anterior como lag
    let mut estimates = Vec::new();
    let mut prev_qty = historical_qtys[n - 1];
    for (j, q) in quotes.iter().enumerate() {
        if q.date.as_str() > last_known_dt {
            let feat = [(n + j) as f64, prev_qty];
            let pred_qty = forest_predict(&forest, feat).max(0.0);
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

// ── Random Forest internals ─────────────────────────────────────────────

struct TreeNode {
    split_feature: usize,
    split_value: f64,
    left: Option<Box<TreeNode>>,
    right: Option<Box<TreeNode>>,
    leaf_value: f64,
}

struct DecisionTree {
    root: TreeNode,
}

fn train_random_forest(
    features: &[[f64; 2]],
    targets: &[f64],
    n_trees: usize,
    max_depth: usize,
) -> Vec<DecisionTree> {
    let m = features.len();
    let mut rng = SimpleRng::new(42);
    let mut trees = Vec::with_capacity(n_trees);

    for _ in 0..n_trees {
        // Bootstrap sample
        let mut boot_features = Vec::with_capacity(m);
        let mut boot_targets = Vec::with_capacity(m);
        for _ in 0..m {
            let idx = rng.next() as usize % m;
            boot_features.push(features[idx]);
            boot_targets.push(targets[idx]);
        }
        let tree = build_tree(&boot_features, &boot_targets, max_depth, &mut rng);
        trees.push(tree);
    }
    trees
}

fn forest_predict(forest: &[DecisionTree], features: [f64; 2]) -> f64 {
    let sum: f64 = forest.iter().map(|t| tree_predict(&t.root, features)).sum();
    sum / forest.len() as f64
}

fn tree_predict(node: &TreeNode, features: [f64; 2]) -> f64 {
    if node.left.is_none() && node.right.is_none() {
        return node.leaf_value;
    }
    if features[node.split_feature] <= node.split_value {
        node.left
            .as_ref()
            .map_or(node.leaf_value, |l| tree_predict(l, features))
    } else {
        node.right
            .as_ref()
            .map_or(node.leaf_value, |r| tree_predict(r, features))
    }
}

fn build_tree(
    features: &[[f64; 2]],
    targets: &[f64],
    depth: usize,
    rng: &mut SimpleRng,
) -> DecisionTree {
    let n = targets.len();
    if n == 0 {
        return DecisionTree {
            root: TreeNode {
                split_feature: 0,
                split_value: 0.0,
                left: None,
                right: None,
                leaf_value: 0.0,
            },
        };
    }

    let mean = targets.iter().sum::<f64>() / n as f64;

    if depth == 0 || n <= 2 {
        return DecisionTree {
            root: TreeNode {
                split_feature: 0,
                split_value: 0.0,
                left: None,
                right: None,
                leaf_value: mean,
            },
        };
    }

    // Encontrar o melhor split (MSE)
    let n_features = 2;
    // Feature bagging: usar feature aleatória
    let split_feat = (rng.next() as usize) % n_features;

    let mut best_split = 0.0_f64;
    let mut best_mse = f64::MAX;

    let mut values: Vec<f64> = features.iter().map(|f| f[split_feat]).collect();
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());

    for i in 1..values.len() {
        let split_val = (values[i - 1] + values[i]) / 2.0;
        let mut left_sum = 0.0;
        let mut left_count = 0;
        let mut right_sum = 0.0;
        let mut right_count = 0;
        for (j, f) in features.iter().enumerate() {
            if f[split_feat] <= split_val {
                left_sum += targets[j];
                left_count += 1;
            } else {
                right_sum += targets[j];
                right_count += 1;
            }
        }
        if left_count == 0 || right_count == 0 {
            continue;
        }
        let left_mean = left_sum / left_count as f64;
        let right_mean = right_sum / right_count as f64;
        let mse: f64 = features
            .iter()
            .enumerate()
            .map(|(j, f)| {
                let pred = if f[split_feat] <= split_val {
                    left_mean
                } else {
                    right_mean
                };
                (targets[j] - pred).powi(2)
            })
            .sum();
        if mse < best_mse {
            best_mse = mse;
            best_split = split_val;
        }
    }

    if best_mse == f64::MAX {
        return DecisionTree {
            root: TreeNode {
                split_feature: 0,
                split_value: 0.0,
                left: None,
                right: None,
                leaf_value: mean,
            },
        };
    }

    let mut left_feat = Vec::new();
    let mut left_targ = Vec::new();
    let mut right_feat = Vec::new();
    let mut right_targ = Vec::new();
    for (j, f) in features.iter().enumerate() {
        if f[split_feat] <= best_split {
            left_feat.push(*f);
            left_targ.push(targets[j]);
        } else {
            right_feat.push(*f);
            right_targ.push(targets[j]);
        }
    }

    let left = build_tree(&left_feat, &left_targ, depth - 1, rng);
    let right = build_tree(&right_feat, &right_targ, depth - 1, rng);

    DecisionTree {
        root: TreeNode {
            split_feature: split_feat,
            split_value: best_split,
            left: Some(Box::new(left.root)),
            right: Some(Box::new(right.root)),
            leaf_value: mean,
        },
    }
}

// ── RNG linear simples para bootstrap ───────────────────────────────────

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.state >> 32
    }
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
