//! The Elo fit and its bootstrap (engine/src/elo.rs) on matches whose ratings are known.

use engine::{PairedMatch, Score, bootstrap, expected_score, fit, percentile};

fn score(a: usize, b: usize, games: f64, score: f64) -> Score {
    Score { a, b, games, score }
}

#[test]
fn a_score_of_75_percent_is_190_8_elo() {
    let fitted = fit(2, &[score(0, 1, 400.0, 300.0)], 1);
    // 400 · log10(0.75 / 0.25) = 400 · log10 3.
    assert!((fitted.elo[0] - 400.0 * 3f64.log10()).abs() < 1e-9);
    assert!((fitted.elo[0] - 190.8).abs() < 0.05);
    assert_eq!(fitted.elo[1], 0.0);
    assert!(fitted.diverged.is_empty());
}

#[test]
fn a_consistent_chain_gives_back_its_differences() {
    // A is 100 above B, B is 150 above C, so A is 250 above C: every match scores what the model
    // expects, and the maximum of the likelihood is exactly there.
    let (ab, bc, ac) = (100.0, 150.0, 250.0);
    let games = 1000.0;
    let scores = [
        score(0, 1, games, games * expected_score(ab)),
        score(1, 2, games, games * expected_score(bc)),
        score(0, 2, games, games * expected_score(ac)),
    ];
    let fitted = fit(3, &scores, 2);
    for (got, want) in fitted.elo.iter().zip([250.0, 150.0, 0.0]) {
        assert!((got - want).abs() < 1e-6, "{got} instead of {want}");
    }
}

#[test]
fn the_zero_stays_where_it_is_put() {
    let scores = [
        score(0, 1, 200.0, 130.0),
        score(1, 2, 200.0, 90.0),
        score(0, 2, 200.0, 120.0),
    ];
    let at = |zero| fit(3, &scores, zero).elo;
    for zero in 0..3 {
        assert_eq!(at(zero)[zero], 0.0);
    }
    // Moving the zero shifts every rating by the same amount.
    let (z0, z2) = (at(0), at(2));
    for i in 0..3 {
        assert!(((z0[i] - z2[i]) - (z0[2] - z2[2])).abs() < 1e-9);
    }
}

#[test]
fn self_play_is_left_out_of_the_fit() {
    let without = fit(2, &[score(0, 1, 100.0, 60.0)], 1);
    let with = fit(2, &[score(0, 1, 100.0, 60.0), score(1, 1, 200.0, 37.0)], 1);
    assert_eq!(with, without);
}

#[test]
fn a_player_that_wins_everything_diverges() {
    // Player 2 scores every point against 0; 0 and 1 are an ordinary match.
    let scores = [score(0, 1, 100.0, 55.0), score(2, 0, 50.0, 50.0)];
    let fitted = fit(3, &scores, 1);
    assert_eq!(fitted.diverged, vec![2]);
    assert_eq!(fitted.elo[2], f64::INFINITY);
    assert!(fitted.elo[0].is_finite());
    // And one that loses everything goes the other way.
    let scores = [score(0, 1, 100.0, 55.0), score(2, 0, 50.0, 0.0)];
    let fitted = fit(3, &scores, 1);
    assert_eq!(fitted.diverged, vec![2]);
    assert_eq!(fitted.elo[2], f64::NEG_INFINITY);
}

#[test]
fn a_player_without_games_is_not_diverged() {
    let fitted = fit(3, &[score(0, 1, 100.0, 60.0)], 1);
    assert!(fitted.diverged.is_empty());
    assert!(fitted.elo[2].is_nan());
}

fn matches() -> Vec<PairedMatch> {
    // Pair scores 0 to 2 in a fixed pattern.
    let pairs = |n: usize, k: usize| (0..n).map(|i| ((i * k) % 5) as f64 / 2.0).collect();
    vec![
        PairedMatch {
            a: 0,
            b: 1,
            pairs: pairs(100, 3),
        },
        PairedMatch {
            a: 1,
            b: 2,
            pairs: pairs(100, 7),
        },
        PairedMatch {
            a: 0,
            b: 2,
            pairs: pairs(50, 11),
        },
    ]
}

#[test]
fn the_same_seed_gives_the_same_bootstrap() {
    let one = bootstrap(3, &matches(), 1, 200, 7);
    assert_eq!(one, bootstrap(3, &matches(), 1, 200, 7));
    assert_ne!(one.players, bootstrap(3, &matches(), 1, 200, 8).players);
    assert_eq!(one.players[1], [0.0, 0.0]);
    assert_eq!(one.diverged, 0);
    // The interval of a fit holds the fit itself.
    let scores: Vec<Score> = matches()
        .iter()
        .map(|m| score(m.a, m.b, 2.0 * m.pairs.len() as f64, m.pairs.iter().sum()))
        .collect();
    let fitted = fit(3, &scores, 1);
    for i in [0, 2] {
        let [lo, hi] = one.players[i];
        assert!(
            lo < fitted.elo[i] && fitted.elo[i] < hi,
            "{lo} {} {hi}",
            fitted.elo[i]
        );
    }
}

#[test]
fn percentiles_interpolate_and_keep_infinities() {
    let values = [1.0, 2.0, 3.0, 4.0, 5.0];
    assert_eq!(percentile(&values, 0.0), 1.0);
    assert_eq!(percentile(&values, 0.5), 3.0);
    assert_eq!(percentile(&values, 0.125), 1.5);
    let values = [1.0, 2.0, f64::INFINITY];
    assert_eq!(percentile(&values, 0.975), f64::INFINITY);
    assert!(percentile(&[], 0.5).is_nan());
}
