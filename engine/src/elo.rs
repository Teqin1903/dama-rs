//! Elo ratings from the games of many matches: one Bradley-Terry model fitted by maximum
//! likelihood, and confidence intervals by resampling the colour-swapped pairs of each match.
//!
//! The model: a player rated `r` scores `expected_score(r - s)` against one rated `s`, a draw counts
//! as half a win. With θ = r · ln 10 / 400 the log-likelihood of a match of n games in which A
//! scored w is w · ln p + (n − w) · ln(1 − p), p = 1 / (1 + e^(θ_B − θ_A)). It is concave in θ, so
//! Newton's method with a line search finds its maximum. Ratings are only defined up to a constant:
//! the player `zero` is fixed at 0.
//!
//! The maximum exists only if the players cannot be split into two groups with one group scoring
//! nothing against the other (Ford's condition: the graph "i scored against j" is strongly
//! connected). A player outside the strongly connected group of `zero` would drift to ±∞; `fit`
//! reports such players instead of iterating without end.

use crate::rng::Mulberry32;
use crate::sprt::elo_of;

/// Elo per unit of θ: 400 / ln 10.
const ELO_PER_THETA: f64 = 400.0 / std::f64::consts::LN_10;

/// The result of a match between players `a` and `b`: `games` games, of which `a` scored `score`
/// points (a draw is ½).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Score {
    pub a: usize,
    pub b: usize,
    pub games: f64,
    pub score: f64,
}

/// The ratings of a fit.
#[derive(Clone, Debug, PartialEq)]
pub struct Fit {
    /// The Elo of every player, `zero` at 0. A player in `diverged` has +∞ (the others never
    /// scored against it), −∞ (it never scored against them) or NaN (neither can be said).
    pub elo: Vec<f64>,
    /// The players whose maximum-likelihood rating does not exist, in order. A player without
    /// games is not one of them: its Elo is NaN, the model says nothing about it.
    pub diverged: Vec<usize>,
}

/// Fits the ratings of players `0..players` to `scores`, with `zero` at 0. Matches of a player
/// against itself are left out: they carry no information about the ratings.
pub fn fit(players: usize, scores: &[Score], zero: usize) -> Fit {
    assert!(zero < players, "the player at 0 must be one of the players");
    let scores: Vec<Score> = scores
        .iter()
        .copied()
        .filter(|s| s.a != s.b && s.games > 0.0)
        .collect();
    // `beats[i][j]`: i scored something against j.
    let mut beats = vec![vec![false; players]; players];
    let mut played = vec![false; players];
    played[zero] = true;
    for s in &scores {
        played[s.a] = true;
        played[s.b] = true;
        beats[s.a][s.b] |= s.score > 0.0;
        beats[s.b][s.a] |= s.score < s.games;
    }
    let from_zero = reachable(&beats, zero, false);
    let to_zero = reachable(&beats, zero, true);
    let group: Vec<usize> = (0..players)
        .filter(|&i| from_zero[i] && to_zero[i])
        .collect();
    let mut elo = vec![f64::NAN; players];
    let mut diverged = Vec::new();
    for i in 0..players {
        if (from_zero[i] && to_zero[i]) || !played[i] {
            continue;
        }
        diverged.push(i);
        elo[i] = match (from_zero[i], to_zero[i]) {
            // Zero's group scored against i, i never scored back.
            (true, false) => f64::NEG_INFINITY,
            (false, true) => f64::INFINITY,
            _ => f64::NAN,
        };
    }
    // The fit within the group, whose games are among its members only.
    let index: Vec<Option<usize>> = (0..players)
        .map(|i| group.iter().position(|&g| g == i))
        .collect();
    let inner: Vec<Score> = scores
        .iter()
        .filter_map(|s| {
            Some(Score {
                a: index[s.a]?,
                b: index[s.b]?,
                ..*s
            })
        })
        .collect();
    let theta = newton(
        group.len(),
        &inner,
        index[zero].expect("zero is in its own group"),
    );
    for (k, &i) in group.iter().enumerate() {
        elo[i] = theta[k] * ELO_PER_THETA;
    }
    Fit { elo, diverged }
}

