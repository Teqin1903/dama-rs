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
//!
//! A match with an SPRT (`MatchPlan::sprt`, paired schedule only) stops as soon as the test decides
//! (`crate::sprt`). The test counts the pairs in the order of the schedule: after each game it takes
//! in every pair whose two games are both over and whose earlier pairs are all counted, so it sees
//! pair 0, 1, 2, ... whatever order the threads finish them in. Once it decides at pair k, the threads
//! stop taking new games, and the record keeps games 0 to 2k + 1 only: the same games, and the same
//! verdict, with any number of threads, and the first games of the fixed match with the same seed.

use std::collections::HashMap;
use std::fmt;
use std::ops::AddAssign;
use std::str::FromStr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::agent::{Agent, AgentError};
use crate::board::{Board, Color};
use crate::movegen::Move;
use crate::rng::{Mulberry32, derive_seed};
use crate::sprt::{Pentanomial, Sprt, SprtResult, Verdict};

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
                    opening: paired_opening_seed(seed, g),
                    a: paired_agent_seeds(seed, g).0,
                    b: paired_agent_seeds(seed, g).1,
                    a_white: g % 2 == 0,
                })
                .collect(),
        }
    }
}

/// The seed of the random opening plies of game `game` in a paired match with seed `seed`: the
/// same for games 2p and 2p + 1.
pub fn paired_opening_seed(seed: u32, game: u32) -> u32 {
    derive_seed(seed, OPENING, game / 2)
}

/// The seeds of agents A and B in game `game` of a paired match with seed `seed`.
pub fn paired_agent_seeds(seed: u32, game: u32) -> (u32, u32) {
    (
        derive_seed(seed, AGENT_A, game),
        derive_seed(seed, AGENT_B, game),
    )
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

/// A position of a game, with what the draw rules need to know about it.
#[derive(Clone, Copy, Debug)]
pub struct GamePosition<'a> {
    pub board: &'a Board,
    pub side: Color,
    /// `board.legal_moves(side)`.
    pub legal: &'a [Move],
    /// How often this position, with this side to move, has occurred in the game, this time
    /// included.
    pub occurrences: u32,
    /// Plies in a row without a capture before it.
    pub quiet: u32,
    /// Plies played before it.
    pub ply: u32,
}

/// How the game ends at `position`, if it does, checked in the order of `playGame`: the third
/// occurrence of a position is a draw; a side without a legal move loses; one piece each is a
/// draw; 50 quiet plies are a draw; reaching `max_plies` is a draw. The winner is `None` for a
/// draw.
pub fn game_end(position: &GamePosition, max_plies: u32) -> Option<(Option<Color>, EndReason)> {
    let board = position.board;
    if position.occurrences >= REPETITIONS {
        return Some((None, EndReason::Repetition));
    }
    if position.legal.is_empty() {
        return Some((Some(position.side.opponent()), EndReason::NoMove));
    }
    if board.pieces(Color::White).len() == 1 && board.pieces(Color::Black).len() == 1 {
        return Some((None, EndReason::OnePieceEach));
    }
    if position.quiet >= QUIET_PLIES {
        return Some((None, EndReason::QuietPlies));
    }
    if position.ply >= max_plies {
        return Some((None, EndReason::Cap));
    }
    None
}

/// Plays a game from `start` with `side` to move: the port of `playGame`.
///
/// Both agents observe every position (`Agent::observe`) as it is reached. Then, before every
/// ply, in the order of agent.js: the third occurrence of a position is a draw; a
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
        white.observe(&board, side);
        black.observe(&board, side);
        let occurrences = seen.entry((board, side)).or_insert(0);
        *occurrences += 1;
        let legal = board.legal_moves(side);
        let position = GamePosition {
            board: &board,
            side,
            legal: &legal,
            occurrences: *occurrences,
            quiet,
            ply,
        };
        if let Some(end) = game_end(&position, settings.max_plies) {
            break end;
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
    /// The depths of the searches that chose them, added up (`Agent::depths`).
    pub depths: u64,
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

    /// The mean depth per move, or `None` if the agent never moved.
    pub fn depth_per_move(&self) -> Option<f64> {
        (self.moves > 0).then(|| self.depths as f64 / self.moves as f64)
    }
}

impl Thinking {
    /// The thinking done since `earlier`, a copy of this one taken before.
    fn since(&self, earlier: Thinking) -> Thinking {
        Thinking {
            moves: self.moves - earlier.moves,
            time: self.time - earlier.time,
            nodes: self.nodes - earlier.nodes,
            depths: self.depths - earlier.depths,
        }
    }
}

