//! The arena: games between two agents under our match settings (`playGame` in
//! reference/js/agent.js), and matches of many games with fixed seeds.
//!
//! `play_game` is a port of `playGame`: the same end-of-game checks in the same order, and random
//! numbers drawn at the same moments. A seeded game here is the same seeded game in JS, move for
//! move.
//!
//! The seeds of a match are ours (docs/reference.md only asks for "the same seeds for every
//! pairing"). There are two schedules:
//!
//! ```text
//! Legacy (2 random opening plies), the schedule of every match before the paired one:
//!   seeds = Mulberry32::new(seed)                          one generator per match
//!   game g:  opening = Mulberry32::new(seeds.next_u32())   the random opening plies
//!            a.new_game(seeds.next_u32())                  agent A's own random numbers
//!            b.new_game(seeds.next_u32())                  agent B's own random numbers
//!            A plays White in even games and Black in odd games
//!
//! Paired (4 random opening plies): game g's seeds come from (seed, g) alone
//!   game g:  opening = Mulberry32::new(derive_seed(seed, OPENING, g / 2))
//!            a.new_game(derive_seed(seed, AGENT_A, g))
//!            b.new_game(derive_seed(seed, AGENT_B, g))
//!            A plays White in even games and Black in odd games
//! ```
//!
//! In both, game g gets the same seeds in every pairing and in a match of any length. The paired
//! schedule plays each opening twice, games 2p and 2p + 1, with the colours swapped: the opening
//! plies are drawn by the arena, not the agents, so the same seed gives the same opening.
//!
//! The seeds of every game are fixed before the first game starts, and every agent is reseeded at
//! the start of a game, so a game does not depend on the games played before it: the games of a
//! match can be played by several threads, in any order, with the same result.

use std::collections::HashMap;
use std::fmt;
use std::ops::AddAssign;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::agent::{Agent, AgentError};
use crate::board::{Board, Color};
use crate::movegen::Move;
use crate::rng::{Mulberry32, derive_seed};

/// A game is drawn when a position occurs for this many times, with the same side to move.
const REPETITIONS: u32 = 3;

/// A game is drawn after this many plies in a row without a capture.
const QUIET_PLIES: u32 = 50;

/// The settings of a match that may change. The other draw rules are fixed, as in agent.js.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchSettings {
    /// The first plies of every game are random legal moves drawn by the arena (`randomPlies`).
    pub opening_plies: u32,
    /// A game that reaches this ply is drawn (`maxPlies`).
    pub max_plies: u32,
}

impl MatchSettings {
    /// The settings of our experiments (docs/reference.md): 2 random opening plies, a draw at
    /// ply 400.
    pub const REFERENCE: MatchSettings = MatchSettings {
        opening_plies: 2,
        max_plies: 400,
    };

    /// The settings of the paired schedule: 4 random opening plies, a draw at ply 400.
    pub const PAIRED: MatchSettings = MatchSettings {
        opening_plies: 4,
        max_plies: 400,
    };
}

/// How the games of a match get their seeds, openings and colours (see the module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Schedule {
    /// One generator for the whole match, 2 random opening plies: the matches of
    /// results/arena.csv before the paired schedule, and of `bench/playgame_log.js`.
    Legacy,
    /// Seeds from (seed, game), 4 random opening plies, every opening played twice with the
    /// colours swapped.
    Paired,
}

/// The streams of `derive_seed` in the paired schedule.
const OPENING: u32 = 1;
const AGENT_A: u32 = 2;
const AGENT_B: u32 = 3;

impl Schedule {
    /// The match settings that go with the schedule.
    pub fn settings(self) -> MatchSettings {
        match self {
            Schedule::Legacy => MatchSettings::REFERENCE,
            Schedule::Paired => MatchSettings::PAIRED,
        }
    }

