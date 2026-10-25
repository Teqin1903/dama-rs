//! U, the baseline agent's utility (docs/reference.md; `U` in reference/js/agent.js).

use crate::board::{Board, Color, Kind};

/// U of a move after which the opponent has no legal move: a win.
pub const WIN: f64 = 1000.0;

/// Two U values closer than this are equal (docs/reference.md).
pub const U_EPSILON: f64 = 1e-9;

/// The weights of U.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weights {
    pub man: f64,
    pub king: f64,
    pub mob: f64,
}

impl Weights {
    /// The tuned weights of our Assignment 3 talk.
    pub const TUNED: Weights = Weights {
        man: 1.0,
        king: 2.5,
        mob: -0.05,
    };

    /// The first guess, before tuning.
    pub const FIRST_GUESS: Weights = Weights {
        man: 1.0,
        king: 3.0,
        mob: 0.1,
    };
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
