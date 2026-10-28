//! Texel tuning: weights of the evaluation fitted to game results.
//!
//! Every quiet position of a game is labelled with the game's result from White's view (1, ½ or
//! 0), and the weights are chosen to make sigmoid(K · value) predict it: they minimise the mean
//! squared error. The value is linear in the weights (U and the positional features of
//! engine/src/eval.rs are sums of weight × feature), so the features of each position are counted
//! once (`Sample`) and every error after that is a dot product per position.
//!
//! The steps: `fit_k` finds the K that fits the starting weights best; then, with K fixed and
//! `man` fixed at 1 (K and the weights could otherwise grow together), `tune` moves one weight at
//! a time by ± a step while the error falls, with smaller and smaller steps (the local search of
//! the original Texel method).

use std::collections::HashSet;

use crate::arena::GameRecord;
use crate::board::{Board, Color};
use crate::eval::{Features, Positional, Weights};

/// The number of weights, in the order of `Weights::KEYS`.
pub const N_WEIGHTS: usize = Weights::KEYS.len();

/// The steps of `tune`, from the largest. Each is a multiple of 0.0001, so weights that start on
/// that grid stay on it.
pub const STEPS: [f64; 10] = [
    0.1, 0.05, 0.02, 0.01, 0.005, 0.002, 0.001, 0.0005, 0.0002, 0.0001,
];

/// A position of a game: its features from White's view and the game's result for White.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    /// men, kings, mob, adv, back, cen, edge, qmob (the order of `Weights::KEYS`), White minus
    /// Black.
    pub features: [f64; N_WEIGHTS],
    /// 1 if White won, ½ for a draw, 0 if Black won.
    pub result: f64,
}

/// The weights as an array in the order of `Weights::KEYS`.
pub fn to_array(w: &Weights) -> [f64; N_WEIGHTS] {
    Weights::KEYS.map(|key| w.get(key).expect("a key of KEYS"))
}

/// The weights from an array in the order of `Weights::KEYS`.
pub fn from_array(a: &[f64; N_WEIGHTS]) -> Weights {
    let [man, king, mob, adv, back, cen, edge, qmob] = *a;
    Weights {
        man,
        king,
        mob,
        adv,
        back,
        cen,
        edge,
        qmob,
    }
}

/// The features of `board` from White's view, whoever is to move: the view of a search leaf,
/// which is scored for the root side whichever side is to move there. The board must not be
/// won: both sides need a legal move.
pub fn features_of(board: &Board) -> [f64; N_WEIGHTS] {
    let u = Features::of(board, Color::White);
    debug_assert!(!u.win, "features of a won position");
    let p = Positional::of(board, Color::White);
    [u.men, u.kings, u.mob, p.adv, p.back, p.cen, p.edge, p.qmob].map(f64::from)
}

/// The value of a position with these features: Σ weight × feature.
pub fn value(features: &[f64; N_WEIGHTS], w: &[f64; N_WEIGHTS]) -> f64 {
    features.iter().zip(w).map(|(f, w)| f * w).sum()
}

/// The positions of a game worth learning from, each once, with the side to move: from ply
/// `skip_plies` on (the random opening plies are not the agents' choice), only quiet ones (the
/// side to move has no capture; with one, the value would change before the position settles,
/// which the search's quiescence never scores either), and only while both sides have a legal
/// move.
pub fn quiet_positions(game: &GameRecord, skip_plies: usize) -> Vec<(Board, Color)> {
    let (mut board, mut side) = (Board::start(), Color::White);
    let mut seen = HashSet::new();
    let mut positions = Vec::new();
    // The position before every move, and the last one.
    for ply in 0..=game.moves.len() {
        if ply >= skip_plies && seen.insert((board, side)) {
            let ours = board.legal_moves(side);
            let quiet = !ours.is_empty() && !ours.iter().any(|mv| mv.is_capture());
            if quiet && !board.legal_moves(side.opponent()).is_empty() {
                positions.push((board, side));
            }
        }
        if let Some(mv) = game.moves.get(ply) {
            board = board.apply(mv);
            side = side.opponent();
        }
    }
    positions
}

/// The `quiet_positions` of a game as samples, labelled with the game's result for White.
pub fn samples_of(game: &GameRecord, skip_plies: usize) -> Vec<Sample> {
    let result = match game.winner {
        Some(Color::White) => 1.0,
        Some(Color::Black) => 0.0,
        None => 0.5,
    };
    quiet_positions(game, skip_plies)
        .iter()
        .map(|(board, _)| Sample {
            features: features_of(board),
            result,
        })
        .collect()
}

/// Whether game `game` of a paired match (counted from 0) belongs to the validation set: every
/// fifth pair. The two games of a pair share their opening, so they go to the same set.
pub fn is_validation(game: usize) -> bool {
    (game / 2) % 5 == 4
}