    /// The seeds of games 0 to `games - 1`.
    fn game_seeds(self, seed: u32, games: u32) -> Vec<GameSeeds> {
        match self {
            Schedule::Legacy => {
                let mut seeds = Mulberry32::new(seed);
                (0..games)
                    .map(|g| GameSeeds {
                        opening: seeds.next_u32(),
                        a: seeds.next_u32(),
                        b: seeds.next_u32(),
                        a_white: g % 2 == 0,
                    })
                    .collect()
            }
            Schedule::Paired => (0..games)
                .map(|g| GameSeeds {
                    opening: derive_seed(seed, OPENING, g / 2),
                    a: derive_seed(seed, AGENT_A, g),
                    b: derive_seed(seed, AGENT_B, g),
                    a_white: g % 2 == 0,
                })
                .collect(),
        }
    }
}

impl fmt::Display for Schedule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Schedule::Legacy => "legacy",
            Schedule::Paired => "paired",
        })
    }
}

impl FromStr for Schedule {
    type Err = String;

    fn from_str(s: &str) -> Result<Schedule, String> {
        match s {
            "legacy" => Ok(Schedule::Legacy),
            "paired" => Ok(Schedule::Paired),
            _ => Err(format!("unknown schedule {s:?}: expected legacy or paired")),
        }
    }
}

/// The seeds and colours of one game of a match.
#[derive(Clone, Copy, Debug)]
struct GameSeeds {
    opening: u32,
    a: u32,
    b: u32,
    a_white: bool,
}

/// Why a game ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EndReason {
    /// The same position occurred for the third time: a draw.
    Repetition,
    /// The side to move had no legal move and lost.
    NoMove,
    /// Each side had one piece left: a draw.
    OnePieceEach,
    /// 50 plies in a row without a capture: a draw.
    QuietPlies,
    /// The game reached the ply limit: a draw.
    Cap,
}

impl EndReason {
    /// Every reason, in the order playGame checks them.
    pub const ALL: [EndReason; 5] = [
        EndReason::Repetition,
        EndReason::NoMove,
        EndReason::OnePieceEach,
        EndReason::QuietPlies,
        EndReason::Cap,
    ];
}

impl fmt::Display for EndReason {
    /// The reason as playGame writes it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            EndReason::Repetition => "repetition",
            EndReason::NoMove => "no move",
            EndReason::OnePieceEach => "one piece each",
            EndReason::QuietPlies => "50 quiet plies",
            EndReason::Cap => "cap",
        })
    }
}

/// A finished game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRecord {
    /// `None` for a draw.
    pub winner: Option<Color>,
    pub reason: EndReason,
    /// Every move played, the random opening plies included.
    pub moves: Vec<Move>,
}

impl GameRecord {
    /// The number of plies played (`plies` in playGame).
    pub fn plies(&self) -> usize {
        self.moves.len()
    }
}

/// Plays a game from `start` with `side` to move: the port of `playGame`.
///
/// Before every ply, in the order of agent.js: the third occurrence of a position is a draw; a
/// side without a legal move loses; one piece each is a draw; 50 quiet plies are a draw; reaching
/// `max_plies` is a draw. The first `opening_plies` plies are random legal moves drawn from
/// `opening`, without asking the agents; after that each agent chooses its own moves.
pub fn play_game(
    start: Board,
    side: Color,
    white: &mut dyn Agent,
    black: &mut dyn Agent,
    opening: &mut Mulberry32,
    settings: &MatchSettings,
) -> Result<GameRecord, AgentError> {
    let (mut board, mut side) = (start, side);
    let (mut ply, mut quiet) = (0, 0);
    // How often each position, with its side to move, has occurred (`seen` in playGame).
    let mut seen: HashMap<(Board, Color), u32> = HashMap::new();
    let mut moves = Vec::new();
    // `loop` is an expression: `break` carries the end of the game out of it.
    let (winner, reason) = loop {
        let occurrences = seen.entry((board, side)).or_insert(0);
        *occurrences += 1;
        if *occurrences >= REPETITIONS {
            break (None, EndReason::Repetition);
        }
        let legal = board.legal_moves(side);
        if legal.is_empty() {
            break (Some(side.opponent()), EndReason::NoMove);
        }
        if board.pieces(Color::White).len() == 1 && board.pieces(Color::Black).len() == 1 {
            break (None, EndReason::OnePieceEach);
        }
        if quiet >= QUIET_PLIES {
            break (None, EndReason::QuietPlies);
        }
        if ply >= settings.max_plies {
            break (None, EndReason::Cap);
        }
        let mv = if ply < settings.opening_plies {
            &legal[opening.next_index(legal.len())]
        } else {
            match side {
                Color::White => white.choose(&board, side, &legal)?,
                Color::Black => black.choose(&board, side, &legal)?,
            }
        };
        quiet = if mv.is_capture() { 0 } else { quiet + 1 };
        board = board.apply(mv);
        moves.push(mv.clone());
        side = side.opponent();
        ply += 1;
    };
    Ok(GameRecord {
        winner,
        reason,
        moves,
    })
}

