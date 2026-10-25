//! Command-line front end of the engine. The text protocol (position / go / bestmove) comes in a
//! later session; for now it prints the start position and White's legal moves.

use engine::{Board, Color};

fn main() {
    let board = Board::start();
    let moves: Vec<String> = board
        .legal_moves(Color::White)
        .iter()
        .map(|m| m.to_string())
        .collect();
    println!("{board}");
    println!("{} legal moves for White: {}", moves.len(), moves.join(" "));
}
