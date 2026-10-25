//! Turkish draughts (TÜDAF rules): board, move generation, perft and the baseline agent.
//!
//! `reference/js/` is the reference: the same square numbers, the same text formats
//! (docs/reference.md), the same order of legal moves, the same U and the same random numbers.

mod bitboard;
mod board;
mod eval;
mod movegen;
mod perft;
mod rng;
mod search;
mod square;

pub use bitboard::Bitboard;
pub use board::{Board, Color, Kind, ParseBoardError, Piece};
pub use eval::{Features, U_EPSILON, WIN, Weights};
pub use movegen::Move;
pub use perft::perft;
pub use rng::Mulberry32;
pub use search::{Candidate, Decision, decide};
pub use square::Square;