/// The samples of whole games, split into a training and a validation set game by game: every
/// game goes with all its positions to the set `is_validation` names for it, so no game is in
/// both. The same position can still come up in games of both sets; `shared` counts those.
#[derive(Debug, Default)]
pub struct DataSet {
    pub train: Vec<Sample>,
    pub validation: Vec<Sample>,
    /// The games in each set: training, validation.
    pub games: [usize; 2],
    /// The distinct positions (board, side to move) of each set's games.
    positions: [HashSet<(Board, Color)>; 2],
}

impl DataSet {
    /// Adds the games of a paired match, in the order of its schedule, from ply `skip_plies` on.
    pub fn add_match<'a>(
        &mut self,
        games: impl IntoIterator<Item = &'a GameRecord>,
        skip_plies: usize,
    ) {
        for (g, game) in games.into_iter().enumerate() {
            let set = usize::from(is_validation(g));
            let samples = samples_of(game, skip_plies);
            self.positions[set].extend(quiet_positions(game, skip_plies));
            self.games[set] += 1;
            if set == 1 {
                self.validation.extend(samples);
            } else {
                self.train.extend(samples);
            }
        }
    }

    /// The distinct validation positions that come up in a training game too.
    pub fn shared(&self) -> usize {
        self.positions[1].intersection(&self.positions[0]).count()
    }

    /// The distinct positions of the validation games.
    pub fn validation_positions(&self) -> usize {
        self.positions[1].len()
    }
}

/// 1 / (1 + e^−x).
fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}

/// The mean squared error of sigmoid(k · value) against the results.
pub fn mse(samples: &[Sample], w: &Weights, k: f64) -> f64 {
    let w = to_array(w);
    error(samples, &w, k)
}

fn error(samples: &[Sample], w: &[f64; N_WEIGHTS], k: f64) -> f64 {
    let sum: f64 = samples
        .iter()
        .map(|s| (s.result - sigmoid(k * value(&s.features, w))).powi(2))
        .sum();
    sum / samples.len() as f64
}

/// The K in [0.001, 10] with the smallest `mse` for the weights `w`: a golden-section search,
/// which assumes one minimum in the interval.
pub fn fit_k(samples: &[Sample], w: &Weights) -> f64 {
    let w = to_array(w);
    let ratio = (5f64.sqrt() - 1.0) / 2.0;
    let (mut lo, mut hi) = (0.001, 10.0);
    let mut a = hi - ratio * (hi - lo);
    let mut b = lo + ratio * (hi - lo);
    let (mut ea, mut eb) = (error(samples, &w, a), error(samples, &w, b));
    while hi - lo > 1e-6 {
        if ea < eb {
            (hi, b, eb) = (b, a, ea);
            a = hi - ratio * (hi - lo);
            ea = error(samples, &w, a);
        } else {
            (lo, a, ea) = (a, b, eb);
            b = lo + ratio * (hi - lo);
            eb = error(samples, &w, b);
        }
    }
    (lo + hi) / 2.0
}

/// The result of `tune`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    /// The weights found, rounded to 4 decimals.
    pub weights: Weights,
    /// Their `mse` on the samples.
    pub error: f64,
    /// The errors computed on the way.
    pub evaluations: u32,
}

/// The weights near `start` with the smallest `mse` at this `k`, the weights named in `fixed`
/// (`Weights::KEYS`) left as they are: for each step of `STEPS`, one weight after another is
/// moved by + or − the step while that lowers the error, until no move does.
pub fn tune(samples: &[Sample], start: &Weights, k: f64, fixed: &[&str]) -> Tuning {
    let free: Vec<usize> = (0..N_WEIGHTS)
        .filter(|&i| !fixed.contains(&Weights::KEYS[i]))
        .collect();
    let mut w = to_array(start);
    let mut best = error(samples, &w, k);
    let mut evaluations = 1;
    for step in STEPS {
        loop {
            let mut improved = false;
            for &i in &free {
                let old = w[i];
                for candidate in [old + step, old - step] {
                    w[i] = candidate;
                    let e = error(samples, &w, k);
                    evaluations += 1;
                    if e < best {
                        best = e;
                        improved = true;
                        break;
                    }
                    w[i] = old;
                }
            }
            if !improved {
                break;
            }
        }
    }
    // The steps keep the weights on a grid of 0.0001 but for rounding errors of the additions.
    let weights = from_array(&w.map(|x| (x * 1e4).round() / 1e4));
    Tuning {
        weights,
        error: mse(samples, &weights, k),
        evaluations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_go_to_an_array_and_back() {
        let w = Weights {
            adv: 0.1,
            qmob: -0.02,
            ..Weights::TUNED
        };
        assert_eq!(to_array(&w), [1.0, 2.5, -0.05, 0.1, 0.0, 0.0, 0.0, -0.02]);
        assert_eq!(from_array(&to_array(&w)), w);
    }

    #[test]
    fn validation_takes_whole_pairs() {
        let validation: Vec<usize> = (0..30).filter(|&g| is_validation(g)).collect();
        assert_eq!(validation, [8, 9, 18, 19, 28, 29]);
    }
}
