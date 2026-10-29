//! The Zobrist referee: the key after a move (`zobrist_after`) is always the key computed from
//! scratch (`zobrist`) of the board after it, on every legal move of the 541 fixed positions for
//! both sides, at every node of the perft tree to depth 5, and along random games to their end
//! (multiple captures, promotions, kings that capture).

mod common;

use std::collections::HashMap;

use engine::{Board, Color, Kind, Mulberry32, zobrist, zobrist_after};
use serde::Deserialize;

#[derive(Deserialize)]
struct Position {
    board: String,
    side: String,
}

/// Every position of decisions.json and movegen.json, with its side to move.
fn positions() -> Vec<(Board, Color)> {
    let mut all = Vec::new();
    for file in ["decisions.json", "movegen.json"] {
        let entries: Vec<Position> = serde_json::from_str(&common::read_testdata(file)).unwrap();
        all.extend(
            (entries.into_iter())
                .map(|p| (p.board.parse::<Board>().unwrap(), common::side(&p.side))),
        );
    }
    assert_eq!(all.len(), 541);
    all
}

/// The errors of every move of `side` on `board`.
fn check_moves(board: &Board, side: Color, what: &str) -> Vec<String> {
    let key = zobrist(board, side);
    let mut errors = Vec::new();
    for mv in board.legal_moves(side) {
        let after = board.apply(&mv);
        if zobrist_after(key, board, &mv) != zobrist(&after, side.opponent()) {
            errors.push(format!("{what}: {side:?} {mv}"));
        }
    }
    errors
}

#[test]
fn the_key_after_every_move_of_the_fixed_positions_is_the_key_from_scratch() {
    let mut errors = Vec::new();
    let mut captures = 0;
    for (i, (board, _)) in positions().iter().enumerate() {
        for side in [Color::White, Color::Black] {
            errors.extend(check_moves(board, side, &format!("position {i}")));
            captures += board
                .legal_moves(side)
                .iter()
                .filter(|m| m.is_capture())
                .count();
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    assert!(
        captures > 100,
        "the positions should have captures: {captures}"
    );
}

/// Checks every move at every node below `board` to `depth` plies, as perft walks them.
fn walk(board: &Board, side: Color, depth: u32, errors: &mut Vec<String>) {
    if depth == 0 {
        return;
    }
    errors.extend(check_moves(board, side, &board.to_string()));
    for mv in board.legal_moves(side) {
        walk(&board.apply(&mv), side.opponent(), depth - 1, errors);
    }
}

#[test]
fn the_key_after_every_move_of_the_perft_tree_is_the_key_from_scratch() {
    let mut errors = Vec::new();
    walk(&Board::start(), Color::White, 5, &mut errors);
    assert!(
        errors.is_empty(),
        "{}",
        errors[..errors.len().min(20)].join("\n")
    );
}

#[test]
fn the_key_follows_random_games_to_their_end() {
    let (mut promotions, mut king_captures, mut multiple) = (0, 0, 0);
    for seed in 1..=200 {
        let mut rng = Mulberry32::new(seed);
        let (mut board, mut side) = (Board::start(), Color::White);
        let mut key = zobrist(&board, side);
        for _ in 0..300 {
            let moves = board.legal_moves(side);
            if moves.is_empty() {
                break;
            }
            let mv = &moves[rng.next_index(moves.len())];
            promotions += usize::from(mv.promotes());
            multiple += usize::from(mv.captured().len() > 1);
            king_captures += usize::from(
                mv.is_capture()
                    && board
                        .piece_at(mv.from())
                        .is_some_and(|p| p.kind == Kind::King),
            );
            key = zobrist_after(key, &board, mv);
            board = board.apply(mv);
            side = side.opponent();
            assert_eq!(key, zobrist(&board, side), "seed {seed}, after {mv}");
        }
    }
    // The games must have tried the hard cases.
    assert!(promotions > 50 && king_captures > 20 && multiple > 50);
}

#[test]
fn the_side_to_move_is_part_of_the_key_and_keys_rarely_collide() {
    let mut seen: HashMap<u64, (Board, Color)> = HashMap::new();
    for (board, side) in positions() {
        assert_ne!(zobrist(&board, side), zobrist(&board, side.opponent()));
        for side in [side, side.opponent()] {
            let position = (board, side);
            if let Some(other) = seen.insert(zobrist(&board, side), position) {
                assert_eq!(other, position, "two positions with one key");
            }
        }
    }
    // The empty board with White to move is the only position whose key is 0.
    assert_eq!(zobrist(&Board::empty(), Color::White), 0);
}
