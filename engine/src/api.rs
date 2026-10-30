//! The engine as the web site sees it (docs/web-api.md): a `Session` holds one game and the agents
//! that play in it, and every call takes and returns JSON. The WebAssembly crate (`web/`) and
//! `dama api` only pass the calls through.
//!
//! A game follows the arena's rules (`arena::game_end`), and an agent set with `set_agent` lives
//! through the game as in an arena game: it is built, reseeded with `new_game(seed)`, observes
//! every position as it is reached, and `think` is its `choose`. So a game played through a
//! session with the arena's seeds and openings is the arena's game, move for move. `undo` keeps
//! what an agent learnt while thinking (its table, heuristics and random numbers) and gives it
//! back the game's history: the agent forgets the game (`Agent::forget_game`) and observes the
//! positions that remain once more.
//!
//! Every call that fails answers `{"error": "..."}` and leaves the session as it was: everything
//! is checked before anything changes. Time budgets (`:tMS`) are refused, since the WebAssembly
//! build has no clock to stop a search with.

use std::sync::OnceLock;
use std::time::Instant;

use crate::agent::{Agent, AgentSpec, Thought};
use crate::arena::{EndReason, GamePosition, MatchSettings, game_end};
use crate::board::{Board, Color, Kind};
use crate::eval::{U_EPSILON, breakdown};
use crate::json::Json;
use crate::movegen::Move;
use crate::trace::{Recorder, RootMove, TraceRequest, Tree};

/// A game drawn at this ply, as in the arena's matches.
const MAX_PLIES: u32 = MatchSettings::PAIRED.max_plies;

/// The trace asked for by `{"trace": {}}` without its fields.
const DEFAULT_TRACE: TraceRequest = TraceRequest {
    iteration: -1,
    max_nodes: 80_000,
};

/// Milliseconds since the first call, from `Instant`: the clock of a native session. The
/// WebAssembly build has no `Instant` and passes its own clock to `Session::new`.
pub fn native_clock() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1e3
}

/// The answer of `version()`: the engine's version and the agent names a session accepts.
pub fn version(engine: &str) -> Json {
    let specs = [
        "random",
        "tuned",
        "first",
        "material",
        "texel1",
        "texel2",
        "texel2s",
        "u:MAN/KING/MOB",
        "mmN",
        "abN[q][r][o][t]",
        "id[q][r][o][t]:nN[:dN]",
        "...[:w=NAME][:KEY=VALUE]",
    ];
    Json::obj(vec![
        ("engine", Json::str(engine)),
        (
            "specs",
            Json::Arr(specs.iter().map(|s| Json::str(s)).collect()),
        ),
    ])
}

/// An agent of the session, with what it was built from.
struct Seat {
    spec: AgentSpec,
    seed: u32,
    agent: Box<dyn Agent>,
}

/// One game and its agents.
pub struct Session {
    clock: fn() -> f64,
    /// The positions of the game, the start first: `positions[k]` is the one after `k` moves.
    positions: Vec<(Board, Color)>,
    moves: Vec<Move>,
    /// `quiet[k]`: the plies in a row without a capture before position `k`.
    quiet: Vec<u32>,
    /// White's agent, then Black's.
    seats: [Option<Seat>; 2],
}

type Answer = Result<Json, String>;

/// The answer as the web API gives it: the value, or `{"error": ...}`.
fn answer(result: Answer) -> Json {
    result.unwrap_or_else(|e| Json::obj(vec![("error", Json::Str(e))]))
}

fn seat_index(side: Color) -> usize {
    match side {
        Color::White => 0,
        Color::Black => 1,
    }
}

fn color_name(side: Color) -> &'static str {
    match side {
        Color::White => "white",
        Color::Black => "black",
    }
}

fn parse_side(side: &str) -> Result<Color, String> {
    match side {
        "white" => Ok(Color::White),
        "black" => Ok(Color::Black),
        other => Err(format!(
            "{other:?} is not a side: expected \"white\" or \"black\""
        )),
    }
}

/// An agent name the session accepts: one `AgentSpec` knows, without a time budget.
fn parse_spec(spec: &str) -> Result<AgentSpec, String> {
    let parsed: AgentSpec = spec.parse().map_err(|e| format!("{e}"))?;
    if let AgentSpec::Deepening { limits, .. } = parsed
        && limits.time.is_some()
    {
        return Err(format!(
            "{spec}: time budgets are not available here; use a node budget such as n20000"
        ));
    }
    Ok(parsed)
}

