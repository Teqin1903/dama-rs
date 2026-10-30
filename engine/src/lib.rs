//! Turkish draughts (TÜDAF rules): board, move generation, perft, the baseline agent, the
//! minimax, alpha-beta and iterative-deepening agents, and the arena that plays agents against each other
//! (with an SPRT that stops a match once its result is clear), and Texel tuning of the weights.
//!
//! `reference/js/` is the reference: the same square numbers, the same text formats
//! (docs/reference.md), the same order of legal moves, the same U, the same random numbers and the
//! same games.

mod agent;
mod api;
mod arena;
mod bitboard;
mod board;
mod elo;
mod eval;
mod json;
mod ladder;
mod movegen;
mod perft;
mod rng;
mod search;
mod sprt;
mod square;
mod table;
mod texel;
mod trace;
mod zobrist;

pub use agent::{
    Agent, AgentError, AgentSpec, AlphaBetaAgent, DeepeningAgent, MinimaxAgent, ParseAgentError,
    RandomAgent, Thought, UAgent,
};
pub use api::{Session, native_clock, version};
pub use arena::{
    AgentFactory, EndReason, GamePosition, GameRecord, MatchGame, MatchPlan, MatchRecord,
    MatchSettings, Schedule, Thinking, game_end, paired_agent_seeds, paired_opening_seed,
    play_game, play_match,
};
pub use bitboard::Bitboard;
pub use board::{Board, Color, Kind, ParseBoardError, Piece};
pub use elo::{Bootstrap, Fit, PairedMatch, Score, bootstrap, elo_or_infinite, fit, percentile};
pub use eval::{
    Breakdown, DRAW, FeatureTerm, Features, Positional, SquareTerm, U_EPSILON, WIN, Weights,
    breakdown, evaluate,
};
pub use json::Json;
pub use ladder::{
    ABLATION, ABLATION_FOOTNOTES, ABLATION_SET, ANCHOR, FOOTNOTES, GAMES_HEADER, GameRow, LADDER,
    LADDER_SET, LONG_MATCH, MID, Meta, Pairing, PairingSummary, Report, Role, Rung, SEED,
    SHORT_MATCH, Set, SideStats, VersionSummary, ablation_plan, plan, rate, rows_of,
};
pub use movegen::Move;
pub use perft::perft;
pub use rng::{Mulberry32, derive_seed};
pub use search::{
    Candidate, Decision, Deepening, Limits, MAX_ID_DEPTH, SearchOptions, alpha_beta, alpha_beta_q,
    alpha_beta_rep, alpha_beta_traced, alpha_beta_with, decide, decide_traced, iterative_deepening,
    iterative_deepening_rep, iterative_deepening_traced, iterative_deepening_with, minimax,
    minimax_q, minimax_traced, static_value,
};
pub use sprt::{Pentanomial, Sprt, SprtResult, Verdict, elo_of, expected_score};
pub use square::Square;
pub use table::{Table, TableOptions};
pub use texel::{
    DataSet, N_WEIGHTS, STEPS, Sample, Tuning, features_of, fit_k, from_array, is_validation, mse,
    quiet_positions, samples_of, to_array, tune, value,
};
pub use trace::{
    Enter, Iteration, NoTrace, Reason, Recorder, RootMove, Stats, TraceNode, TraceRequest, Tracer,
    Tree, TtProbe, ValueBound,
};
pub use zobrist::{zobrist, zobrist_after};
