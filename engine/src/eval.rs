//! U, the baseline agent's utility (docs/reference.md; `U` in reference/js/agent.js), and the
//! positional features we add to it (`Positional`), each with a weight that is 0 unless an agent's
//! name sets it.
//!
//! All features are differences, ours minus theirs, from the view of the side that has just moved
//! (at a search leaf: the root side). A man's home row is the rearmost row it can stand on: rank 2
//! for White, rank 7 for Black (men never move backwards, so they never reach their own rank 1 or
//! 8).
//!
//! - `adv`, advancement: the rows each man has moved up from its home row (White rank − 2, Black
//!   7 − rank, so 0 to 5), summed over the men.
//! - `back`, back row: the men still on their home row, the last guard against an enemy man coming
//!   to be crowned.
//! - `cen`, centre: the pieces, men and kings, on the 16 squares c3–f6.
//! - `edge`, edge: the pieces on files a and h.
//! - `qmob`, quiet mobility: the quiet moves (no capture) the pieces have, counted as if capturing
//!   were not mandatory. U's `mob` counts legal moves, which collapse to the few longest captures
//!   whenever a side must capture; `qmob` does not.
//!
//! With every positional weight 0, `evaluate` is U itself: the same double, computed the same way.

use std::fmt;

use crate::bitboard::Bitboard;
use crate::board::{Board, Color, Kind};

/// U of a move after which the opponent has no legal move: a win.
pub const WIN: f64 = 1000.0;

/// The value of a position the search has seen before (a repetition): a draw, worth neither side
/// anything.
pub const DRAW: f64 = 0.0;

/// Two U values closer than this are equal (docs/reference.md).
pub const U_EPSILON: f64 = 1e-9;

/// The weights of U (`man`, `king`, `mob`) and of the positional features (the module
/// documentation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weights {
    pub man: f64,
    pub king: f64,
    pub mob: f64,
    pub adv: f64,
    pub back: f64,
    pub cen: f64,
    pub edge: f64,
    pub qmob: f64,
}

impl Weights {
    /// Every weight 0.
    pub const ZERO: Weights = Weights {
        man: 0.0,
        king: 0.0,
        mob: 0.0,
        adv: 0.0,
        back: 0.0,
        cen: 0.0,
        edge: 0.0,
        qmob: 0.0,
    };

    /// The tuned weights of our Assignment 3 talk.
    pub const TUNED: Weights = Weights {
        man: 1.0,
        king: 2.5,
        mob: -0.05,
        ..Weights::ZERO
    };

    /// The first guess, before tuning.
    pub const FIRST_GUESS: Weights = Weights {
        man: 1.0,
        king: 3.0,
        mob: 0.1,
        ..Weights::ZERO
    };

    /// Material only: a king is worth three men and mobility does not count.
    pub const MATERIAL: Weights = Weights {
        man: 1.0,
        king: 3.0,
        mob: 0.0,
        ..Weights::ZERO
    };