/// The options object of a call, given as JSON text.
fn parse_options(text: &str) -> Result<Json, String> {
    let options = Json::parse(text)?;
    match options {
        Json::Obj(_) => Ok(options),
        _ => Err("options must be a JSON object".to_string()),
    }
}

/// A whole number from `lo` to `hi` in field `key` of `options`, or `default` if it is missing.
fn whole(options: &Json, key: &str, default: i64, (lo, hi): (i64, i64)) -> Result<i64, String> {
    match options.get(key) {
        None | Some(Json::Null) => Ok(default),
        Some(Json::Num(x)) if x.fract() == 0.0 && *x >= lo as f64 && *x <= hi as f64 => {
            Ok(*x as i64)
        }
        Some(_) => Err(format!("{key} must be a whole number from {lo} to {hi}")),
    }
}

/// The move notations in field `key` of `options`.
fn notations(options: &Json, key: &str) -> Result<Option<Vec<String>>, String> {
    match options.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::Arr(items)) => items
            .iter()
            .map(|m| {
                m.as_str()
                    .map(str::to_string)
                    .ok_or(format!("{key} must be a list of moves"))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Some),
        Some(_) => Err(format!("{key} must be a list of moves")),
    }
}

/// The trace asked for in `options`, if any.
fn trace_request(options: &Json) -> Result<Option<TraceRequest>, String> {
    match options.get("trace") {
        None | Some(Json::Null) | Some(Json::Bool(false)) => Ok(None),
        Some(Json::Bool(true)) => Ok(Some(DEFAULT_TRACE)),
        Some(trace @ Json::Obj(_)) => {
            let iteration = whole(trace, "iteration", -1, (-1, 64))?;
            if iteration == 0 {
                return Err("trace.iteration must be -1 (the last) or a depth from 1".to_string());
            }
            let max_nodes = whole(trace, "max_nodes", 80_000, (1, 10_000_000))?;
            Ok(Some(TraceRequest {
                iteration: iteration as i32,
                max_nodes: max_nodes as usize,
            }))
        }
        Some(_) => Err("trace must be an object".to_string()),
    }
}

/// The legal move written `notation`, if there is one.
fn find_move(legal: &[Move], notation: &str) -> Option<usize> {
    legal.iter().position(|m| m.to_string() == notation)
}

fn notation_list(moves: &[Move]) -> Json {
    Json::Arr(moves.iter().map(|m| Json::Str(m.to_string())).collect())
}

/// A finite number, or `null` for an infinite bound.
fn finite(x: f64) -> Json {
    if x.is_finite() {
        Json::Num(x)
    } else {
        Json::Null
    }
}

fn count(n: u64) -> Json {
    Json::Num(n as f64)
}

/// A move as docs/web-api.md writes it.
fn move_json(mv: &Move) -> Json {
    let squares = |sq: &mut dyn Iterator<Item = crate::square::Square>| {
        Json::Arr(sq.map(|s| Json::Str(s.to_string())).collect())
    };
    Json::obj(vec![
        ("move", Json::Str(mv.to_string())),
        ("from", Json::Str(mv.from().to_string())),
        ("to", Json::Str(mv.to().to_string())),
        ("path", squares(&mut mv.path().iter().copied())),
        ("captured", squares(&mut mv.captured())),
        ("promotes", Json::Bool(mv.promotes())),
    ])
}

fn reason_key(reason: EndReason) -> &'static str {
    match reason {
        EndReason::Repetition => "repetition",
        EndReason::NoMove => "no_moves",
        EndReason::OnePieceEach => "one_each",
        EndReason::QuietPlies => "fifty",
        EndReason::Cap => "ply_cap",
    }
}

impl Session {
    /// The start position with White to move and no agents; `clock` gives milliseconds.
    pub fn new(clock: fn() -> f64) -> Session {
        Session {
            clock,
            positions: vec![(Board::start(), Color::White)],
            moves: Vec::new(),
            quiet: vec![0],
            seats: [None, None],
        }
    }