/// How long an agent thought during a match.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Thinking {
    /// The moves it chose. The random opening plies are the arena's, not the agent's.
    pub moves: u64,
    /// The time spent choosing them.
    pub time: Duration,
    /// The positions its search produced while choosing them (`Agent::nodes`).
    pub nodes: u64,
}

impl Thinking {
    /// The mean time per move, or `None` if the agent never moved.
    pub fn per_move(&self) -> Option<Duration> {
        (self.moves > 0).then(|| self.time.div_f64(self.moves as f64))
    }

    /// The mean number of nodes per move, or `None` if the agent never moved.
    pub fn nodes_per_move(&self) -> Option<f64> {
        (self.moves > 0).then(|| self.nodes as f64 / self.moves as f64)
    }
}

/// `+=` adds up the thinking of an agent in several threads.
impl AddAssign for Thinking {
    fn add_assign(&mut self, other: Thinking) {
        self.moves += other.moves;
        self.time += other.time;
        self.nodes += other.nodes;
    }
}

/// An agent with a stopwatch and a node counter around `choose`.
struct Timed<'a> {
    agent: &'a mut dyn Agent,
    thinking: Thinking,
}

impl Agent for Timed<'_> {
    fn new_game(&mut self, seed: u32) -> Result<(), AgentError> {
        self.agent.new_game(seed)
    }

    fn choose<'m>(
        &mut self,
        board: &Board,
        side: Color,
        legal: &'m [Move],
    ) -> Result<&'m Move, AgentError> {
        let (start, nodes) = (Instant::now(), self.agent.nodes());
        let chosen = self.agent.choose(board, side, legal);
        self.thinking.time += start.elapsed();
        self.thinking.nodes += self.agent.nodes() - nodes;
        self.thinking.moves += 1;
        chosen
    }

    fn nodes(&self) -> u64 {
        self.agent.nodes()
    }
}

/// A game of a match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchGame {
    /// Agent A played White.
    pub a_white: bool,
    pub record: GameRecord,
}

impl MatchGame {
    /// The colour agent A played.
    pub fn a_color(&self) -> Color {
        if self.a_white {
            Color::White
        } else {
            Color::Black
        }
    }

    /// Agent A's score: 1 for a win, ½ for a draw, 0 for a loss.
    pub fn a_score(&self) -> f64 {
        match self.record.winner {
            None => 0.5,
            Some(winner) if winner == self.a_color() => 1.0,
            Some(_) => 0.0,
        }
    }
}

/// A finished match between agents A and B.
#[derive(Clone, Debug)]
pub struct MatchRecord {
    pub games: Vec<MatchGame>,
    pub a: Thinking,
    pub b: Thinking,
}

impl MatchRecord {
    /// A's wins, draws and losses.
    pub fn wins_draws_losses(&self) -> (u32, u32, u32) {
        let mut wdl = (0, 0, 0);
        for game in &self.games {
            match game.record.winner {
                None => wdl.1 += 1,
                Some(winner) if winner == game.a_color() => wdl.0 += 1,
                Some(_) => wdl.2 += 1,
            }
        }
        wdl
    }

    /// A's mean score per game, or `None` without games.
    pub fn score(&self) -> Option<f64> {
        let n = self.games.len();
        (n > 0).then(|| self.games.iter().map(MatchGame::a_score).sum::<f64>() / n as f64)
    }

    /// The standard error of `score`, s / √N with the sample variance s² = Σ(x − x̄)² / (N − 1),
    /// or `None` with fewer than two games.
    pub fn standard_error(&self) -> Option<f64> {
        let n = self.games.len();
        let mean = self.score().filter(|_| n >= 2)?;
        let squares: f64 = self
            .games
            .iter()
            .map(|game| (game.a_score() - mean).powi(2))
            .sum();
        Some((squares / ((n - 1) * n) as f64).sqrt())
    }

