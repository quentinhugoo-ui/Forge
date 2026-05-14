//! Π.23 (Wave 13, 2026-05-02) — Walk-forward optimization parallel.
//!
//! **Origine** : Wealth-Lab, Backtrader, Robert Pardo "Walk-Forward
//! Analysis" (1992). Pattern canonique pour éviter overfitting :
//!
//!   1. Découper l'historique en fenêtres ordonnées.
//!   2. Pour chaque fenêtre k :
//!      - Optimiser les paramètres sur la fenêtre k (in-sample).
//!      - Tester les paramètres optimaux sur la fenêtre k+1 (out-of-sample).
//!   3. Aggregate les résultats out-of-sample → vraie performance.
//!
//! Une stratégie qui "marche" sur un seul backtest peut être pure
//! overfitting. Walk-forward = preuve qu'elle généralise sur des
//! données qu'elle n'a jamais vues.
//!
//! Le pattern est intrinsèquement parallèle : N fenêtres × M
//! combinaisons de paramètres = N×M backtests indépendants. Si un
//! bottleneck mesuré apparaît, paralléliser via un simple thread pool.
//!
//! ## Pourquoi pour Forge ?
//!
//! Wave 12 a livré le backtest end-to-end. Wave 13 ajoute la rigueur
//! statistique : prouver qu'une stratégie qu'on adopte n'est pas du
//! data dredging.
//!
//! ## Architecture Wave 13 minimal viable
//!
//! - `WalkForwardConfig { window_size, step, n_windows }`.
//! - `WalkForwardWindow { in_sample: Range, out_of_sample: Range }`.
//! - `WalkForwardResult { window, in_sample_score, out_of_sample_score }`.
//! - `walk_forward<P, F>(config, params, optimize_fn)` :
//!     * `optimize_fn(in_sample_range, params) -> (best_params, in_score)`
//!     * Test best_params sur out_of_sample → out_score.
//!     * Returns Vec<WalkForwardResult>.
//! - Single-thread Wave 13 minimal (parallélisation triviale si un
//!   bottleneck est mesuré).
//!
//! ## Limitations Wave 13 minimal
//!
//! - Single-thread (le pattern est trivialement parallélisable, on
//!   attend de mesurer un bottleneck réel)
//! - Pas de rolling vs anchored windows distinction (Wave 14+)
//! - Score = i64 (caller convertit Q3132 raw si besoin)

use std::ops::Range;

/// Config walk-forward.
#[allow(dead_code)] // Wave 13 — primitives exposées pour orchestration Wave 14+.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WalkForwardConfig {
    /// Taille de chaque fenêtre in-sample (en nombre de bars).
    pub window_size: usize,
    /// Pas entre fenêtres (en nombre de bars). step < window_size = overlap.
    pub step: usize,
    /// Nombre total de fenêtres à exécuter.
    pub n_windows: usize,
}

/// Une fenêtre walk-forward = (in_sample_range, out_of_sample_range).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkForwardWindow {
    pub in_sample: Range<usize>,
    pub out_of_sample: Range<usize>,
}

/// Résultat pour une fenêtre.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct WalkForwardResult<P: Clone> {
    pub window: WalkForwardWindow,
    pub best_params: P,
    pub in_sample_score: i64,
    pub out_of_sample_score: i64,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalkForwardError {
    InsufficientData { needed: usize, available: usize },
    InvalidConfig(&'static str),
}

impl std::fmt::Display for WalkForwardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalkForwardError::InsufficientData { needed, available } =>
                write!(f, "walk-forward: need {} bars, have {}", needed, available),
            WalkForwardError::InvalidConfig(s) =>
                write!(f, "walk-forward config invalid: {}", s),
        }
    }
}