    /// The weight sets with a name, which `w=NAME` in an agent name starts from
    /// (`idqr:n2000:w=material`).
    pub const NAMED: [(&'static str, Weights); 3] = [
        ("material", Weights::MATERIAL),
        ("first", Weights::FIRST_GUESS),
        ("tuned", Weights::TUNED),
    ];

    /// The weight set called `name` (one of `NAMED`).
    pub fn named(name: &str) -> Option<Weights> {
        Weights::NAMED
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, w)| *w)
    }

    /// The names of the weights in agent names (`idqr:n2000:adv=0.1`), in the order they are written.
    pub const KEYS: [&'static str; 8] =
        ["man", "king", "mob", "adv", "back", "cen", "edge", "qmob"];

    /// The weight called `key` (one of `KEYS`).
    pub fn get(&self, key: &str) -> Option<f64> {
        Some(match key {
            "man" => self.man,
            "king" => self.king,
            "mob" => self.mob,
            "adv" => self.adv,
            "back" => self.back,
            "cen" => self.cen,
            "edge" => self.edge,
            "qmob" => self.qmob,
            _ => return None,
        })
    }

    /// The weight called `key`, to change it.
    fn get_mut(&mut self, key: &str) -> Option<&mut f64> {
        Some(match key {
            "man" => &mut self.man,
            "king" => &mut self.king,
            "mob" => &mut self.mob,
            "adv" => &mut self.adv,
            "back" => &mut self.back,
            "cen" => &mut self.cen,
            "edge" => &mut self.edge,
            "qmob" => &mut self.qmob,
            _ => return None,
        })
    }

    /// These weights with `overrides`, one or more `KEY=VALUE` separated by `:` (for example
    /// `adv=0.1:king=3`): each KEY one of `KEYS`, at most once, and each VALUE a finite number.
    /// The first part may instead be `w=NAME`: the weights then start from the set `NAME` of
    /// `NAMED` (`w=material:mob=0.1`).
    pub fn with_overrides(mut self, overrides: &str) -> Result<Weights, String> {
        let mut seen = Vec::new();
        for (i, part) in overrides.split(':').enumerate() {
            let (key, value) = part
                .split_once('=')
                .ok_or_else(|| format!("{part:?} is not KEY=VALUE"))?;
            if key == "w" {
                if i > 0 {
                    return Err("w=NAME must come before the other weights".to_string());
                }
                self = Weights::named(value).ok_or_else(|| {
                    let names: Vec<&str> = Weights::NAMED.iter().map(|(n, _)| *n).collect();
                    format!("w={value}: expected one of {}", names.join(", "))
                })?;
                continue;
            }
            if seen.contains(&key) {
                return Err(format!("{key} is set twice"));
            }
            seen.push(key);
            let weight = self
                .get_mut(key)
                .ok_or_else(|| format!("unknown weight {key:?}"))?;
            *weight = value
                .parse::<f64>()
                .ok()
                .filter(|x| x.is_finite())
                .ok_or_else(|| format!("{key}={value}: expected a finite number"))?;
        }
        Ok(self)
    }

    /// The weights that differ from `base`, as `:KEY=VALUE` parts in the order of `KEYS`: the
    /// suffix that `with_overrides` turns `base` back into these weights with.
    pub fn overrides_from(&self, base: &Weights) -> String {
        let mut parts = String::new();
        for key in Weights::KEYS {
            let (ours, theirs) = (self.get(key), base.get(key));
            if ours != theirs {
                parts.push_str(&format!(":{key}={}", ours.expect("a key of KEYS")));
            }
        }
        parts
    }

    /// The suffix that names these weights in a search agent's name: `overrides_from` the named
    /// set it needs the fewest parts from, written `:w=NAME` first unless that set is the tuned
    /// one (on a tie the tuned set, so a name without `w=` keeps its old spelling).
    pub fn suffix(&self) -> String {
        let tuned = self.overrides_from(&Weights::TUNED);
        let count = |s: &str| s.matches(':').count();
        let mut best = tuned;
        for (name, base) in Weights::NAMED {
            if base == Weights::TUNED {
                continue;
            }
            let candidate = format!(":w={name}{}", self.overrides_from(&base));
            if count(&candidate) < count(&best) {
                best = candidate;
            }
        }
        best
    }

    /// Whether any positional weight is not 0: only then does `evaluate` compute the features.
    pub fn has_positional(&self) -> bool {
        [self.adv, self.back, self.cen, self.edge, self.qmob]
            .iter()
            .any(|&w| w != 0.0)
    }
}

impl fmt::Display for Weights {
    /// `man/king/mob`, the way we write U's weights: `1/2.5/-0.05`. The positional weights are not
    /// shown (`overrides_from` writes them).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}/{}", self.man, self.king, self.mob)
    }
}

/// The features of U on a board right after `side` has moved, so the opponent is to move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Features {
    /// Our men minus their men.
    pub men: i32,
    /// Our kings minus their kings.
    pub kings: i32,
    /// Our legal moves minus their legal moves, both counted on this board with the full rules.
    pub mob: i32,
    /// The opponent has no legal move. The other features are then 0, as in agent.js.
    pub win: bool,
}

impl Features {
    /// The features of `board`, on which `side` has just moved.
    pub fn of(board: &Board, side: Color) -> Features {
        let opponent = side.opponent();
        let their_moves = board.legal_moves(opponent).len() as i32;
        if their_moves == 0 {
            return Features {
                men: 0,
                kings: 0,
                mob: 0,
                win: true,
            };
        }
        let difference = |kind| board.count(side, kind) as i32 - board.count(opponent, kind) as i32;
        Features {
            men: difference(Kind::Man),
            kings: difference(Kind::King),
            mob: board.legal_moves(side).len() as i32 - their_moves,
            win: false,
        }
    }

    /// The features of `board` from `side`'s view at a leaf of the minimax search, where either
    /// side may be to move: never a win (a side without moves is caught before the leaf), and the
    /// move counts come from the caller, which has already generated one of them.
    pub fn leaf(board: &Board, side: Color, our_moves: usize, their_moves: usize) -> Features {
        let opponent = side.opponent();
        let difference = |kind| board.count(side, kind) as i32 - board.count(opponent, kind) as i32;
        Features {
            men: difference(Kind::Man),
            kings: difference(Kind::King),
            mob: our_moves as i32 - their_moves as i32,
            win: false,
        }
    }

