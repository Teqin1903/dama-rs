//! The baseline agent's search: one ply (`decide` in reference/js/agent.js). Minimax will come
//! later, behind this module.

use crate::board::{Board, Color};
use crate::eval::{Features, U_EPSILON, Weights};
use crate::movegen::Move;
use crate::rng::Mulberry32;

/// A legal move with the features and U of the board after it.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub mv: Move,
    pub features: Features,
    pub u: f64,
}

/// Every legal move evaluated one ply deep, and the ones with the highest U.
#[derive(Clone, Debug)]
pub struct Decision {
    /// One candidate per legal move, in legal-move order (the order of engine.js).
    pub candidates: Vec<Candidate>,
    /// Indices into `candidates` of the moves whose U is within `U_EPSILON` of the highest, in
    /// order.
    pub top: Vec<usize>,
}

/// Evaluates every legal move of `side` on `board` with U.
pub fn decide(board: &Board, side: Color, weights: Weights) -> Decision {
    let candidates: Vec<Candidate> = board
        .legal_moves(side)
        .into_iter()
        .map(|mv| {
            let features = Features::of(&board.apply(&mv), side);
            Candidate {
                u: features.u(weights),
                features,
                mv,
            }
        })
        .collect();
    // f64 has no total order (NaN), so the highest U is a fold rather than `max()`.
    let best = candidates
        .iter()
        .map(|c| c.u)
        .fold(f64::NEG_INFINITY, f64::max);
    let top = (0..candidates.len())
        .filter(|&i| best - candidates[i].u <= U_EPSILON)
        .collect();
    Decision { candidates, top }
}

impl Decision {
    /// The move to play, or `None` if there is no legal move. Without `rng` it is the first move
    /// with the highest U. With `rng` it is picked as agent.js does,
    /// `top[Math.floor(rnd() * top.length)]`, which draws a number even when one move is best.
    pub fn choose(&self, rng: Option<&mut Mulberry32>) -> Option<&Move> {
        if self.top.is_empty() {
            return None;
        }
        let i = match rng {
            Some(rng) => (rng.next_f64() * self.top.len() as f64).floor() as usize,
            None => 0,
        };
        Some(&self.candidates[self.top[i]].mv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A decision over the first `n` moves of the start position, all with the same U.
    fn tied(n: usize) -> Decision {
        let features = Features {
            men: 0,
            kings: 0,
            mob: 0,
            win: false,
        };
        let candidates: Vec<Candidate> = Board::start()
            .legal_moves(Color::White)
            .into_iter()
            .take(n)
            .map(|mv| Candidate {
                mv,
                features,
                u: 0.0,
            })
            .collect();
        Decision {
            top: (0..candidates.len()).collect(),
            candidates,
        }
    }

    #[test]
    fn choose_picks_top_at_floor_of_rnd_times_len() {
        let decision = tied(3);
        assert_eq!(decision.choose(None), Some(&decision.candidates[0].mv));
        // Seed 2026 draws 0.455... first, and floor(0.455... * 3) = 1.
        let mut rng = Mulberry32::new(2026);
        assert_eq!(
            decision.choose(Some(&mut rng)),
            Some(&decision.candidates[1].mv)
        );
    }

    #[test]
    fn choose_draws_a_number_even_with_one_top_move() {
        let decision = tied(1);
        let mut rng = Mulberry32::new(1);
        let mut reference = Mulberry32::new(1);
        decision.choose(Some(&mut rng));
        reference.next_f64();
        assert_eq!(rng.next_u32(), reference.next_u32());
    }

    #[test]
    fn no_legal_move_no_choice_and_no_draw() {
        let decision = tied(0);
        let mut rng = Mulberry32::new(1);
        assert_eq!(decision.choose(Some(&mut rng)), None);
        assert_eq!(rng.next_u32(), Mulberry32::new(1).next_u32());
    }
}