/// The players reachable from `start` along `beats` (backwards if `reverse`).
fn reachable(beats: &[Vec<bool>], start: usize, reverse: bool) -> Vec<bool> {
    let n = beats.len();
    let mut seen = vec![false; n];
    let mut stack = vec![start];
    seen[start] = true;
    while let Some(i) = stack.pop() {
        for j in 0..n {
            let edge = if reverse { beats[j][i] } else { beats[i][j] };
            if edge && !seen[j] {
                seen[j] = true;
                stack.push(j);
            }
        }
    }
    seen
}

/// The log-likelihood of `scores` at `theta`.
fn log_likelihood(scores: &[Score], theta: &[f64]) -> f64 {
    scores
        .iter()
        .map(|s| {
            let d = theta[s.a] - theta[s.b];
            // ln p = −ln(1 + e^−d) and ln(1 − p) = −ln(1 + e^d), computed without overflow.
            let ln_p = -softplus(-d);
            let ln_q = -softplus(d);
            s.score * ln_p + (s.games - s.score) * ln_q
        })
        .sum()
}

/// ln(1 + e^x) without overflow.
fn softplus(x: f64) -> f64 {
    if x > 0.0 {
        x + (-x).exp().ln_1p()
    } else {
        x.exp().ln_1p()
    }
}

/// The θ that maximises the likelihood, θ[zero] = 0; the players must form one strongly connected
/// group, so the maximum exists.
fn newton(players: usize, scores: &[Score], zero: usize) -> Vec<f64> {
    let mut theta: Vec<f64> = vec![0.0; players];
    // The unknowns: every player but `zero`.
    let free: Vec<usize> = (0..players).filter(|&i| i != zero).collect();
    for _ in 0..200 {
        let mut gradient = vec![0.0; players];
        let mut hessian = vec![vec![0.0; players]; players];
        for s in scores {
            let p = 1.0 / (1.0 + (theta[s.b] - theta[s.a]).exp());
            let g = s.score - s.games * p;
            let h = s.games * p * (1.0 - p);
            gradient[s.a] += g;
            gradient[s.b] -= g;
            hessian[s.a][s.a] -= h;
            hessian[s.b][s.b] -= h;
            hessian[s.a][s.b] += h;
            hessian[s.b][s.a] += h;
        }
        // Solve H · step = −g over the free players.
        let matrix: Vec<Vec<f64>> = free
            .iter()
            .map(|&i| free.iter().map(|&j| hessian[i][j]).collect())
            .collect();
        let rhs: Vec<f64> = free.iter().map(|&i| -gradient[i]).collect();
        let Some(step) = solve(matrix, rhs) else {
            break;
        };
        // Halve the step until the likelihood does not fall.
        let before = log_likelihood(scores, &theta);
        let mut length: f64 = 1.0;
        let mut next = theta.clone();
        loop {
            for (k, &i) in free.iter().enumerate() {
                next[i] = theta[i] + length * step[k];
            }
            if log_likelihood(scores, &next) >= before || length < 1e-6 {
                break;
            }
            length /= 2.0;
        }
        let largest = step.iter().fold(0.0f64, |m, x| m.max((length * x).abs()));
        theta = next;
        if largest < 1e-13 {
            break;
        }
    }
    theta
}

/// Solves `a · x = b` by Gaussian elimination with partial pivoting, or `None` if `a` is singular.
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        let pivot = (col..n).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))?;
        if a[pivot][col].abs() < 1e-300 {
            return None;
        }
        a.swap(col, pivot);
        b.swap(col, pivot);
        for row in col + 1..n {
            let factor = a[row][col] / a[col][col];
            let pivot_row = a[col].clone();
            for (x, p) in a[row].iter_mut().zip(&pivot_row).skip(col) {
                *x -= factor * p;
            }
            b[row] -= factor * b[col];
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let sum: f64 = (row + 1..n).map(|k| a[row][k] * x[k]).sum();
        x[row] = (b[row] - sum) / a[row][row];
    }
    Some(x)
}

