//! Turkish draughts (TÜDAF rules): board, move generation, perft, the baseline agent, the
//! minimax and alpha-beta agents and the arena that plays agents against each other.
//!
//! `reference/js/` is the reference: the same square numbers, the same text formats
//! (docs/reference.md), the same order of legal moves, the same U, the same random numbers and the
//! same games.

mod agent;
mod arena;
mod bitboard;
mod board;
mod eval;
mod movegen;
mod perft;
mod rng;
mod search;
mod square;

pub use agent::{
    Agent, AgentError, AgentSpec, AlphaBetaAgent, MinimaxAgent, ParseAgentError, RandomAgent,
    UAgent,
};
pub use arena::{
    EndReason, GameRecord, MatchGame, MatchRecord, MatchSettings, Thinking, play_game, play_match,
};
pub use bitboard::Bitboard;
pub use board::{Board, Color, Kind, ParseBoardError, Piece};
pub use eval::{Features, U_EPSILON, WIN, Weights};
pub use movegen::Move;
pub use perft::perft;
pub use rng::Mulberry32;
pub use search::{Candidate, Decision, alpha_beta, decide, minimax};
pub use square::Square;