    /// U with the weights `w`: `WIN` for a win, otherwise man·men + king·kings + mob·mob.
    /// Computed in this order in f64 it is the same double as in agent.js: Rust neither reorders
    /// these operations nor fuses a multiplication and an addition into one FMA.
    pub fn u(&self, w: Weights) -> f64 {
        if self.win {
            return WIN;
        }
        w.man * f64::from(self.men) + w.king * f64::from(self.kings) + w.mob * f64::from(self.mob)
    }
}

/// The squares of rank `r` (0 = rank 1).
const fn rank(r: u32) -> u64 {
    0xff << (8 * r)
}

/// c3–f6.
const CENTER: Bitboard = Bitboard::from_bits(0x0000_3c3c_3c3c_0000);

/// Files a and h.
const FILES_A_H: Bitboard = Bitboard::from_bits(0x8181_8181_8181_8181);

/// The positional features of a board from one side's view (module documentation).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Positional {
    pub adv: i32,
    pub back: i32,
    pub cen: i32,
    pub edge: i32,
    pub qmob: i32,
}

impl Positional {
    /// The features of `board` for `side`: its counts minus the opponent's.
    pub fn of(board: &Board, side: Color) -> Positional {
        let ours = Positional::counts(board, side);
        let theirs = Positional::counts(board, side.opponent());
        Positional {
            adv: ours.adv - theirs.adv,
            back: ours.back - theirs.back,
            cen: ours.cen - theirs.cen,
            edge: ours.edge - theirs.edge,
            qmob: ours.qmob - theirs.qmob,
        }
    }

    /// The counts of `color` alone.
    fn counts(board: &Board, color: Color) -> Positional {
        let pieces = board.pieces(color);
        let men = (pieces & !board.kings()).bits();
        // Rank 2 to rank 7 are ranks 1 to 6 counted from 0; a man on the home row has advanced 0.
        let (home, rows_up): (u32, fn(u32) -> u32) = match color {
            Color::White => (1, |r| r - 1),
            Color::Black => (6, |r| 6 - r),
        };
        let adv = (1..=6)
            .map(|r| rows_up(r) * (men & rank(r)).count_ones())
            .sum::<u32>();
        Positional {
            adv: adv as i32,
            back: (men & rank(home)).count_ones() as i32,
            cen: (pieces & CENTER).len() as i32,
            edge: (pieces & FILES_A_H).len() as i32,
            qmob: board.quiet_move_count(color) as i32,
        }
    }

    /// adv·adv + back·back + cen·cen + edge·edge + qmob·qmob, in this order.
    pub fn u(&self, w: Weights) -> f64 {
        w.adv * f64::from(self.adv)
            + w.back * f64::from(self.back)
            + w.cen * f64::from(self.cen)
            + w.edge * f64::from(self.edge)
            + w.qmob * f64::from(self.qmob)
    }
}

/// The value of `board`, on which U's `features` were counted from `side`'s view, with the weights
/// `w`: U, plus the positional features when a positional weight is set and the position is not a
/// win. With every positional weight 0 it is `features.u(w)`, bit for bit, and costs nothing more.
pub fn evaluate(board: &Board, side: Color, features: &Features, w: Weights) -> f64 {
    let u = features.u(w);
    if features.win || !w.has_positional() {
        return u;
    }
    u + Positional::of(board, side).u(w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn u_is_the_same_double_as_in_agent_js() {
        // g2-h2 in the slide-7 position of testdata/decisions.json has u 7.95.
        let g2_h2 = Features {
            men: 10,
            kings: -1,
            mob: -9,
            win: false,
        };
        assert_eq!(g2_h2.u(Weights::TUNED), 7.95);
    }

    #[test]
    fn a_win_is_worth_win_whatever_the_weights() {
        let win = Features {
            men: 0,
            kings: 0,
            mob: 0,
            win: true,
        };
        assert_eq!(win.u(Weights::TUNED), WIN);
        assert_eq!(win.u(Weights::FIRST_GUESS), WIN);
    }

    #[test]
    fn features_after_a_move() {
        // The start position as if White had just moved: equal material, 8 legal moves each.
        let start = Features::of(&Board::start(), Color::White);
        assert_eq!(
            start,
            Features {
                men: 0,
                kings: 0,
                mob: 0,
                win: false
            }
        );
        // White has just taken Black's last piece: Black has no legal move.
        let board: Board =
            "......../......../...w..../......../......../......../......../........"
                .parse()
                .unwrap();
        assert!(Features::of(&board, Color::White).win);
    }
}