    /// The current position.
    fn current(&self) -> (Board, Color) {
        *self.positions.last().expect("a game has a position")
    }

    /// How the game has ended, if it has.
    fn end(&self) -> Option<(Option<Color>, EndReason)> {
        let (board, side) = self.current();
        let occurrences = self
            .positions
            .iter()
            .filter(|&&p| p == (board, side))
            .count();
        let legal = board.legal_moves(side);
        let position = GamePosition {
            board: &board,
            side,
            legal: &legal,
            occurrences: occurrences as u32,
            quiet: *self.quiet.last().expect("one count per position"),
            ply: self.moves.len() as u32,
        };
        game_end(&position, MAX_PLIES)
    }

    /// A new agent for `spec` and `seed` that has observed `positions`.
    fn seat(spec: AgentSpec, seed: u32, positions: &[(Board, Color)]) -> Result<Seat, String> {
        let mut agent = spec.build();
        agent.new_game(seed).map_err(|e| e.to_string())?;
        for (board, side) in positions {
            agent.observe(board, *side);
        }
        Ok(Seat { spec, seed, agent })
    }

    /// The seats built again over `positions` (see the module documentation).
    fn reseated(&self, positions: &[(Board, Color)]) -> Result<[Option<Seat>; 2], String> {
        let again = |seat: &Option<Seat>| {
            (seat.as_ref())
                .map(|s| Session::seat(s.spec, s.seed, positions))
                .transpose()
        };
        Ok([again(&self.seats[0])?, again(&self.seats[1])?])
    }

    /// The position `back` plies before the current one: its index in `positions`.
    fn back(&self, options: &Json) -> Result<usize, String> {
        let played = self.moves.len() as i64;
        let back = whole(options, "back", 0, (0, played))
            .map_err(|_| format!("back must be a whole number from 0 to {played}"))?;
        Ok((played - back) as usize)
    }

    /// The position `back` plies before the current one, after the moves of `line`.
    fn line_position(&self, options: &Json) -> Result<(Board, Color), String> {
        let (mut board, mut side) = self.positions[self.back(options)?];
        for notation in notations(options, "line")?.unwrap_or_default() {
            let legal = board.legal_moves(side);
            let i = find_move(&legal, &notation)
                .ok_or(format!("{notation} is not a legal move in the line"))?;
            board = board.apply(&legal[i]);
            side = side.opponent();
        }
        Ok((board, side))
    }

    // The calls of docs/web-api.md.

    /// Starts a game from `board` (docs/reference.md) with `turn` to move. The agents stay, built
    /// again for the new game.
    pub fn reset(&mut self, board: &str, turn: &str) -> Json {
        answer(self.try_reset(board, turn))
    }

    fn try_reset(&mut self, board: &str, turn: &str) -> Answer {
        let board: Board = board.parse().map_err(|e| format!("bad board: {e}"))?;
        let side = parse_side(turn)?;
        let positions = vec![(board, side)];
        self.seats = self.reseated(&positions)?;
        self.positions = positions;
        self.moves.clear();
        self.quiet = vec![0];
        Ok(self.state_json())
    }

    pub fn state(&self) -> Json {
        self.state_json()
    }

    fn state_json(&self) -> Json {
        let (board, side) = self.current();
        let (status, reason) = match self.end() {
            None => (Json::str("playing"), Json::Null),
            Some((winner, reason)) => (
                Json::str(winner.map_or("draw", color_name)),
                Json::str(reason_key(reason)),
            ),
        };
        let occurrences = self
            .positions
            .iter()
            .filter(|&&p| p == (board, side))
            .count();
        let pieces = |c: Color, k: Kind| Json::Num(f64::from(board.count(c, k)));
        Json::obj(vec![
            ("board", Json::Str(board.to_string())),
            ("turn", Json::str(color_name(side))),
            ("ply", count(self.moves.len() as u64)),
            ("moves", notation_list(&self.moves)),
            ("status", status),
            ("reason", reason),
            (
                "quiet_plies",
                count(u64::from(*self.quiet.last().expect("a count"))),
            ),
            ("repetition_count", count(occurrences as u64)),
            (
                "pieces",
                Json::obj(vec![
                    ("white_men", pieces(Color::White, Kind::Man)),
                    ("white_kings", pieces(Color::White, Kind::King)),
                    ("black_men", pieces(Color::Black, Kind::Man)),
                    ("black_kings", pieces(Color::Black, Kind::King)),
                ]),
            ),
        ])
    }

