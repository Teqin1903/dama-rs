//! Turkish draughts (TÜDAF rules): board, move generation, perft, the baseline agent, the
//! minimax, alpha-beta and iterative-deepening agents, and the arena that plays agents against each other
//! (with an SPRT that stops a match once its result is clear), and Texel tuning of the weights.
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
mod sprt;
mod square;
mod table;
mod texel;
mod zobrist;

pub use agent::{
    Agent, AgentError, AgentSpec, AlphaBetaAgent, DeepeningAgent, MinimaxAgent, ParseAgentError,
    RandomAgent, UAgent,
};
pub use arena::{
    AgentFactory, EndReason, GameRecord, MatchGame, MatchPlan, MatchRecord, MatchSettings,
    Schedule, Thinking, play_game, play_match,
};
pub use bitboard::Bitboard;
pub use board::{Board, Color, Kind, ParseBoardError, Piece};
pub use eval::{DRAW, Features, Positional, U_EPSILON, WIN, Weights, evaluate};
pub use movegen::Move;
pub use perft::perft;
pub use rng::{Mulberry32, derive_seed};
pub use search::{
    Candidate, Decision, Deepening, Limits, MAX_ID_DEPTH, SearchOptions, alpha_beta, alpha_beta_q,
    alpha_beta_rep, alpha_beta_with, decide, iterative_deepening, iterative_deepening_rep,
    iterative_deepening_with, minimax, minimax_q,
};
pub use sprt::{Pentanomial, Sprt, SprtResult, Verdict, elo_of, expected_score};
pub use square::Square;
pub use table::{Table, TableOptions};
pub use texel::{
    DataSet, N_WEIGHTS, STEPS, Sample, Tuning, features_of, fit_k, from_array, is_validation, mse,
    quiet_positions, samples_of, to_array, tune, value,
};
pub use zobrist::{zobrist, zobrist_after};