/// `+=` adds up the thinking of an agent in several threads.
impl AddAssign for Thinking {
    fn add_assign(&mut self, other: Thinking) {
        self.moves += other.moves;
        self.time += other.time;
        self.nodes += other.nodes;
        self.depths += other.depths;
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

    fn observe(&mut self, board: &Board, side: Color) {
        self.agent.observe(board, side);
    }

    fn forget_game(&mut self) {
        self.agent.forget_game();
    }

    fn choose<'m>(
        &mut self,
        board: &Board,
        side: Color,
        legal: &'m [Move],
    ) -> Result<&'m Move, AgentError> {
        let (start, nodes, depths) = (Instant::now(), self.agent.nodes(), self.agent.depths());
        let chosen = self.agent.choose(board, side, legal);
        self.thinking.time += start.elapsed();
        self.thinking.nodes += self.agent.nodes() - nodes;
        self.thinking.depths += self.agent.depths() - depths;
        self.thinking.moves += 1;
        chosen
    }

    fn nodes(&self) -> u64 {
        self.agent.nodes()
    }

    fn depths(&self) -> u64 {
        self.agent.depths()
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
    /// The games in the order of the schedule; with an SPRT, those up to the pair that decided.
    pub games: Vec<MatchGame>,
    /// The thinking of A and of B in each game of `games`, in the same order. It is kept apart
    /// from the games because the time differs from run to run while the games do not.
    pub per_game: Vec<[Thinking; 2]>,
    /// The thinking of every game played, with an SPRT also of the games that were still being
    /// played when the test decided (at most one per thread).
    pub a: Thinking,
    pub b: Thinking,
    /// The outcome of the SPRT, if the match had one.
    pub sprt: Option<SprtResult>,
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

/// What a match is: how many games, the seed, the schedule, how many threads play them, and
/// whether an SPRT may stop it early.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MatchPlan {
    /// The number of games, or with an SPRT the most games; even for the paired schedule.
    pub games: u32,
    pub seed: u32,
    pub schedule: Schedule,
    /// The threads that play the games at the same time (at least 1). The games, and so the
    /// result, are the same with any number of threads; the time per move is not.
    pub threads: usize,
    /// Stop when this test decides; only with the paired schedule.
    pub sprt: Option<Sprt>,
}

/// The SPRT of a match while it is played, shared by its threads.
struct Monitor {
    sprt: Sprt,
    state: Mutex<MonitorState>,
    /// Set once the test has decided: the threads take no new game.
    stop: AtomicBool,
}

struct MonitorState {
    /// A's score in every game that is over, by game index.
    scores: Vec<Option<f64>>,
    /// The pairs counted so far: pairs 0 to `counted.pairs() - 1`.
    counted: Pentanomial,
    llr: f64,
    verdict: Option<Verdict>,
}

impl Monitor {
    fn new(sprt: Sprt, games: usize) -> Monitor {
        Monitor {
            sprt,
            state: Mutex::new(MonitorState {
                scores: vec![None; games],
                counted: Pentanomial::default(),
                llr: 0.0,
                verdict: None,
            }),
            stop: AtomicBool::new(false),
        }
    }

    /// Game `g` is over with A's score `score`: counts every pair that is now complete, in order,
    /// until the test decides.
    fn record(&self, g: usize, score: f64) {
        // `lock` gives the one thread holding the mutex access to the state; a panic in another
        // thread while holding it would make it an `Err`, and the match is lost anyway then.
        let mut state = self.state.lock().expect("an arena thread panicked");
        if state.verdict.is_some() {
            return;
        }
        state.scores[g] = Some(score);
        loop {
            let p = state.counted.pairs() as usize;
            if 2 * p == state.scores.len() {
                state.verdict = Some(Verdict::Inconclusive);
                break;
            }
            let (Some(first), Some(second)) = (state.scores[2 * p], state.scores[2 * p + 1]) else {
                break;
            };
            state.counted.add(first + second);
            state.llr = self.sprt.llr(&state.counted);
            if let Some(verdict) = self.sprt.verdict(state.llr, state.counted.pairs()) {
                state.verdict = Some(verdict);
                break;
            }
        }
        if state.verdict.is_some() {
            self.stop.store(true, Ordering::Relaxed);
        }
    }