    /// The mean number of plies per game, or `None` without games.
    pub fn mean_plies(&self) -> Option<f64> {
        let n = self.games.len();
        let plies: usize = self.games.iter().map(|game| game.record.plies()).sum();
        (n > 0).then(|| plies as f64 / n as f64)
    }

    /// How many games ended for each reason, in the order of `EndReason::ALL`.
    pub fn endings(&self) -> [(EndReason, u32); 5] {
        EndReason::ALL.map(|reason| {
            let n = self
                .games
                .iter()
                .filter(|game| game.record.reason == reason)
                .count();
            (reason, n as u32)
        })
    }
}

/// Builds a fresh agent. Every thread of a match builds its own agents with it, so the factory is
/// shared between threads (`Sync`) while the agents are not.
pub type AgentFactory<'f> = dyn Fn() -> Box<dyn Agent> + Sync + 'f;

/// What a match is: how many games, the seed, the schedule, and how many threads play them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchPlan {
    /// The number of games; even for the paired schedule.
    pub games: u32,
    pub seed: u32,
    pub schedule: Schedule,
    /// The threads that play the games at the same time (at least 1). The games, and so the
    /// result, are the same with any number of threads; the time per move is not.
    pub threads: usize,
}

/// The games one thread played, with its agents' thinking, or the first error it met.
#[derive(Default)]
struct Played {
    games: Vec<(usize, MatchGame)>,
    error: Option<(usize, AgentError)>,
    a: Thinking,
    b: Thinking,
}

/// Plays the games of `plan` between agents A and B from the start position, with the seeds and
/// colours of its schedule (see the module documentation). If an agent fails, the other threads
/// stop after their current game, and the error of the earliest failed game is returned.
///
/// # Panics
/// If the paired schedule is asked for an odd number of games, or for 0 threads.
pub fn play_match(
    a: &AgentFactory,
    b: &AgentFactory,
    plan: &MatchPlan,
) -> Result<MatchRecord, AgentError> {
    assert!(
        plan.schedule != Schedule::Paired || plan.games.is_multiple_of(2),
        "the paired schedule plays every opening twice: an even number of games"
    );
    assert!(plan.threads >= 1, "a match needs at least one thread");
    let seeds = plan.schedule.game_seeds(plan.seed, plan.games);
    let settings = plan.schedule.settings();
    let next = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let threads = plan.threads.min(seeds.len()).max(1);
    // `thread::scope` waits for every thread it spawned before it returns, so the threads may
    // borrow `seeds`, `next` and `failed` from this function.
    let parts: Vec<Played> = thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| scope.spawn(|| play_games(a, b, &seeds, &next, &failed, &settings)))
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().expect("an arena thread panicked"))
            .collect()
    });
    let mut games = Vec::with_capacity(seeds.len());
    let (mut a_thinking, mut b_thinking) = (Thinking::default(), Thinking::default());
    let mut error: Option<(usize, AgentError)> = None;
    for part in parts {
        games.extend(part.games);
        a_thinking += part.a;
        b_thinking += part.b;
        if let Some((g, e)) = part.error
            && error.as_ref().is_none_or(|(first, _)| g < *first)
        {
            error = Some((g, e));
        }
    }
    if let Some((_, e)) = error {
        return Err(e);
    }
    games.sort_by_key(|(g, _)| *g);
    Ok(MatchRecord {
        games: games.into_iter().map(|(_, game)| game).collect(),
        a: a_thinking,
        b: b_thinking,
    })
}