/// A match for the bootstrap: players `a` and `b`, and A's points in each colour-swapped pair of
/// games (0, ½, 1, 1½ or 2).
#[derive(Clone, Debug, PartialEq)]
pub struct PairedMatch {
    pub a: usize,
    pub b: usize,
    pub pairs: Vec<f64>,
}

impl PairedMatch {
    fn score(&self, pairs: impl Iterator<Item = f64>) -> Score {
        Score {
            a: self.a,
            b: self.b,
            games: 2.0 * self.pairs.len() as f64,
            score: pairs.sum(),
        }
    }
}

/// The outcome of the bootstrap.
#[derive(Clone, Debug, PartialEq)]
pub struct Bootstrap {
    pub samples: usize,
    pub seed: u32,
    /// The 2.5% and 97.5% percentiles of each player's Elo (relative to `zero`); a bound may be
    /// infinite when some samples diverged.
    pub players: Vec<[f64; 2]>,
    /// The same for the Elo of each match's own score (`elo_of`, ±∞ at 0% and 100%).
    pub matches: Vec<[f64; 2]>,
    /// The samples whose fit had a diverged player.
    pub diverged: usize,
}

/// Resamples, within each match, its pairs of games with replacement, `samples` times from
/// mulberry32 seeded with `seed`, and refits the ratings each time.
pub fn bootstrap(
    players: usize,
    matches: &[PairedMatch],
    zero: usize,
    samples: usize,
    seed: u32,
) -> Bootstrap {
    let mut rng = Mulberry32::new(seed);
    let mut elos: Vec<Vec<f64>> = vec![Vec::with_capacity(samples); players];
    let mut match_elos: Vec<Vec<f64>> = vec![Vec::with_capacity(samples); matches.len()];
    let mut diverged = 0;
    for _ in 0..samples {
        let scores: Vec<Score> = matches
            .iter()
            .map(|m| {
                let n = m.pairs.len();
                let drawn: Vec<f64> = (0..n).map(|_| m.pairs[rng.next_index(n)]).collect();
                m.score(drawn.into_iter())
            })
            .collect();
        for (k, s) in scores.iter().enumerate() {
            match_elos[k].push(elo_or_infinite(s.score / s.games));
        }
        let fitted = fit(players, &scores, zero);
        if !fitted.diverged.is_empty() {
            diverged += 1;
        }
        for (i, elo) in fitted.elo.into_iter().enumerate() {
            elos[i].push(elo);
        }
    }
    Bootstrap {
        samples,
        seed,
        players: elos.iter().map(|v| interval(v)).collect(),
        matches: match_elos.iter().map(|v| interval(v)).collect(),
        diverged,
    }
}

/// `elo_of(score)`, or ±∞ at a score of 0 or 1.
pub fn elo_or_infinite(score: f64) -> f64 {
    if score <= 0.0 {
        f64::NEG_INFINITY
    } else if score >= 1.0 {
        f64::INFINITY
    } else {
        elo_of(score)
    }
}

/// The 2.5% and 97.5% percentiles of `values` (NaN left out), interpolated between order
/// statistics as R's default (type 7) does.
fn interval(values: &[f64]) -> [f64; 2] {
    let mut sorted: Vec<f64> = values.iter().copied().filter(|x| !x.is_nan()).collect();
    sorted.sort_by(f64::total_cmp);
    [percentile(&sorted, 0.025), percentile(&sorted, 0.975)]
}

/// The `q` quantile of sorted `values`, NaN without values.
pub fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.is_empty() {
        return f64::NAN;
    }
    let h = (sorted.len() - 1) as f64 * q;
    let (lo, hi) = (h.floor() as usize, h.ceil() as usize);
    if lo == hi || sorted[lo] == sorted[hi] {
        sorted[lo]
    } else if !sorted[lo].is_finite() || !sorted[hi].is_finite() {
        // No interpolation with an infinity: the nearer order statistic.
        sorted[h.round() as usize]
    } else {
        sorted[lo] + (h - lo as f64) * (sorted[hi] - sorted[lo])
    }
}