    /// The outcome; the match must be over.
    fn result(self) -> SprtResult {
        let state = self.state.into_inner().expect("an arena thread panicked");
        SprtResult {
            sprt: self.sprt,
            verdict: state
                .verdict
                .expect("every game was played or the test decided"),
            llr: state.llr,
            pairs: state.counted,
        }
    }
}

/// The games one thread played, with its agents' thinking, or the first error it met.
#[derive(Default)]
struct Played {
    games: Vec<(usize, MatchGame, [Thinking; 2])>,
    error: Option<(usize, AgentError)>,
    a: Thinking,
    b: Thinking,
}

/// Plays the games of `plan` between agents A and B from the start position, with the seeds and
/// colours of its schedule (see the module documentation). If an agent fails, the other threads
/// stop after their current game, and the error of the earliest failed game is returned.
///
/// # Panics
/// If the paired schedule is asked for an odd number of games, an SPRT with the legacy
/// schedule, or 0 threads.
pub fn play_match(
    a: &AgentFactory,
    b: &AgentFactory,
    plan: &MatchPlan,
) -> Result<MatchRecord, AgentError> {
    assert!(
        plan.schedule != Schedule::Paired || plan.games.is_multiple_of(2),
        "the paired schedule plays every opening twice: an even number of games"
    );
    assert!(
        plan.sprt.is_none() || plan.schedule == Schedule::Paired,
        "an SPRT counts pairs of games: it needs the paired schedule"
    );
    assert!(plan.threads >= 1, "a match needs at least one thread");
    let seeds = plan.schedule.game_seeds(plan.seed, plan.games);
    let settings = plan.schedule.settings();
    let next = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let monitor = plan.sprt.map(|sprt| Monitor::new(sprt, seeds.len()));
    let threads = plan.threads.min(seeds.len()).max(1);
    // `thread::scope` waits for every thread it spawned before it returns, so the threads may
    // borrow `seeds`, `next` and `failed` from this function.
    let parts: Vec<Played> = thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope
                    .spawn(|| play_games(a, b, &seeds, &next, &failed, monitor.as_ref(), &settings))
            })
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
    games.sort_by_key(|(g, _, _)| *g);
    let sprt = monitor.map(Monitor::result);
    if let Some(result) = &sprt {
        // The games after the pair that decided do not count; some may not even be there.
        games.truncate(2 * result.pairs.pairs() as usize);
    }
    // `unzip` splits an iterator of pairs into two collections.
    let (games, per_game) = games
        .into_iter()
        .map(|(_, game, thinking)| (game, thinking))
        .unzip();
    Ok(MatchRecord {
        games,
        per_game,
        a: a_thinking,
        b: b_thinking,
        sprt,
    })
}

/// One thread of `play_match`: takes the next unplayed game until there is none, until a game
/// anywhere has failed, or until the SPRT has decided.
fn play_games(
    a: &AgentFactory,
    b: &AgentFactory,
    seeds: &[GameSeeds],
    next: &AtomicUsize,
    failed: &AtomicBool,
    monitor: Option<&Monitor>,
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
    let decided = || monitor.is_some_and(|m| m.stop.load(Ordering::Relaxed));
    while !failed.load(Ordering::Relaxed) && !decided() {
        // `fetch_add` hands every game index to exactly one thread.
        let g = next.fetch_add(1, Ordering::Relaxed);
        let Some(game_seeds) = seeds.get(g) else {
            break;
        };
        let (a_before, b_before) = (a.thinking, b.thinking);
        match play_one(&mut a, &mut b, game_seeds, settings) {
            Ok(game) => {
                if let Some(monitor) = monitor {
                    monitor.record(g, game.a_score());
                }
                let thinking = [a.thinking.since(a_before), b.thinking.since(b_before)];
                played.games.push((g, game, thinking));
            }
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
            per_game: vec![[Thinking::default(); 2]; games.len()],
            games,
            a: Thinking::default(),
            b: Thinking::default(),
            sprt: None,
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
            depths: 10,
        };
        assert_eq!(thinking.per_move(), Some(Duration::from_nanos(2500)));
        assert_eq!(thinking.nodes_per_move(), Some(22.5));
        assert_eq!(thinking.depth_per_move(), Some(2.5));
        assert_eq!(Thinking::default().depth_per_move(), None);
        assert_eq!(Thinking::default().per_move(), None);
        assert_eq!(Thinking::default().nodes_per_move(), None);
    }
}