/// One thread of `play_match`: takes the next unplayed game until there is none, or until a game
/// anywhere has failed.
fn play_games(
    a: &AgentFactory,
    b: &AgentFactory,
    seeds: &[GameSeeds],
    next: &AtomicUsize,
    failed: &AtomicBool,
    settings: &MatchSettings,
) -> Played {
    let (mut a_agent, mut b_agent) = (a(), b());
    let mut a = Timed {
        agent: a_agent.as_mut(),
        thinking: Thinking::default(),
    };
    let mut b = Timed {
        agent: b_agent.as_mut(),
        thinking: Thinking::default(),
    };
    let mut played = Played::default();
    while !failed.load(Ordering::Relaxed) {
        // `fetch_add` hands every game index to exactly one thread.
        let g = next.fetch_add(1, Ordering::Relaxed);
        let Some(game_seeds) = seeds.get(g) else {
            break;
        };
        match play_one(&mut a, &mut b, game_seeds, settings) {
            Ok(game) => played.games.push((g, game)),
            Err(e) => {
                failed.store(true, Ordering::Relaxed);
                played.error = Some((g, e));
            }
        }
    }
    played.a = a.thinking;
    played.b = b.thinking;
    played
}

/// One game of a match from the start position, with these seeds and colours.
fn play_one(
    a: &mut dyn Agent,
    b: &mut dyn Agent,
    seeds: &GameSeeds,
    settings: &MatchSettings,
) -> Result<MatchGame, AgentError> {
    let mut opening = Mulberry32::new(seeds.opening);
    a.new_game(seeds.a)?;
    b.new_game(seeds.b)?;
    let (white, black): (&mut dyn Agent, &mut dyn Agent) =
        if seeds.a_white { (a, b) } else { (b, a) };
    let record = play_game(
        Board::start(),
        Color::White,
        white,
        black,
        &mut opening,
        settings,
    )?;
    Ok(MatchGame {
        a_white: seeds.a_white,
        record,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A game of `plies` plies (any moves will do) that A, playing White or not, ended so.
    fn game(a_white: bool, winner: Option<Color>, reason: EndReason, plies: usize) -> MatchGame {
        let some_move = Board::start().legal_moves(Color::White).swap_remove(0);
        MatchGame {
            a_white,
            record: GameRecord {
                winner,
                reason,
                moves: vec![some_move; plies],
            },
        }
    }

    fn record(games: Vec<MatchGame>) -> MatchRecord {
        MatchRecord {
            games,
            a: Thinking::default(),
            b: Thinking::default(),
        }
    }

    #[test]
    fn a_score_follows_the_colour_a_played() {
        let white_wins = Some(Color::White);
        assert_eq!(game(true, white_wins, EndReason::NoMove, 9).a_score(), 1.0);
        assert_eq!(game(false, white_wins, EndReason::NoMove, 9).a_score(), 0.0);
        assert_eq!(game(false, None, EndReason::Cap, 400).a_score(), 0.5);
    }

    #[test]
    fn statistics_of_a_match() {
        let match_ = record(vec![
            game(true, Some(Color::White), EndReason::NoMove, 30),
            game(false, None, EndReason::Repetition, 50),
        ]);
        assert_eq!(match_.wins_draws_losses(), (1, 1, 0));
        assert_eq!(match_.score(), Some(0.75));
        // Scores 1 and ½: s² = (¼² + ¼²) / 1 = ⅛, and s / √2 = √(⅛ / 2) = ¼.
        assert_eq!(match_.standard_error(), Some(0.25));
        assert_eq!(match_.mean_plies(), Some(40.0));
        assert_eq!(
            match_.endings(),
            [
                (EndReason::Repetition, 1),
                (EndReason::NoMove, 1),
                (EndReason::OnePieceEach, 0),
                (EndReason::QuietPlies, 0),
                (EndReason::Cap, 0),
            ]
        );
    }

    #[test]
    fn statistics_need_games() {
        let empty = record(Vec::new());
        assert_eq!(empty.score(), None);
        assert_eq!(empty.standard_error(), None);
        assert_eq!(empty.mean_plies(), None);
        let one = record(vec![game(true, None, EndReason::Cap, 400)]);
        assert_eq!(one.score(), Some(0.5));
        assert_eq!(one.standard_error(), None);
    }

    #[test]
    fn time_per_move() {
        let thinking = Thinking {
            moves: 4,
            time: Duration::from_micros(10),
            nodes: 90,
        };
        assert_eq!(thinking.per_move(), Some(Duration::from_nanos(2500)));
        assert_eq!(thinking.nodes_per_move(), Some(22.5));
        assert_eq!(Thinking::default().per_move(), None);
        assert_eq!(Thinking::default().nodes_per_move(), None);
    }
}
