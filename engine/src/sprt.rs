//! The sequential probability ratio test (SPRT) that stops a match as soon as its result is clear.
//!
//! The paired schedule plays every opening twice with the colours swapped, so the two games of a
//! pair are not independent: an opening that favours White gives each agent one good game. The
//! unit of the test is therefore the pair, whose score for A is 0, ½, 1, 1½ or 2: the pentanomial
//! model (the five counts are a `Pentanomial`).
//!
//! The hypotheses are about A's Elo over B on the logistic scale, where an Elo difference e gives
//! the expected score s(e) = 1 / (1 + 10^(−e/400)):
//!
//! - H0: e = elo0 (A is no better), H1: e = elo1 (A is better by elo1).
//!
//! The log-likelihood ratio is the generalised SPRT approximation of M. Van den Bergh, with the
//! pair scores scaled to [0, 1], their mean μ and variance σ² over N pairs:
//!
//! ```text
//! LLR = N · (s1 − s0) · (2μ − s0 − s1) / (2σ²)
//! ```
//!
//! The test accepts H1 when LLR ≥ ln((1 − β) / α), H0 when LLR ≤ ln(β / (1 − α)), and goes on
//! otherwise: α is the chance of accepting H1 when H0 holds, β that of accepting H0 when H1 holds.
//! It decides only after `min_games` games (`min_games / 2` pairs, rounded up): with a handful of
//! pairs the variance is a poor estimate, and a lopsided start can cross a bound by luck.

use std::fmt;

/// The settings of a test.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sprt {
    /// A's Elo over B under H0.
    pub elo0: f64,
    /// A's Elo over B under H1; more than `elo0`.
    pub elo1: f64,
    /// The chance of accepting H1 when H0 holds.
    pub alpha: f64,
    /// The chance of accepting H0 when H1 holds.
    pub beta: f64,
    /// No verdict before this many games (pairs × 2) have been counted.
    pub min_games: u32,
}

impl Default for Sprt {
    /// Elo0 = 0, Elo1 = 20, α = β = 0.05, at least 100 games.
    fn default() -> Sprt {
        Sprt {
            elo0: 0.0,
            elo1: 20.0,
            alpha: 0.05,
            beta: 0.05,
            min_games: 100,
        }
    }
}

/// The expected score of a side `elo` Elo stronger than its opponent.
pub fn expected_score(elo: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf(-elo / 400.0))
}

/// The Elo difference that gives the expected score `score` (0 < score < 1).
pub fn elo_of(score: f64) -> f64 {
    -400.0 * (1.0 / score - 1.0).log10()
}

impl Sprt {
    /// The LLR at or below which H0 is accepted, and at or above which H1 is.
    pub fn bounds(&self) -> (f64, f64) {
        (
            (self.beta / (1.0 - self.alpha)).ln(),
            ((1.0 - self.beta) / self.alpha).ln(),
        )
    }

    /// The log-likelihood ratio of H1 against H0 after the pairs in `pairs`; 0 while their
    /// scores do not vary (the variance is 0, so there is nothing to measure yet).
    pub fn llr(&self, pairs: &Pentanomial) -> f64 {
        let Some((mean, variance)) = pairs.mean_variance() else {
            return 0.0;
        };
        if variance <= 0.0 {
            return 0.0;
        }
        let (s0, s1) = (expected_score(self.elo0), expected_score(self.elo1));
        pairs.pairs() as f64 * (s1 - s0) * (2.0 * mean - s0 - s1) / (2.0 * variance)
    }

    /// The verdict of `llr` after `pairs` pairs, or `None` if the test goes on: always before
    /// `min_games` games.
    pub fn verdict(&self, llr: f64, pairs: u32) -> Option<Verdict> {
        let (lower, upper) = self.bounds();
        if 2 * pairs < self.min_games {
            None
        } else if llr >= upper {
            Some(Verdict::H1)
        } else if llr <= lower {
            Some(Verdict::H0)
        } else {
            None
        }
    }
}

/// How a test ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// A is not better by `elo1`.
    H0,
    /// A is better than `elo0`.
    H1,
    /// The match reached its most games before either bound.
    Inconclusive,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Verdict::H0 => "H0",
            Verdict::H1 => "H1",
            Verdict::Inconclusive => "none",
        })
    }
}

/// The number of pairs in which A scored 0, ½, 1, 1½ and 2.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pentanomial(pub [u32; 5]);

impl Pentanomial {
    /// Counts one more pair, in which A scored `score` (0, 0.5, 1, 1.5 or 2).
    pub fn add(&mut self, score: f64) {
        let half_points = (score * 2.0).round() as usize;
        self.0[half_points] += 1;
    }

    pub fn pairs(&self) -> u32 {
        self.0.iter().sum()
    }