    /// The legal moves of the current position.
    pub fn legal(&self) -> Json {
        let (board, side) = self.current();
        Json::Arr(board.legal_moves(side).iter().map(move_json).collect())
    }

    /// Plays `mv` (our notation), which must be legal, in a game that has not ended.
    pub fn play(&mut self, mv: &str) -> Json {
        answer(self.try_play(mv))
    }

    fn try_play(&mut self, notation: &str) -> Answer {
        if let Some((_, reason)) = self.end() {
            return Err(format!("the game is over ({})", reason_key(reason)));
        }
        let (board, side) = self.current();
        let legal = board.legal_moves(side);
        let i = find_move(&legal, notation).ok_or(format!(
            "{notation} is not a legal move for {}",
            color_name(side)
        ))?;
        let mv = legal[i].clone();
        let after = (board.apply(&mv), side.opponent());
        let quiet = if mv.is_capture() {
            0
        } else {
            self.quiet.last().expect("a count") + 1
        };
        self.positions.push(after);
        self.quiet.push(quiet);
        self.moves.push(mv);
        for seat in self.seats.iter_mut().flatten() {
            seat.agent.observe(&after.0, after.1);
        }
        Ok(self.state_json())
    }

    /// Takes back the last move; the agents observe the game again (module documentation).
    pub fn undo(&mut self) -> Json {
        answer(self.try_undo())
    }

    fn try_undo(&mut self) -> Answer {
        if self.moves.is_empty() {
            return Err("there is no move to take back".to_string());
        }
        self.positions.pop();
        self.quiet.pop();
        self.moves.pop();
        for seat in self.seats.iter_mut().flatten() {
            seat.agent.forget_game();
            for (board, side) in &self.positions {
                seat.agent.observe(board, *side);
            }
        }
        Ok(self.state_json())
    }

    /// Gives `side` the agent `spec`, seeded with `seed`, that has observed the game so far.
    pub fn set_agent(&mut self, side: &str, spec: &str, seed: u32) -> Json {
        answer(self.try_set_agent(side, spec, seed))
    }

    fn try_set_agent(&mut self, side: &str, spec: &str, seed: u32) -> Answer {
        let side = parse_side(side)?;
        let spec = parse_spec(spec)?;
        let seat = Session::seat(spec, seed, &self.positions)?;
        self.seats[seat_index(side)] = Some(seat);
        Ok(Json::obj(vec![
            ("ok", Json::Bool(true)),
            ("spec", Json::Str(spec.to_string())),
        ]))
    }

    /// `side`'s agent decides on the current position: the arena's decision (`Report`).
    pub fn think(&mut self, side: &str, options: &str) -> Json {
        answer(parse_options(options).and_then(|o| self.try_think(side, &o)))
    }

    fn try_think(&mut self, side: &str, options: &Json) -> Answer {
        let side = parse_side(side)?;
        let request = trace_request(options)?;
        if let Some((_, reason)) = self.end() {
            return Err(format!("the game is over ({})", reason_key(reason)));
        }
        let (board, to_move) = self.current();
        if side != to_move {
            return Err(format!("it is {}'s turn", color_name(to_move)));
        }
        let ply = self.moves.len();
        let clock = self.clock;
        let seat = (self.seats[seat_index(side)].as_mut()).ok_or(format!(
            "{} has no agent: call set_agent first",
            color_name(side)
        ))?;
        let legal = board.legal_moves(side);
        let mut recorder = Recorder::new(clock, request);
        let start = clock();
        let thought = (seat.agent)
            .think(&board, side, &legal, &mut recorder, None)
            .map_err(|e| e.to_string())?;
        let ms = clock() - start;
        let spec = seat.spec;
        Ok(report(
            &spec,
            (&board, side, ply),
            &legal,
            &thought,
            &recorder,
            ms,
        ))
    }

    /// A fresh agent `spec` searches the position `back` plies before the current one, having
    /// observed the game up to it; `only` limits its root moves, `seed` seeds it (default 1).
    pub fn analyze(&mut self, spec: &str, options: &str) -> Json {
        answer(parse_options(options).and_then(|o| self.try_analyze(spec, &o)))
    }