/// Génère les fenêtres walk-forward selon la config.
#[allow(dead_code)]
/// Fenêtre k : in_sample = [k*step, k*step + window_size),
///            out_of_sample = [k*step + window_size, k*step + window_size + step).
pub fn windows(
    config: WalkForwardConfig,
    total_bars: usize,
) -> Result<Vec<WalkForwardWindow>, WalkForwardError> {
    if config.window_size == 0 {
        return Err(WalkForwardError::InvalidConfig("window_size must be > 0"));
    }
    if config.step == 0 {
        return Err(WalkForwardError::InvalidConfig("step must be > 0"));
    }
    if config.n_windows == 0 {
        return Ok(Vec::new());
    }
    let last_oos_end = (config.n_windows - 1) * config.step
        + config.window_size + config.step;
    if last_oos_end > total_bars {
        return Err(WalkForwardError::InsufficientData {
            needed: last_oos_end, available: total_bars,
        });
    }
    let mut out = Vec::with_capacity(config.n_windows);
    for k in 0..config.n_windows {
        let in_start = k * config.step;
        let in_end = in_start + config.window_size;
        let oos_end = in_end + config.step;
        out.push(WalkForwardWindow {
            in_sample: in_start..in_end,
            out_of_sample: in_end..oos_end,
        });
    }
    Ok(out)
}

/// Walk-forward orchestration. `optimize_fn` reçoit la range in-sample
#[allow(dead_code)]
/// et la liste des paramètres candidats, retourne `(best_param, in_score)`.
/// `test_fn` reçoit la range out-of-sample et le best_param, retourne
/// out_of_sample_score.
pub fn walk_forward<P, F, G>(
    config: WalkForwardConfig,
    params: &[P],
    total_bars: usize,
    mut optimize_fn: F,
    mut test_fn: G,
) -> Result<Vec<WalkForwardResult<P>>, WalkForwardError>
where
    P: Clone,
    F: FnMut(Range<usize>, &[P]) -> (P, i64),
    G: FnMut(Range<usize>, &P) -> i64,
{
    let win_list = windows(config, total_bars)?;
    let mut results = Vec::with_capacity(win_list.len());
    for win in win_list {
        let (best_params, in_score) = optimize_fn(win.in_sample.clone(), params);
        let oos_score = test_fn(win.out_of_sample.clone(), &best_params);
        results.push(WalkForwardResult {
            window: win,
            best_params,
            in_sample_score: in_score,
            out_of_sample_score: oos_score,
        });
    }
    Ok(results)
}

/// Aggregate moyenne des scores out-of-sample (mesure de
/// generalization). Si la moyenne out-of-sample est largement
/// inférieure à la moyenne in-sample, c'est un signal d'overfitting.
#[allow(dead_code)]
pub fn average_oos_score<P: Clone>(results: &[WalkForwardResult<P>]) -> i64 {
    if results.is_empty() {
        return 0;
    }
    let sum: i64 = results.iter().map(|r| r.out_of_sample_score).sum();
    sum / results.len() as i64
}