    /// The mean and the variance (Σ(x − μ)² / N) of the pair scores scaled to [0, 1], or `None`
    /// without pairs.
    fn mean_variance(&self) -> Option<(f64, f64)> {
        let n = f64::from(self.pairs());
        if n == 0.0 {
            return None;
        }
        // Pair score i half-points scaled to [0, 1] is i / 4.
        let scaled = |i: usize| i as f64 / 4.0;
        let mean = (0..5)
            .map(|i| f64::from(self.0[i]) * scaled(i))
            .sum::<f64>()
            / n;
        let variance = (0..5)
            .map(|i| f64::from(self.0[i]) * (scaled(i) - mean).powi(2))
            .sum::<f64>()
            / n;
        Some((mean, variance))
    }

    /// A's Elo over B from these pairs and the half-width of its 95% interval, or `None` without
    /// pairs or when A scored everything or nothing (the Elo is then infinite).
    pub fn elo(&self) -> Option<(f64, f64)> {
        let (mean, variance) = self.mean_variance()?;
        if mean <= 0.0 || mean >= 1.0 {
            return None;
        }
        let error = 1.96 * (variance / f64::from(self.pairs())).sqrt();
        let (low, high) = ((mean - error).max(1e-9), (mean + error).min(1.0 - 1e-9));
        Some((elo_of(mean), (elo_of(high) - elo_of(low)) / 2.0))
    }
}

/// The outcome of a test in a match.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SprtResult {
    pub sprt: Sprt,
    pub verdict: Verdict,
    /// The LLR after the last pair counted.
    pub llr: f64,
    /// The pairs counted, in the order of the schedule.
    pub pairs: Pentanomial,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_of_five_percent() {
        let (lower, upper) = Sprt::default().bounds();
        assert!((lower + 2.944_438_979_166_44).abs() < 1e-12);
        assert!((upper - 2.944_438_979_166_44).abs() < 1e-12);
    }

    #[test]
    fn llr_of_a_known_sample() {
        // Computed in Python with the formula of the module documentation.
        let pairs = Pentanomial([3, 10, 50, 20, 7]);
        let llr = Sprt::default().llr(&pairs);
        assert!((llr - 1.999_100_947_872_034_7).abs() < 1e-12, "{llr}");
        let (elo, error) = pairs.elo().unwrap();
        assert!((elo - 34.860_070_287_560_08).abs() < 1e-9, "{elo}");
        assert!((error - 31.225_471_012_110_017).abs() < 1e-9, "{error}");
    }

    #[test]
    fn the_llr_leans_towards_the_hypothesis_the_scores_fit() {
        let sprt = Sprt::default();
        // An even match: as many lost pairs as won ones.
        let even = Pentanomial([0, 30, 40, 30, 0]);
        assert!(sprt.llr(&even) < 0.0);
        // A clearly better A.
        let better = Pentanomial([0, 10, 40, 50, 0]);
        assert!(sprt.llr(&better) > 0.0);
        assert_eq!(sprt.verdict(sprt.llr(&better), 100), Some(Verdict::H1));
        // More of the same evidence moves the LLR further.
        let even_twice = Pentanomial([0, 60, 80, 60, 0]);
        assert!(sprt.llr(&even_twice) < sprt.llr(&even));
    }

    #[test]
    fn no_verdict_before_the_minimum_of_games() {
        let sprt = Sprt::default();
        for llr in [-100.0, 100.0] {
            assert_eq!(sprt.verdict(llr, 49), None);
        }
        assert_eq!(sprt.verdict(-100.0, 50), Some(Verdict::H0));
        assert_eq!(sprt.verdict(100.0, 50), Some(Verdict::H1));
        let odd = Sprt {
            min_games: 7,
            ..Sprt::default()
        };
        assert_eq!(odd.verdict(100.0, 3), None);
        assert_eq!(odd.verdict(100.0, 4), Some(Verdict::H1));
    }

    #[test]
    fn no_spread_no_evidence() {
        let sprt = Sprt::default();
        assert_eq!(sprt.llr(&Pentanomial::default()), 0.0);
        assert_eq!(sprt.llr(&Pentanomial([0, 0, 25, 0, 0])), 0.0);
        assert_eq!(Pentanomial([0, 0, 0, 0, 9]).elo(), None);
    }

    #[test]
    fn pair_scores_go_to_their_counts() {
        let mut pairs = Pentanomial::default();
        for score in [0.0, 0.5, 1.0, 1.0, 1.5, 2.0] {
            pairs.add(score);
        }
        assert_eq!(pairs, Pentanomial([1, 1, 2, 1, 1]));
        assert_eq!(pairs.pairs(), 6);
    }
}
