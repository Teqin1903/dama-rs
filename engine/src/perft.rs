//! Perft: the number of leaves of the move tree, the standard check of a move generator.

use crate::board::{Board, Color};

/// The number of leaves of the move tree `depth` plies deep from `board`, `side` to move.
///
/// As in engine.js and kish, a position with no legal move counts as one leaf, and the last ply
/// is counted without being played (bulk counting).
pub fn perft(board: &Board, side: Color, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = board.legal_moves(side);
    if moves.is_empty() {
        return 1;
    }
    if depth == 1 {
        return moves.len() as u64;
    }
    moves
        .iter()
        .map(|mv| perft(&board.apply(mv), side.opponent(), depth - 1))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_position_has_eight_moves() {
        assert_eq!(perft(&Board::start(), Color::White, 0), 1);
        assert_eq!(perft(&Board::start(), Color::White, 1), 8);
    }

    #[test]
    fn a_side_without_moves_is_one_leaf() {
        // The white man on a2 is boxed in: a3 and b2 are taken, a4 and c2 block the jumps.
        let board: Board =
            "......../......../......../......../b......./b......./wbb...../........"
                .parse()
                .unwrap();
        assert!(board.legal_moves(Color::White).is_empty());
        for depth in 0..=3 {
            assert_eq!(perft(&board, Color::White, depth), 1, "depth {depth}");
        }
    }
}