/// Idem pour in-sample. Comparer avec average_oos_score donne le
/// "walk-forward efficiency ratio" = avg_oos / avg_in.
#[allow(dead_code)]
pub fn average_in_sample_score<P: Clone>(results: &[WalkForwardResult<P>]) -> i64 {
    if results.is_empty() {
        return 0;
    }
    let sum: i64 = results.iter().map(|r| r.in_sample_score).sum();
    sum / results.len() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_generates_correct_ranges() {
        let config = WalkForwardConfig {
            window_size: 100, step: 50, n_windows: 3,
        };
        let wins = windows(config, 250).unwrap();
        assert_eq!(wins.len(), 3);
        assert_eq!(wins[0].in_sample, 0..100);
        assert_eq!(wins[0].out_of_sample, 100..150);
        assert_eq!(wins[1].in_sample, 50..150);
        assert_eq!(wins[1].out_of_sample, 150..200);
        assert_eq!(wins[2].in_sample, 100..200);
        assert_eq!(wins[2].out_of_sample, 200..250);
    }

    #[test]
    fn windows_insufficient_data_errors() {
        let config = WalkForwardConfig {
            window_size: 100, step: 50, n_windows: 5,
        };
        let err = windows(config, 100).unwrap_err();
        assert!(matches!(err, WalkForwardError::InsufficientData { .. }));
    }

    #[test]
    fn windows_zero_size_errors() {
        let config = WalkForwardConfig {
            window_size: 0, step: 50, n_windows: 1,
        };
        assert!(matches!(
            windows(config, 100),
            Err(WalkForwardError::InvalidConfig(_))
        ));
    }

    #[test]
    fn windows_zero_step_errors() {
        let config = WalkForwardConfig {
            window_size: 100, step: 0, n_windows: 1,
        };
        assert!(matches!(
            windows(config, 100),
            Err(WalkForwardError::InvalidConfig(_))
        ));
    }

    #[test]
    fn walk_forward_basic_run() {
        // Synthetic : data[i] = i. Optimize_fn pick le param qui matche
        // le mieux la moyenne (ici on pick toujours params[0] pour
        // simplicité). Test_fn returns mean of out_of_sample range.
        let config = WalkForwardConfig {
            window_size: 10, step: 5, n_windows: 3,
        };
        let params = vec![1, 2, 3];
        let results = walk_forward(
            config, &params, 25,
            |range, params| {
                // Pick le premier param, score = sum range.
                let score = range.sum::<usize>() as i64;
                (params[0], score)
            },
            |range, _param| range.sum::<usize>() as i64,
        ).unwrap();
        assert_eq!(results.len(), 3);
        // In-sample window 0 = 0..10 sum = 45.
        assert_eq!(results[0].in_sample_score, 45);
        // Out-of-sample window 0 = 10..15 sum = 60.
        assert_eq!(results[0].out_of_sample_score, 60);
    }

    #[test]
    fn average_oos_computes_mean() {
        let results = vec![
            WalkForwardResult {
                window: WalkForwardWindow { in_sample: 0..10, out_of_sample: 10..15 },
                best_params: 0i32,
                in_sample_score: 100,
                out_of_sample_score: 80,
            },
            WalkForwardResult {
                window: WalkForwardWindow { in_sample: 5..15, out_of_sample: 15..20 },
                best_params: 0i32,
                in_sample_score: 120,
                out_of_sample_score: 90,
            },
        ];
        assert_eq!(average_oos_score(&results), 85);
        assert_eq!(average_in_sample_score(&results), 110);
    }

    #[test]
    fn average_empty_returns_zero() {
        let results: Vec<WalkForwardResult<i32>> = Vec::new();
        assert_eq!(average_oos_score(&results), 0);
        assert_eq!(average_in_sample_score(&results), 0);
    }

    #[test]
    fn walk_forward_n_windows_zero_returns_empty() {
        let config = WalkForwardConfig {
            window_size: 10, step: 5, n_windows: 0,
        };
        let wins = windows(config, 100).unwrap();
        assert!(wins.is_empty());
    }

    #[test]
    fn walk_forward_overfitting_detection() {
        // Optimisation overfit : in-sample score artificiellement élevé,
        // out-of-sample score faible → ratio < 1 = overfitting.
        let config = WalkForwardConfig {
            window_size: 5, step: 5, n_windows: 2,
        };
        let params = vec![0i32];
        let results = walk_forward(
            config, &params, 15,
            // In-sample : score = 1000 (overfit).
            |_range, params| (params[0], 1000),
            // Out-of-sample : score = 100 (généralise mal).
            |_range, _param| 100,
        ).unwrap();
        let in_avg = average_in_sample_score(&results);
        let oos_avg = average_oos_score(&results);
        assert_eq!(in_avg, 1000);
        assert_eq!(oos_avg, 100);
        // Walk-forward efficiency = oos / in = 0.10 → strong overfitting signal.
        // (oos × 10 = 1000, equal to in_avg = strict overfitting boundary.)
        assert!(oos_avg * 10 <= in_avg);
    }

    #[test]
    fn walk_forward_no_overfitting_signal() {
        // In-sample et out-of-sample similaires → strategy generalize.
        let config = WalkForwardConfig {
            window_size: 5, step: 5, n_windows: 2,
        };
        let params = vec![42];
        let results = walk_forward(
            config, &params, 15,
            |_range, params| (params[0], 100),
            |_range, _param| 95,
        ).unwrap();
        let in_avg = average_in_sample_score(&results);
        let oos_avg = average_oos_score(&results);
        assert!(oos_avg * 100 / in_avg >= 90); // ≥ 90% efficiency
    }

    #[test]
    fn walk_forward_passes_best_param_to_test() {
        // Vérifie que le best_param renvoyé par optimize_fn est bien
        // utilisé dans test_fn.
        let config = WalkForwardConfig {
            window_size: 5, step: 5, n_windows: 1,
        };
        let params = vec![10, 20, 30];
        let results = walk_forward(
            config, &params, 10,
            |_range, params| (params[1], 50),  // optimize picks 20
            |_range, &param| (param * 2) as i64,  // test returns 2 × param
        ).unwrap();
        assert_eq!(results[0].best_params, 20);
        assert_eq!(results[0].out_of_sample_score, 40);
    }
}