    fn try_analyze(&self, spec: &str, options: &Json) -> Answer {
        let spec = parse_spec(spec)?;
        let k = self.back(options)?;
        let seed = whole(options, "seed", 1, (0, i64::from(u32::MAX)))? as u32;
        let request = trace_request(options)?;
        let (board, side) = self.positions[k];
        let legal = board.legal_moves(side);
        if legal.is_empty() {
            return Err(format!("{} has no legal move there", color_name(side)));
        }
        let only = match notations(options, "only")? {
            None => None,
            Some(list) if list.is_empty() => return Err("only is empty".to_string()),
            Some(list) => Some(
                list.iter()
                    .map(|n| find_move(&legal, n).ok_or(format!("{n} is not a legal move there")))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        };
        let mut seat = Session::seat(spec, seed, &self.positions[..=k])?;
        let mut recorder = Recorder::new(self.clock, request);
        let start = (self.clock)();
        let thought = (seat.agent)
            .think(&board, side, &legal, &mut recorder, only.as_deref())
            .map_err(|e| e.to_string())?;
        let ms = (self.clock)() - start;
        Ok(report(
            &spec,
            (&board, side, k),
            &legal,
            &thought,
            &recorder,
            ms,
        ))
    }

    /// The static evaluation with `weights` (a weight set or agent name: `texel1`,
    /// `tuned:cen=0.05`, `idqrot:n20000:w=texel1`) of the position `back` plies before the current
    /// one after the moves of `line`, broken down (`Eval`), from the view of `side` in the options:
    /// by default the side that has just moved there.
    pub fn eval(&self, weights: &str, options: &str) -> Json {
        answer(parse_options(options).and_then(|o| self.try_eval(weights, &o)))
    }

    fn try_eval(&self, weights: &str, options: &Json) -> Answer {
        let w = parse_spec(weights)?
            .weights()
            .ok_or(format!("{weights} has no weights"))?;
        let (board, to_move) = self.line_position(options)?;
        // By default the view of the side that has just moved, as the search scores a leaf.
        let side = match options.get("side") {
            None => to_move.opponent(),
            Some(side) => parse_side(side.as_str().ok_or("side must be \"white\" or \"black\"")?)?,
        };
        let b = breakdown(&board, to_move, side, w);
        let features = b.features.iter().map(|f| {
            Json::obj(vec![
                ("key", Json::str(f.key)),
                ("name", Json::str(f.name)),
                ("white", Json::Num(f64::from(f.white))),
                ("black", Json::Num(f64::from(f.black))),
                ("weight", Json::Num(f.weight)),
                ("contribution", Json::Num(f.contribution)),
            ])
        });
        let squares = b.squares.iter().map(|s| {
            Json::obj(vec![
                ("square", Json::Str(s.square.to_string())),
                ("piece", Json::Str(s.piece.to_char().to_string())),
                ("contribution", Json::Num(s.contribution)),
            ])
        });
        Ok(Json::obj(vec![
            ("weights", Json::str(weights)),
            ("side", Json::str(color_name(side))),
            ("board", Json::Str(board.to_string())),
            ("total", Json::Num(b.total)),
            (
                "win",
                b.win.map_or(Json::Null, |c| Json::str(color_name(c))),
            ),
            ("features", Json::Arr(features.collect())),
            ("squares", Json::Arr(squares.collect())),
        ]))
    }

    /// The position `back` plies before the current one after the moves of `line`, with its
    /// legal moves (`Position`).
    pub fn position(&self, options: &str) -> Json {
        answer(parse_options(options).and_then(|o| self.try_position(&o)))
    }

    fn try_position(&self, options: &Json) -> Answer {
        let (board, side) = self.line_position(options)?;
        Ok(Json::obj(vec![
            ("board", Json::Str(board.to_string())),
            ("turn", Json::str(color_name(side))),
            (
                "legal",
                Json::Arr(board.legal_moves(side).iter().map(move_json).collect()),
            ),
        ]))
    }

    /// The call `name` with `args` (strings, numbers, or an options object), as `dama api` reads
    /// it: `{"call": "think", "args": ["white", "{}"]}`. `engine` is the version string.
    pub fn call(&mut self, name: &str, args: &[Json], engine: &str) -> Json {
        answer(self.try_call(name, args, engine))
    }

    fn try_call(&mut self, name: &str, args: &[Json], engine: &str) -> Answer {
        let text = |k: usize| -> Result<String, String> {
            match args.get(k) {
                Some(Json::Str(s)) => Ok(s.clone()),
                Some(other @ Json::Obj(_)) => Ok(other.to_string()),
                None if matches!(name, "think" | "analyze" | "eval" | "position") => {
                    Ok("{}".to_string())
                }
                _ => Err(format!("{name}: argument {} must be a string", k + 1)),
            }
        };
        let wanted = match name {
            "version" | "new" | "state" | "legal" | "undo" => 0,
            "play" | "position" => 1,
            "reset" | "think" | "analyze" | "eval" => 2,
            "set_agent" => 3,
            _ => return Err(format!("unknown call {name:?}")),
        };
        if args.len() > wanted {
            return Err(format!("{name} takes {wanted} arguments"));
        }
        // An answer that is `{"error": ...}` is passed on as it is.
        Ok(match name {
            "version" => version(engine),
            "new" => {
                *self = Session::new(self.clock);
                self.state_json()
            }
            "state" => self.state(),
            "legal" => self.legal(),
            "undo" => self.undo(),
            "play" => self.play(&text(0)?),
            "position" => self.position(&text(0)?),
            "reset" => self.reset(&text(0)?, &text(1)?),
            "think" => self.think(&text(0)?, &text(1)?),
            "analyze" => self.analyze(&text(0)?, &text(1)?),
            "eval" => self.eval(&text(0)?, &text(1)?),
            _ => {
                let seed = match args.get(2) {
                    Some(Json::Num(x))
                        if x.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(x) =>
                    {
                        *x as u32
                    }
                    _ => {
                        return Err(
                            "set_agent: the seed must be a whole number from 0 to 4294967295"
                                .to_string(),
                        );
                    }
                };
                self.set_agent(&text(0)?, &text(1)?, seed)
            }
        })
    }
}

/// The best root move of an iteration: the first in legal order of those within `U_EPSILON` of
/// the highest value, as `Decision` picks without random numbers.
fn iteration_best(root: &[RootMove]) -> Option<&RootMove> {
    let high = root
        .iter()
        .map(|r| r.value)
        .fold(f64::NEG_INFINITY, f64::max);
    root.iter()
        .filter(|r| high - r.value <= U_EPSILON)
        .min_by_key(|r| r.index)
}

/// The `Report` of a decision by `spec` at `(board, side, ply)`.
fn report(
    spec: &AgentSpec,
    (board, side, ply): (&Board, Color, usize),
    legal: &[Move],
    thought: &Thought,
    recorder: &Recorder,
    ms: f64,
) -> Json {
    let best = &legal[thought.index];
    let decision = thought.decision.as_ref();
    let chosen = decision.and_then(|d| d.candidates.iter().find(|c| c.index == thought.index));
    let last = recorder.iterations.last();
    let pv = last
        .and_then(|it| it.root.iter().find(|r| r.index == thought.index))
        .map_or_else(|| vec![best.clone()], |r| r.pv.clone());
    let ties: Vec<Move> = decision.map_or_else(
        || vec![best.clone()],
        |d| d.top.iter().map(|&t| d.candidates[t].mv.clone()).collect(),
    );
    let iterations = recorder.iterations.iter().map(|it| {
        let top = iteration_best(&it.root);
        Json::obj(vec![
            ("depth", count(u64::from(it.depth))),
            (
                "best",
                top.map_or(Json::Null, |r| Json::Str(r.mv.to_string())),
            ),
            ("value", top.map_or(Json::Null, |r| Json::Num(r.value))),
            ("nodes", count(it.nodes)),
            ("ms", Json::num(it.ms, 3)),
            (
                "pv",
                top.map_or(Json::Arr(Vec::new()), |r| notation_list(&r.pv)),
            ),
        ])
    });
    let root = legal.iter().enumerate().map(|(i, mv)| {
        let found = last.and_then(|it| it.root.iter().find(|r| r.index == i));
        match found {
            Some(r) => Json::obj(vec![
                ("move", Json::Str(mv.to_string())),
                ("value", Json::Num(r.value)),
                ("bound", Json::str(r.bound.key())),
                ("order", count(u64::from(r.order))),
                ("reason", Json::str(r.reason.key())),
                ("nodes", count(r.nodes)),
            ]),
            None => Json::obj(vec![
                ("move", Json::Str(mv.to_string())),
                ("value", Json::Null),
                ("bound", Json::Null),
                ("order", Json::Null),
                ("reason", Json::Null),
                ("nodes", count(0)),
            ]),
        }
    });
    let s = recorder.stats;
    let q_nodes = decision.map_or(0, |d| d.q_nodes);
    let stats = Json::obj(vec![
        ("qnodes", count(q_nodes)),
        ("tt_probes", count(s.tt_probes)),
        ("tt_hits", count(s.tt_hits)),
        ("tt_cutoffs", count(s.tt_cutoffs)),
        ("beta_cutoffs", count(s.beta_cutoffs)),
        ("first_move_cutoffs", count(s.first_move_cutoffs)),
        ("repetition_draws", count(s.repetition_draws)),
    ]);
    let mut fields = vec![
        ("spec", Json::Str(spec.to_string())),
        ("side", Json::str(color_name(side))),
        ("board", Json::Str(board.to_string())),
        ("turn", Json::str(color_name(side))),
        ("ply", count(ply as u64)),
        ("legal", count(legal.len() as u64)),
        ("best", Json::Str(best.to_string())),
        ("value", chosen.map_or(Json::Null, |c| Json::Num(c.u))),
        (
            "bound",
            chosen.map_or(Json::Null, |c| {
                Json::str(if c.exact { "exact" } else { "upper" })
            }),
        ),
        ("depth", count(u64::from(thought.depth))),
        ("nodes", count(decision.map_or(0, |d| d.nodes))),
        ("qnodes", count(q_nodes)),
        ("ms", Json::num(ms, 3)),
        ("pv", notation_list(&pv)),
        ("ties", notation_list(&ties)),
        ("iterations", Json::Arr(iterations.collect())),
        ("root", Json::Arr(root.collect())),
        ("stats", stats),
    ];
    if recorder.requested() {
        fields.push((
            "trace",
            recorder.tree.as_ref().map_or(Json::Null, tree_json),
        ));
    }
    Json::obj(fields)
}

/// A recorded tree as docs/web-api.md writes it.
fn tree_json(tree: &Tree) -> Json {
    let nodes = tree.nodes.iter().enumerate().map(|(id, n)| {
        Json::obj(vec![
            ("id", count(id as u64)),
            ("parent", n.parent.map_or(Json::Null, |p| count(p as u64))),
            (
                "move",
                n.mv.as_ref()
                    .map_or(Json::Null, |m| Json::Str(m.to_string())),
            ),
            ("ply", count(u64::from(n.ply))),
            ("depth", count(u64::from(n.draft))),
            (
                "kind",
                Json::str(if n.quiescence { "quiescence" } else { "main" }),
            ),
            ("alpha", finite(n.window.0)),
            ("beta", finite(n.window.1)),
            ("value", Json::Num(n.value)),
            ("bound", Json::str(n.bound.key())),
            ("static", n.static_value.map_or(Json::Null, Json::Num)),
            (
                "children",
                Json::Arr(n.children.iter().map(|&c| count(c as u64)).collect()),
            ),
            ("legal", count(n.legal as u64)),
            ("order", n.order.map_or(Json::Null, |r| Json::str(r.key()))),
            ("cutoff", Json::Bool(n.cutoff)),
            ("tt", n.tt.map_or(Json::Null, |t| Json::str(t.key()))),
            (
                "draw",
                if n.repetition {
                    Json::str("repetition")
                } else {
                    Json::Null
                },
            ),
            ("sub", count(n.sub)),
            ("in", count(n.visit)),
            ("out", count(n.exit)),
        ])
    });
    Json::obj(vec![
        ("iteration", count(u64::from(tree.iteration))),
        ("nodes_total", count(tree.nodes_total)),
        ("truncated", Json::Bool(tree.truncated)),
        ("nodes", Json::Arr(nodes.collect())),
    ])
}
