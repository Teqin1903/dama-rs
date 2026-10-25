//! Turkish draughts (TÜDAF rules): board, move generation and perft.
//!
//! `reference/js/engine.js` is the reference: the same square numbers, the same text formats
//! (docs/reference.md) and the same order of legal moves.

mod bitboard;
mod board;
mod movegen;
mod perft;
mod square;

pub use bitboard::Bitboard;
pub use board::{Board, Color, Kind, ParseBoardError, Piece};
pub use movegen::Move;
pub use perft::perft;
pub use square::Square;
