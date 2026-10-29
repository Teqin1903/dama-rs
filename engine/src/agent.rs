//! The agents that play in the arena: the random player, the baseline agent (`randomPolicy` and
//! `agentPolicy` in reference/js/agent.js), the minimax agent, the alpha-beta agent and the
//! iterative-deepening agent, each of the last two also with repetitions recognised.
//!
//! The arena talks to an agent only through the `Agent` trait: a game starts, and a move is
//! chosen from the legal moves of a position. That is also all the text protocol (position / go /
//! bestmove) carries, so an external program can later become one more `Agent` without any change
//! to the arena.

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use crate::board::{Board, Color};
use crate::eval::Weights;
use crate::movegen::Move;
use crate::rng::Mulberry32;
use crate::search::{
    Decision, Limits, MAX_ID_DEPTH, SearchOptions, alpha_beta_with, decide,
    iterative_deepening_with, minimax,
};
#[cfg(test)]
use crate::search::{alpha_beta, iterative_deepening};
use crate::table::{Table, TableOptions};

/// A player in the arena.
///
/// Built-in agents never fail, but an external program can (it exits, or answers with a move that
/// is not legal), so both calls return a `Result`.
pub trait Agent {
    /// A game starts. Built-in agents reseed their random numbers with `seed`; an agent without
    /// random numbers has nothing to do.
    fn new_game(&mut self, _seed: u32) -> Result<(), AgentError> {
        Ok(())
    }

    /// The game has reached `board` with `side` to move. The arena calls this for every position
    /// of a game, the start and the random opening plies included, before it asks for a move, so
    /// an agent that remembers the game can recognise a repetition. Most agents ignore it.
    fn observe(&mut self, _board: &Board, _side: Color) {}

    /// The move to play on `board` with `side` to move. `legal` is `board.legal_moves(side)` and
    /// is never empty; the move returned is one of its elements.
    fn choose<'m>(
        &mut self,
        board: &Board,
        side: Color,
        legal: &'m [Move],
    ) -> Result<&'m Move, AgentError>;

    /// The positions this agent's search has produced since it was built, for the arena's nodes
    /// per move. 0 for an agent that does not search.
    fn nodes(&self) -> u64 {
        0
    }
}

/// Why an agent could not play, in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentError(pub String);

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for AgentError {}

/// The positions of the current game since its last capture, for an agent that recognises
/// repetitions. A position from before a capture cannot occur again (it has more pieces), so
/// those are dropped: the search compares against fewer positions.
#[derive(Clone, Debug, Default)]
struct GameHistory {
    positions: Vec<(Board, Color)>,
}

impl GameHistory {
    fn clear(&mut self) {
        self.positions.clear();
    }

    fn push(&mut self, board: &Board, side: Color) {
        let pieces = |b: &Board| b.pieces(Color::White).len() + b.pieces(Color::Black).len();
        if let Some((last, _)) = self.positions.last()
            && pieces(board) < pieces(last)
        {
            self.positions.clear();
        }
        self.positions.push((*board, side));
    }
}

/// Plays a random legal move: `randomPolicy` in agent.js.
#[derive(Clone, Debug)]
pub struct RandomAgent {
    rng: Mulberry32,
}

impl RandomAgent {
    pub fn new(seed: u32) -> RandomAgent {
        RandomAgent {
            rng: Mulberry32::new(seed),
        }
    }
}

impl Agent for RandomAgent {
    fn new_game(&mut self, seed: u32) -> Result<(), AgentError> {
        self.rng = Mulberry32::new(seed);
        Ok(())
    }

    /// `ms[Math.floor(rnd() * ms.length)]`.
    fn choose<'m>(
        &mut self,
        _board: &Board,
        _side: Color,
        legal: &'m [Move],
    ) -> Result<&'m Move, AgentError> {
        Ok(&legal[self.rng.next_index(legal.len())])
    }
}

/// The baseline agent: a legal move with the highest U one ply deep, ties broken at random as
/// agent.js breaks them (`agentPolicy(w, rnd)`).
#[derive(Clone, Debug)]
pub struct UAgent {
    weights: Weights,
    rng: Mulberry32,
    nodes: u64,
}

impl UAgent {
    pub fn new(weights: Weights, seed: u32) -> UAgent {
        UAgent {
            weights,
            rng: Mulberry32::new(seed),
            nodes: 0,
        }
    }
}

/// The legal move that `decision` picks with `rng`. The candidates are the legal moves in the
/// order of `legal_moves`, so the index of the chosen candidate is its index in `legal` too.
fn play<'m>(decision: &Decision, rng: &mut Mulberry32, legal: &'m [Move]) -> &'m Move {
    let i = decision.pick(Some(rng)).expect("`legal` is not empty");
    debug_assert_eq!(legal[i], decision.candidates[i].mv);
    &legal[i]
}

impl Agent for UAgent {
    fn new_game(&mut self, seed: u32) -> Result<(), AgentError> {
        self.rng = Mulberry32::new(seed);
        Ok(())
    }

    fn choose<'m>(
        &mut self,
        board: &Board,
        side: Color,
        legal: &'m [Move],
    ) -> Result<&'m Move, AgentError> {
        let decision = decide(board, side, self.weights);
        self.nodes += decision.nodes;
        Ok(play(&decision, &mut self.rng, legal))
    }

    fn nodes(&self) -> u64 {
        self.nodes
    }
}

/// Minimax `depth` plies deep with U at the leaves (`search::minimax`), ties broken at random as
/// the baseline agent breaks them.
#[derive(Clone, Debug)]
pub struct MinimaxAgent {
    weights: Weights,
    depth: u32,
    rng: Mulberry32,
    nodes: u64,
}

impl MinimaxAgent {
    pub fn new(weights: Weights, depth: u32, seed: u32) -> MinimaxAgent {
        MinimaxAgent {
            weights,
            depth,
            rng: Mulberry32::new(seed),
            nodes: 0,
        }
    }
}

impl Agent for MinimaxAgent {
    fn new_game(&mut self, seed: u32) -> Result<(), AgentError> {
        self.rng = Mulberry32::new(seed);
        Ok(())
    }

    fn choose<'m>(
        &mut self,
        board: &Board,
        side: Color,
        legal: &'m [Move],
    ) -> Result<&'m Move, AgentError> {
        let decision = minimax(board, side, self.weights, self.depth);
        self.nodes += decision.nodes;
        Ok(play(&decision, &mut self.rng, legal))
    }

    fn nodes(&self) -> u64 {
        self.nodes
    }
}

/// Alpha-beta `depth` plies deep (`search::alpha_beta`): the same moves as `MinimaxAgent` with
/// the same depth and seed, from fewer nodes. With `quiescence`, captures are searched past the
/// depth limit (`search::alpha_beta_q`). With `with_repetitions`, a repetition of a position of the
/// game or of the search path is a draw (`search::alpha_beta_rep`). With `with_ordering`, the
/// moves are ordered: the same moves from fewer nodes (`search::alpha_beta_with`).
#[derive(Clone, Debug)]
pub struct AlphaBetaAgent {
    weights: Weights,
    depth: u32,
    quiescence: bool,
    order: bool,
    /// `Some` with a transposition table.
    table: Option<Table>,
    /// `Some` when repetitions are recognised: the game so far.
    history: Option<GameHistory>,
    rng: Mulberry32,
    nodes: u64,
}

impl AlphaBetaAgent {
    pub fn new(weights: Weights, depth: u32, quiescence: bool, seed: u32) -> AlphaBetaAgent {
        AlphaBetaAgent {
            weights,
            depth,
            quiescence,
            order: false,
            table: None,
            history: None,
            rng: Mulberry32::new(seed),
            nodes: 0,
        }
    }

    /// The same agent, recognising repetitions if `on`.
    pub fn with_repetitions(mut self, on: bool) -> AlphaBetaAgent {
        self.history = on.then(GameHistory::default);
        self
    }

    /// The same agent, ordering its moves if `on`.
    pub fn with_ordering(mut self, on: bool) -> AlphaBetaAgent {
        self.order = on;
        self
    }

    /// The same agent, with a transposition table of these options if `Some` (cleared at the start
    /// of every game).
    pub fn with_table(mut self, options: Option<TableOptions>) -> AlphaBetaAgent {
        self.table = options.map(Table::new);
        self
    }
}

impl Agent for AlphaBetaAgent {
    fn new_game(&mut self, seed: u32) -> Result<(), AgentError> {
        self.rng = Mulberry32::new(seed);
        if let Some(history) = &mut self.history {
            history.clear();
        }
        if let Some(table) = &mut self.table {
            table.clear();
        }
        Ok(())
    }

    fn choose<'m>(
        &mut self,
        board: &Board,
        side: Color,
        legal: &'m [Move],
    ) -> Result<&'m Move, AgentError> {
        let options = SearchOptions {
            quiescence: self.quiescence,
            order: self.order,
        };
        let history = self.history.as_ref().map(|h| h.positions.as_slice());
        let decision = alpha_beta_with(
            board,
            side,
            self.weights,
            self.depth,
            options,
            history,
            self.table.as_mut(),
        );
        self.nodes += decision.nodes;
        Ok(play(&decision, &mut self.rng, legal))
    }

    fn observe(&mut self, board: &Board, side: Color) {
        if let Some(history) = &mut self.history {
            history.push(board, side);
        }
    }

    fn nodes(&self) -> u64 {
        self.nodes
    }
}

/// Iterative deepening (`search::iterative_deepening`): alpha-beta 1, 2, 3, ... plies deep within
/// `limits`, the move of the deepest completed iteration, with or without quiescence. With only a
/// depth limit N it plays the moves of `AlphaBetaAgent` at depth N. With `with_repetitions`, a
/// repetition is a draw (`search::iterative_deepening_rep`). With `with_ordering`, the moves are
/// ordered (`search::iterative_deepening_with`): with only a depth limit the same moves from fewer
/// nodes, with a node or time budget deeper iterations.
#[derive(Clone, Debug)]
pub struct DeepeningAgent {
    weights: Weights,
    limits: Limits,
    quiescence: bool,
    order: bool,
    /// `Some` with a transposition table.
    table: Option<Table>,
    /// `Some` when repetitions are recognised: the game so far.
    history: Option<GameHistory>,
    rng: Mulberry32,
    nodes: u64,
}

impl DeepeningAgent {
    pub fn new(weights: Weights, limits: Limits, quiescence: bool, seed: u32) -> DeepeningAgent {
        DeepeningAgent {
            weights,
            limits,
            quiescence,
            order: false,
            table: None,
            history: None,
            rng: Mulberry32::new(seed),
            nodes: 0,
        }
    }

    /// The same agent, recognising repetitions if `on`.
    pub fn with_repetitions(mut self, on: bool) -> DeepeningAgent {
        self.history = on.then(GameHistory::default);
        self
    }

    /// The same agent, ordering its moves if `on`.
    pub fn with_ordering(mut self, on: bool) -> DeepeningAgent {
        self.order = on;
        self
    }

    /// The same agent, with a transposition table of these options if `Some` (cleared at the start
    /// of every game).
    pub fn with_table(mut self, options: Option<TableOptions>) -> DeepeningAgent {
        self.table = options.map(Table::new);
        self
    }
}

impl Agent for DeepeningAgent {
    fn new_game(&mut self, seed: u32) -> Result<(), AgentError> {
        self.rng = Mulberry32::new(seed);
        if let Some(history) = &mut self.history {
            history.clear();
        }
        if let Some(table) = &mut self.table {
            table.clear();
        }
        Ok(())
    }

    fn choose<'m>(
        &mut self,
        board: &Board,
        side: Color,
        legal: &'m [Move],
    ) -> Result<&'m Move, AgentError> {
        let options = SearchOptions {
            quiescence: self.quiescence,
            order: self.order,
        };
        let history = self.history.as_ref().map(|h| h.positions.as_slice());
        let deepening = iterative_deepening_with(
            board,
            side,
            self.weights,
            self.limits,
            options,
            history,
            self.table.as_mut(),
        );
        self.nodes += deepening.decision.nodes;
        Ok(play(&deepening.decision, &mut self.rng, legal))
    }

    fn observe(&mut self, board: &Board, side: Color) {
        if let Some(history) = &mut self.history {
            history.push(board, side);
        }
    }

    fn nodes(&self) -> u64 {
        self.nodes
    }
}

/// An agent as the command line names it: `random`, a preset name (`material`, `first`,
/// `tuned`, `texel1`, `texel2`, `texel2s`), `u:MAN/KING/MOB` for the baseline agent with any weights, `mmN` for minimax N
/// plies deep, `abN` for alpha-beta N plies deep, or `id:LIMITS` for iterative deepening, the
/// searches with the tuned weights. LIMITS are one or more of `dN`
/// (at most N plies), `nK` (at most K nodes per move) and `tMS` (at most MS milliseconds per move),
/// separated by `:`, for example `id:n20000` or `id:d5:t50`. Letters after `abN` and `id` add to
/// the search, each at most once and in this order: `q` quiescence (`abNq`, `idq:LIMITS`), `r`
/// repetitions (`abNr`, `abNqr`, `idqr:LIMITS`), `o` move ordering (`abNo`, `idqro:LIMITS`), `t` a
/// transposition table (`abNot`, `idqrot:LIMITS`) of 16 MB unless a part `:hash=MB` says
/// otherwise, in the referee's strict mode with a part `:tt=strict`.
///
/// Any name but `random` may end in `:KEY=VALUE` parts that set weights (`Weights::KEYS`), on top
/// of the tuned weights for a search and of the named weights for the baseline agent:
/// `idqr:n2000:adv=0.1:qmob=0.02`, `ab3q:king=3`, `tuned:cen=0.05`. The first part may be
/// `w=NAME` to start from the named set `NAME` (`Weights::NAMED`) instead: `idqr:n2000:w=material`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AgentSpec {
    Random,
    /// The baseline agent with these weights.
    U(Weights),
    /// Minimax `depth` plies deep, 1 to `MAX_DEPTH`.
    Minimax {
        depth: u32,
        weights: Weights,
    },
    /// Alpha-beta `depth` plies deep, 1 to `MAX_DEPTH`.
    AlphaBeta {
        depth: u32,
        quiescence: bool,
        repetition: bool,
        order: bool,
        table: Option<TableOptions>,
        weights: Weights,
    },
    /// Iterative deepening within `limits`.
    Deepening {
        limits: Limits,
        quiescence: bool,
        repetition: bool,
        order: bool,
        table: Option<TableOptions>,
        weights: Weights,
    },
}

impl AgentSpec {
    /// The weights that have a name.
    pub const PRESETS: [(&'static str, Weights); Weights::NAMED.len()] = Weights::NAMED;

    /// The deepest search (`mmN`, `abN`) the command line accepts: plain minimax grows about
    /// tenfold per ply.
    pub const MAX_DEPTH: u32 = 9;

    /// The name of the preset with these weights, if there is one.
    pub fn preset(&self) -> Option<&'static str> {
        match self {
            AgentSpec::Random
            | AgentSpec::Minimax { .. }
            | AgentSpec::AlphaBeta { .. }
            | AgentSpec::Deepening { .. } => None,
            AgentSpec::U(weights) => AgentSpec::PRESETS
                .iter()
                .find(|(_, w)| w == weights)
                .map(|(name, _)| *name),
        }
    }

    /// The weights of U this agent plays with; none for `random`.
    pub fn weights(&self) -> Option<Weights> {
        match *self {
            AgentSpec::Random => None,
            AgentSpec::U(weights)
            | AgentSpec::Minimax { weights, .. }
            | AgentSpec::AlphaBeta { weights, .. }
            | AgentSpec::Deepening { weights, .. } => Some(weights),
        }
    }

    /// The table's options, to change them; none without a table.
    fn table_mut(&mut self) -> Option<&mut TableOptions> {
        match self {
            AgentSpec::AlphaBeta { table, .. } | AgentSpec::Deepening { table, .. } => {
                table.as_mut()
            }
            _ => None,
        }
    }

    /// The weights, to change them; none for `random`.
    fn weights_mut(&mut self) -> Option<&mut Weights> {
        match self {
            AgentSpec::Random => None,
            AgentSpec::U(weights)
            | AgentSpec::Minimax { weights, .. }
            | AgentSpec::AlphaBeta { weights, .. }
            | AgentSpec::Deepening { weights, .. } => Some(weights),
        }
    }

    /// A new agent of this kind. The arena reseeds it before every game.
    pub fn build(&self) -> Box<dyn Agent> {
        match *self {
            AgentSpec::Random => Box::new(RandomAgent::new(0)),
            AgentSpec::U(weights) => Box::new(UAgent::new(weights, 0)),
            AgentSpec::Minimax { depth, weights } => Box::new(MinimaxAgent::new(weights, depth, 0)),
            AgentSpec::AlphaBeta {
                depth,
                quiescence,
                repetition,
                order,
                table,
                weights,
            } => Box::new(
                AlphaBetaAgent::new(weights, depth, quiescence, 0)
                    .with_repetitions(repetition)
                    .with_ordering(order)
                    .with_table(table),
            ),
            AgentSpec::Deepening {
                limits,
                quiescence,
                repetition,
                order,
                table,
                weights,
            } => Box::new(
                DeepeningAgent::new(weights, limits, quiescence, 0)
                    .with_repetitions(repetition)
                    .with_ordering(order)
                    .with_table(table),
            ),
        }
    }
}

impl fmt::Display for AgentSpec {
    /// The canonical name: `random`, `mmN`, `abN` and `id:` with their letters in the order `q`,
    /// `r`, `o`, `t` (`abNqr`, `idqrot:`) and the limits in the order d, n, t, the preset's name, or else `u:MAN/KING/MOB`.
    /// `"u:1/3/0".parse()` therefore prints as `material`. Then the weights as `:KEY=VALUE` in the
    /// order of `Weights::KEYS`: for a search those that differ from the tuned weights or, when
    /// that takes fewer parts, `:w=NAME` and those that differ from that named set
    /// (`Weights::suffix`), for the baseline agent (unless it is a preset) the positional weights
    /// that are not 0. A weight set to its default is left out: `idqr:n2000:adv=0` prints as
    /// `idqr:n2000`. Last the table's options that are not the default: `:hash=MB`, `:tt=strict`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self, self.preset()) {
            (AgentSpec::Random, _) => f.write_str("random"),
            (AgentSpec::U(_), Some(name)) => f.write_str(name),
            (AgentSpec::U(weights), None) => {
                let u_only = Weights {
                    man: weights.man,
                    king: weights.king,
                    mob: weights.mob,
                    ..Weights::ZERO
                };
                write!(f, "u:{weights}{}", weights.overrides_from(&u_only))
            }
            (AgentSpec::Minimax { depth, weights }, _) => {
                write!(f, "mm{depth}{}", weights.suffix())
            }
            (
                AgentSpec::AlphaBeta {
                    depth,
                    quiescence,
                    repetition,
                    order,
                    table,
                    weights,
                },
                _,
            ) => write!(
                f,
                "ab{depth}{}{}{}",
                letters(*quiescence, *repetition, *order, table.is_some()),
                weights.suffix(),
                table_suffix(table)
            ),
            (
                AgentSpec::Deepening {
                    limits,
                    quiescence,
                    repetition,
                    order,
                    table,
                    weights,
                },
                _,
            ) => {
                write!(
                    f,
                    "id{}",
                    letters(*quiescence, *repetition, *order, table.is_some())
                )?;
                if let Some(depth) = limits.depth {
                    write!(f, ":d{depth}")?;
                }
                if let Some(nodes) = limits.nodes {
                    write!(f, ":n{nodes}")?;
                }
                if let Some(time) = limits.time {
                    write!(f, ":t{}", time.as_millis())?;
                }
                write!(f, "{}{}", weights.suffix(), table_suffix(table))
            }
        }
    }
}

/// The letters of `ab` and `id` names: `q` for quiescence, `r` for repetitions, `o` for move
/// ordering, `t` for a transposition table, in that order.
fn letters(quiescence: bool, repetition: bool, order: bool, table: bool) -> String {
    [
        (quiescence, 'q'),
        (repetition, 'r'),
        (order, 'o'),
        (table, 't'),
    ]
    .into_iter()
    .filter_map(|(on, letter)| on.then_some(letter))
    .collect()
}

/// The table's parts of a name: `:hash=MB` unless the default, `:tt=strict` in strict mode.
fn table_suffix(table: &Option<TableOptions>) -> String {
    let mut suffix = String::new();
    if let Some(options) = table {
        if options.mb != TableOptions::DEFAULT_MB {
            suffix.push_str(&format!(":hash={}", options.mb));
        }
        if options.strict {
            suffix.push_str(":tt=strict");
        }
    }
    suffix
}

/// The search options of `letters` (see `letters`): quiescence, repetitions, ordering, table;
/// `None` unless each of `q`, `r`, `o`, `t` appears at most once and in that order, with nothing
/// else.
fn parse_letters(letters: &str) -> Option<(bool, bool, bool, bool)> {
    let mut rest = letters;
    let mut take = |letter: char| match rest.strip_prefix(letter) {
        Some(after) => {
            rest = after;
            true
        }
        None => false,
    };
    let options = (take('q'), take('r'), take('o'), take('t'));
    rest.is_empty().then_some(options)
}

/// Why a name is not an agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseAgentError {
    /// Neither `random`, a preset name, `u:...`, `mmN`, `abN` nor `id:...` (`abN` and `id` with
    /// their letters `q`, `r`, `o`).
    Unknown(String),
    /// `mm` or `ab` is not followed by a depth from 1 to `AgentSpec::MAX_DEPTH` (and for `ab`, its
    /// optional letters `q`, `r`, `o`, in that order).
    Depth(String),
    /// `u:` is not followed by three finite numbers separated by `/`.
    Weights(String),
    /// `id` with its letters is not followed by one to three different limits `:dN`, `:nK`, `:tMS`, each a
    /// positive whole number (N at most `MAX_ID_DEPTH`).
    Limits(String),
    /// The `:KEY=VALUE` parts at the end of the name (the first string) do not set weights; the
    /// second string says why.
    Weight(String, String),
    /// The `:hash=MB` and `:tt=strict` parts of the name (the first string) do not set a table;
    /// the second string says why.
    Table(String, String),
}

impl fmt::Display for ParseAgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseAgentError::Unknown(name) => write!(
                f,
                "unknown agent {name:?}: expected random, material, first, tuned, texel1, texel2, texel2s, \
                 u:MAN/KING/MOB, \
                 mmN, abN, id:LIMITS, the last two with letters q (quiescence), r (repetitions) \
                 o (move ordering) and t (transposition table) in that order: abNqrot, \
                 idqrot:LIMITS"
            ),
            ParseAgentError::Depth(name) => write!(
                f,
                "agent {name:?}: expected a depth from 1 to {} after mm or ab, for example mm3, ab5, \
                 ab3q, ab3qr or ab3qro",
                AgentSpec::MAX_DEPTH
            ),
            ParseAgentError::Weights(name) => write!(
                f,
                "agent {name:?}: expected three numbers after u:, separated by '/', \
                 for example u:1/2.5/-0.05"
            ),
            ParseAgentError::Limits(name) => write!(
                f,
                "agent {name:?}: expected limits after id (or idq, idqro, ...), separated by ':': dN (plies, 1 to \
                 {MAX_ID_DEPTH}), nK (nodes per move), tMS (milliseconds per move), \
                 for example id:n20000 or id:d5:t50"
            ),
            ParseAgentError::Weight(name, why) => write!(
                f,
                "agent {name:?}: {why}; weights are set with :KEY=VALUE at the end of the name, \
                 KEY one of {} (each once) and VALUE a finite number, for example \
                 idqr:n2000:adv=0.1; a first part w=NAME starts from a named weight set",
                Weights::KEYS.join(", ")
            ),
            ParseAgentError::Table(name, why) => write!(
                f,
                "agent {name:?}: {why}; an ab or id agent with the letter t has a transposition \
                 table, sized with :hash=MB (default {} MB) and made strict with :tt=strict, for \
                 example idqrot:n20000:hash=64",
                TableOptions::DEFAULT_MB
            ),
        }
    }
}

impl std::error::Error for ParseAgentError {}

impl FromStr for AgentSpec {
    type Err = ParseAgentError;

    /// The name without its `:KEY=VALUE` parts is read by `parse_base`; the parts then set weights.
    fn from_str(s: &str) -> Result<AgentSpec, ParseAgentError> {
        let parts: Vec<&str> = s.split(':').collect();
        // The weights are the parts after the last one without '='.
        let first_weight = parts
            .iter()
            .rposition(|part| !part.contains('='))
            .map_or(0, |i| i + 1);
        if first_weight == parts.len() {
            return parse_base(s);
        }
        if first_weight == 0 {
            return Err(ParseAgentError::Unknown(s.to_string()));
        }
        let mut spec = parse_base(&parts[..first_weight].join(":"))?;
        // `hash` and `tt` set the table; every other part sets a weight.
        let (table_parts, weight_parts): (Vec<&str>, Vec<&str>) = parts[first_weight..]
            .iter()
            .partition(|part| part.starts_with("hash=") || part.starts_with("tt="));
        if !table_parts.is_empty() {
            let table_error = |why: &str| ParseAgentError::Table(s.to_string(), why.to_string());
            let table = spec
                .table_mut()
                .ok_or_else(|| table_error("hash and tt need a table, the letter t"))?;
            let (mut mb, mut strict) = (None, None);
            for part in table_parts {
                match part.split_once('=') {
                    Some(("hash", value)) if mb.is_none() => {
                        mb = Some(
                            value
                                .parse::<u32>()
                                .ok()
                                .filter(|n| (1..=TableOptions::MAX_MB).contains(n))
                                .filter(|_| value.bytes().all(|b| b.is_ascii_digit()))
                                .ok_or_else(|| {
                                    table_error("hash is a whole number of megabytes, 1 to 65536")
                                })?,
                        )
                    }
                    Some(("tt", "strict")) if strict.is_none() => strict = Some(true),
                    Some(("tt", _)) if strict.is_none() => {
                        return Err(table_error("tt=strict is the only table mode to name"));
                    }
                    _ => return Err(table_error("hash and tt may each be given once")),
                }
            }
            *table = TableOptions {
                mb: mb.unwrap_or(table.mb),
                strict: strict.unwrap_or(table.strict),
            };
        }
        if !weight_parts.is_empty() {
            let weight_error = |why: String| ParseAgentError::Weight(s.to_string(), why);
            let weights = spec
                .weights_mut()
                .ok_or_else(|| weight_error("random has no weights".to_string()))?;
            *weights = weights
                .with_overrides(&weight_parts.join(":"))
                .map_err(weight_error)?;
        }
        Ok(spec)
    }
}

/// An agent name without weights.
fn parse_base(s: &str) -> Result<AgentSpec, ParseAgentError> {
    if s == "random" {
        return Ok(AgentSpec::Random);
    }
    if let Some((_, weights)) = AgentSpec::PRESETS.iter().find(|(name, _)| *name == s) {
        return Ok(AgentSpec::U(*weights));
    }
    // "id", its letters up to the first ':', then the limits.
    let deepening = s.strip_prefix("id").and_then(|rest| {
        let (letters, limits) = rest.split_at(rest.find(':').unwrap_or(rest.len()));
        Some((parse_letters(letters)?, limits))
    });
    if let Some(((quiescence, repetition, order, table), limits)) = deepening {
        return parse_limits(limits)
            .map(|limits| AgentSpec::Deepening {
                limits,
                quiescence,
                repetition,
                order,
                table: table.then(TableOptions::default),
                weights: Weights::TUNED,
            })
            .ok_or_else(|| ParseAgentError::Limits(s.to_string()));
    }
    let depth = |digits: &str| -> Result<u32, ParseAgentError> {
        digits
            .parse()
            .ok()
            .filter(|d| (1..=AgentSpec::MAX_DEPTH).contains(d))
            .ok_or_else(|| ParseAgentError::Depth(s.to_string()))
    };
    if let Some(digits) = s.strip_prefix("mm") {
        return depth(digits).map(|depth| AgentSpec::Minimax {
            depth,
            weights: Weights::TUNED,
        });
    }
    if let Some(rest) = s.strip_prefix("ab") {
        let (digits, letters) = rest.split_at(
            rest.find(|c: char| !c.is_ascii_digit())
                .unwrap_or(rest.len()),
        );
        let (quiescence, repetition, order, table) =
            parse_letters(letters).ok_or_else(|| ParseAgentError::Depth(s.to_string()))?;
        return depth(digits).map(|depth| AgentSpec::AlphaBeta {
            depth,
            quiescence,
            repetition,
            order,
            table: table.then(TableOptions::default),
            weights: Weights::TUNED,
        });
    }
    let Some(numbers) = s.strip_prefix("u:") else {
        return Err(ParseAgentError::Unknown(s.to_string()));
    };
    // `collect` into an Option stops at the first part that is not a finite number.
    let numbers: Option<Vec<f64>> = numbers
        .split('/')
        .map(|part| part.parse::<f64>().ok().filter(|x| x.is_finite()))
        .collect();
    match numbers.as_deref() {
        Some(&[man, king, mob]) => Ok(AgentSpec::U(Weights {
            man,
            king,
            mob,
            ..Weights::ZERO
        })),
        _ => Err(ParseAgentError::Weights(s.to_string())),
    }
}

/// The limits of `id:LIMITS` from `:LIMITS`, or `None` if they are not one to three different
/// limits `:dN`, `:nK`, `:tMS` with positive whole numbers.
fn parse_limits(s: &str) -> Option<Limits> {
    let mut limits = Limits::default();
    // `split` on ":d5:n100" gives "", "d5", "n100": the empty part before the first ':' is skipped.
    let mut parts = s.split(':');
    if parts.next() != Some("") {
        return None;
    }
    for part in parts {
        let (kind, number) = part.split_at_checked(1)?;
        // `u64::from_str` accepts a leading '+'; a limit is digits only.
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let number: u64 = number.parse().ok().filter(|&n| n > 0)?;
        match kind {
            "d" if limits.depth.is_none() => {
                limits.depth = Some(u32::try_from(number).ok().filter(|&d| d <= MAX_ID_DEPTH)?)
            }
            "n" if limits.nodes.is_none() => limits.nodes = Some(number),
            "t" if limits.time.is_none() => limits.time = Some(Duration::from_millis(number)),
            _ => return None,
        }
    }
    (limits != Limits::default()).then_some(limits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(s: &str) -> AgentSpec {
        s.parse().unwrap()
    }

    #[test]
    fn names_and_weights() {
        assert_eq!(spec("random"), AgentSpec::Random);
        assert_eq!(spec("material"), AgentSpec::U(Weights::MATERIAL));
        assert_eq!(spec("first"), AgentSpec::U(Weights::FIRST_GUESS));
        assert_eq!(spec("tuned"), AgentSpec::U(Weights::TUNED));
        assert_eq!(
            spec("mm3"),
            AgentSpec::Minimax {
                depth: 3,
                weights: Weights::TUNED
            }
        );
        let ab_with = |depth, quiescence, repetition, order| AgentSpec::AlphaBeta {
            depth,
            quiescence,
            repetition,
            order,
            table: None,
            weights: Weights::TUNED,
        };
        let ab = |depth, quiescence, repetition| ab_with(depth, quiescence, repetition, false);
        assert_eq!(spec("ab5"), ab(5, false, false));
        assert_eq!(spec("ab3q"), ab(3, true, false));
        assert_eq!(spec("ab3r"), ab(3, false, true));
        assert_eq!(spec("ab3qr"), ab(3, true, true));
        let ordered = |depth, quiescence, repetition| ab_with(depth, quiescence, repetition, true);
        assert_eq!(spec("ab6o"), ordered(6, false, false));
        assert_eq!(spec("ab3qo"), ordered(3, true, false));
        assert_eq!(spec("ab3ro"), ordered(3, false, true));
        assert_eq!(spec("ab3qro"), ordered(3, true, true));
        let nodes = Limits {
            nodes: Some(20000),
            ..Limits::default()
        };
        let id_with = |limits, quiescence, repetition, order| AgentSpec::Deepening {
            limits,
            quiescence,
            repetition,
            order,
            table: None,
            weights: Weights::TUNED,
        };
        let id = |limits, quiescence, repetition| id_with(limits, quiescence, repetition, false);
        assert_eq!(spec("id:n20000"), id(nodes, false, false));
        assert_eq!(spec("idq:n20000"), id(nodes, true, false));
        assert_eq!(spec("idr:n20000"), id(nodes, false, true));
        assert_eq!(spec("idqr:n20000"), id(nodes, true, true));
        assert_eq!(spec("idqro:n20000"), id_with(nodes, true, true, true));
        assert_eq!(spec("ido:n20000"), id_with(nodes, false, false, true));
        let limits = Limits {
            depth: Some(5),
            nodes: None,
            time: Some(Duration::from_millis(50)),
        };
        assert_eq!(spec("id:t50:d5"), id(limits, false, false));
        assert_eq!(
            spec("u:1/2/0.2"),
            AgentSpec::U(Weights {
                man: 1.0,
                king: 2.0,
                mob: 0.2,
                ..Weights::ZERO
            })
        );
    }

    #[test]
    fn weights_at_the_end_of_a_name() {
        let weights = |s: &str| spec(s).weights().unwrap();
        assert_eq!(
            weights("idqr:n2000:adv=0.1:qmob=-0.02"),
            Weights {
                adv: 0.1,
                qmob: -0.02,
                ..Weights::TUNED
            }
        );
        assert_eq!(
            spec("idqr:n2000:adv=0.1"),
            AgentSpec::Deepening {
                limits: Limits {
                    nodes: Some(2000),
                    ..Limits::default()
                },
                quiescence: true,
                repetition: true,
                order: false,
                table: None,
                weights: Weights {
                    adv: 0.1,
                    ..Weights::TUNED
                },
            }
        );
        // The king's value, and the other weights of U, are set the same way.
        assert_eq!(
            weights("ab3q:king=3:mob=0"),
            Weights {
                king: 3.0,
                mob: 0.0,
                ..Weights::TUNED
            }
        );
        // On top of the named weights for the baseline agent.
        assert_eq!(
            weights("material:cen=0.5"),
            Weights {
                cen: 0.5,
                ..Weights::MATERIAL
            }
        );
        assert_eq!(
            weights("u:1/2/0.2:edge=-1:back=2"),
            Weights {
                man: 1.0,
                king: 2.0,
                mob: 0.2,
                edge: -1.0,
                back: 2.0,
                ..Weights::ZERO
            }
        );
        // A weight set to its default changes nothing.
        assert_eq!(spec("idqr:n2000:adv=0:king=2.5"), spec("idqr:n2000"));
        assert_eq!(spec("tuned:qmob=0"), spec("tuned"));
    }

    #[test]
    fn named_weight_sets_in_a_name() {
        let weights = |s: &str| spec(s).weights().unwrap();
        assert_eq!(weights("idqr:n2000:w=material"), Weights::MATERIAL);
        assert_eq!(
            weights("ab3q:w=first:cen=0.5"),
            Weights {
                cen: 0.5,
                ..Weights::FIRST_GUESS
            }
        );
        // For the baseline agent too, replacing its named weights.
        assert_eq!(weights("tuned:w=material"), Weights::MATERIAL);
        assert_eq!(spec("idqr:n2000:w=tuned"), spec("idqr:n2000"));
        // The canonical name starts from the named set that needs the fewest parts.
        for name in [
            "idqr:n2000:w=material",
            "ab3q:w=first:cen=0.5",
            "idqr:n2000:w=first:adv=0.1:cen=0.2",
        ] {
            assert_eq!(spec(name).to_string(), name);
        }
        assert_eq!(spec("idqr:n2000:w=tuned").to_string(), "idqr:n2000");
        // The Texel weights, and one of them taken out for an ablation.
        assert_eq!(weights("idqr:n2000:w=texel1"), Weights::TEXEL1);
        for name in ["idqr:n2000:w=texel1", "idqr:n2000:w=texel1:adv=0"] {
            assert_eq!(spec(name).to_string(), name);
        }
        assert_eq!(spec("texel1"), AgentSpec::U(Weights::TEXEL1));
        assert_eq!(spec("texel1").to_string(), "texel1");
        // The second Texel tuning, full and simplified.
        assert_eq!(weights("idqr:n2000:w=texel2"), Weights::TEXEL2);
        assert_eq!(weights("idqr:n20000:w=texel2s"), Weights::TEXEL2S);
        for name in ["idqr:n2000:w=texel2", "idqr:n20000:w=texel2s", "texel2s"] {
            assert_eq!(spec(name).to_string(), name);
        }
        assert_eq!(spec("ab3:king=3:mob=0").to_string(), "ab3:w=material");
        // One part from tuned beats two from material.
        assert_eq!(spec("ab3:w=material:mob=-0.05").to_string(), "ab3:king=3");
    }

    #[test]
    fn names_with_weights_are_canonical_and_parse_back() {
        for name in [
            "mm3:adv=0.1",
            "ab3q:king=3",
            "ab5qr:man=1.5:adv=0.25:back=-1:cen=0.05:edge=-0.1:qmob=0.01",
            "id:d5:n100:t50:mob=0",
            "idqr:n2000:adv=0.1:qmob=0.02",
            "u:1/2.5/-0.05:cen=0.05",
            "u:1/2/0.2:adv=0.00001",
        ] {
            assert_eq!(spec(name).to_string(), name);
        }
        // Rust writes f64 without an exponent.
        assert_eq!(spec("ab3:adv=1e-5").to_string(), "ab3:adv=0.00001");
        // Weights in any order come out in the order of Weights::KEYS.
        assert_eq!(
            spec("idqr:n2000:qmob=0.02:adv=0.1:king=3").to_string(),
            "idqr:n2000:king=3:adv=0.1:qmob=0.02"
        );
        // A preset with a positional weight is written with its numbers.
        assert_eq!(spec("tuned:cen=0.05").to_string(), "u:1/2.5/-0.05:cen=0.05");
        assert_eq!(spec("ab3:king=2.5").to_string(), "ab3");
    }

    #[test]
    fn bad_weights_are_errors() {
        let weight = |s: &str| match s.parse::<AgentSpec>() {
            Err(ParseAgentError::Weight(name, _)) => name == s,
            _ => false,
        };
        for s in [
            "ab3:adv=",
            "ab3:adv=x",
            "ab3:adv=inf",
            "ab3:adv=NaN",
            "ab3:speed=1",
            "ab3:adv=1:adv=2",
            "ab3:adv=1=2",
            "random:adv=1",
            "idqr:n2000:ADV=1",
            "ab3:w=foo",
            "ab3:adv=0:w=tuned",
            "ab3:w=tuned:w=material",
        ] {
            assert!(weight(s), "{s} should be a weight error");
        }
        // A weight in front of the agent's own parts leaves those unreadable.
        assert_eq!(
            "adv=1".parse::<AgentSpec>(),
            Err(ParseAgentError::Unknown("adv=1".to_string()))
        );
        assert!(matches!(
            "id:adv=1:n200".parse::<AgentSpec>(),
            Err(ParseAgentError::Limits(_))
        ));
    }

    #[test]
    fn display_is_canonical_and_parses_back() {
        assert_eq!(spec("u:1/2.5/-0.05").to_string(), "tuned");
        assert_eq!(spec("u:1/3/0").to_string(), "material");
        assert_eq!(spec("u:1/3/0.1").to_string(), "first");
        for name in [
            "random",
            "material",
            "first",
            "tuned",
            "u:1/2/0.2",
            "u:0.5/4/-1",
            "mm1",
            "mm9",
            "ab1",
            "ab9",
            "id:d3",
            "id:n20000",
            "id:t50",
            "id:d5:n20000:t50",
            "id:d64",
            "ab3q",
            "ab9q",
            "idq:n20000",
            "idq:d3:t10",
            "ab1r",
            "ab9qr",
            "idr:d3",
            "idqr:n20000",
            "ab6o",
            "ab9qro",
            "ab3ro",
            "idqro:n20000",
            "ido:d3",
            "idro:t50",
        ] {
            assert_eq!(spec(name).to_string(), name);
        }
        assert_eq!(spec("id:t50:n100:d2").to_string(), "id:d2:n100:t50");
    }

    #[test]
    fn bad_names_are_errors() {
        let unknown = |s: &str| Err(ParseAgentError::Unknown(s.to_string()));
        let weights = |s: &str| Err(ParseAgentError::Weights(s.to_string()));
        let depth = |s: &str| Err(ParseAgentError::Depth(s.to_string()));
        for s in [
            "", "Tuned", "minimax", "1/2/3", "u", "idd3", "idqd3", "idx:d3", "idrq:d3", "idqrr:d3",
            "idoq:d3", "idoo:d3", "idqor:n5",
        ] {
            assert_eq!(s.parse::<AgentSpec>(), unknown(s));
        }
        for s in [
            "mm", "mm0", "mm10", "mm-1", "mm3x", "mm 3", "mm3q", "ab", "ab0", "ab10", "ab3x",
            "abq", "ab0q", "ab3qq", "ab10q", "ab3rq", "ab3rr", "abr", "ab0qr", "ab3qrq", "ab3oq",
            "ab3oo", "abo", "ab0o", "mm3o",
        ] {
            assert_eq!(s.parse::<AgentSpec>(), depth(s));
        }
        let limits = |s: &str| Err(ParseAgentError::Limits(s.to_string()));
        for s in [
            "id", "id:", "id:d", "id:d0", "id:d65", "id:n0", "id:x5", "id:d3:d4", "id:d3:",
            "id::d3", "id:n+5", "id:t-1", "id:t5ms", "id:n 5", "idq", "idq:", "idq:d0", "idq::n5",
            "idr", "idqr", "idqr:", "idr:d0", "ido", "idqro:", "idqro:d0",
        ] {
            assert_eq!(s.parse::<AgentSpec>(), limits(s));
        }
        for s in [
            "u:",
            "u:1/2",
            "u:1/2/3/4",
            "u:1/2/x",
            "u:1//3",
            "u:inf/1/1",
            "u:1/NaN/1",
        ] {
            assert_eq!(s.parse::<AgentSpec>(), weights(s));
        }
    }

    #[test]
    fn random_agent_draws_as_random_policy() {
        let board = Board::start();
        let legal = board.legal_moves(Color::White);
        let mut agent = RandomAgent::new(5);
        agent.new_game(2026).unwrap();
        // Seed 2026 draws 0.455... first: floor(0.455... * 8) = 3.
        assert_eq!(agent.choose(&board, Color::White, &legal), Ok(&legal[3]));
        // A new game with the same seed draws the same numbers again.
        agent.new_game(2026).unwrap();
        assert_eq!(agent.choose(&board, Color::White, &legal), Ok(&legal[3]));
    }

    #[test]
    fn u_agent_plays_the_move_decide_picks() {
        // From the start, mirror-image moves tie, so the seed decides between them.
        let board = Board::start();
        let legal = board.legal_moves(Color::White);
        let decision = decide(&board, Color::White, Weights::TUNED);
        assert!(decision.top.len() > 1);
        let mut agent = UAgent::new(Weights::TUNED, 0);
        for seed in [1, 2, 3, 2026] {
            agent.new_game(seed).unwrap();
            let expected = decision.choose(Some(&mut Mulberry32::new(seed)));
            assert_eq!(agent.choose(&board, Color::White, &legal).ok(), expected);
        }
        // Four moves, eight legal moves each, one node per legal move.
        assert_eq!(agent.nodes(), 32);
    }

    #[test]
    fn minimax_agent_plays_the_move_minimax_picks_and_counts_nodes() {
        let board = Board::start();
        let legal = board.legal_moves(Color::White);
        let decision = minimax(&board, Color::White, Weights::TUNED, 2);
        let mut agent = MinimaxAgent::new(Weights::TUNED, 2, 0);
        for seed in [1, 2, 3, 2026] {
            agent.new_game(seed).unwrap();
            let expected = decision.choose(Some(&mut Mulberry32::new(seed)));
            assert_eq!(agent.choose(&board, Color::White, &legal).ok(), expected);
        }
        assert_eq!(agent.nodes(), 4 * decision.nodes);
    }

    #[test]
    fn deepening_agent_with_a_depth_limit_plays_as_alpha_beta() {
        let board = Board::start();
        let legal = board.legal_moves(Color::White);
        let ab = alpha_beta(&board, Color::White, Weights::TUNED, 3);
        let limits = Limits {
            depth: Some(3),
            ..Limits::default()
        };
        let one = iterative_deepening(&board, Color::White, Weights::TUNED, limits, false);
        let mut agent = DeepeningAgent::new(Weights::TUNED, limits, false, 0);
        for seed in [1, 2, 3, 2026] {
            agent.new_game(seed).unwrap();
            let expected = ab.choose(Some(&mut Mulberry32::new(seed)));
            assert_eq!(agent.choose(&board, Color::White, &legal).ok(), expected);
        }
        assert_eq!(agent.nodes(), 4 * one.decision.nodes);
    }

    #[test]
    fn alpha_beta_agent_plays_the_move_minimax_picks_from_fewer_nodes() {
        let board = Board::start();
        let legal = board.legal_moves(Color::White);
        let mm = minimax(&board, Color::White, Weights::TUNED, 3);
        let ab = alpha_beta(&board, Color::White, Weights::TUNED, 3);
        assert!(ab.nodes < mm.nodes);
        let mut agent = AlphaBetaAgent::new(Weights::TUNED, 3, false, 0);
        for seed in [1, 2, 3, 2026] {
            agent.new_game(seed).unwrap();
            let expected = mm.choose(Some(&mut Mulberry32::new(seed)));
            assert_eq!(agent.choose(&board, Color::White, &legal).ok(), expected);
        }
        assert_eq!(agent.nodes(), 4 * ab.nodes);
    }

    #[test]
    fn table_letters_and_parts() {
        let table = |s: &str| match spec(s) {
            AgentSpec::AlphaBeta { table, .. } | AgentSpec::Deepening { table, .. } => table,
            _ => panic!("{s} is not a search"),
        };
        assert_eq!(table("ab6o"), None);
        assert_eq!(table("ab6ot"), Some(TableOptions::default()));
        assert_eq!(table("ab6t"), Some(TableOptions::default()));
        assert_eq!(
            table("idqrot:n20000:w=texel1:hash=64"),
            Some(TableOptions {
                mb: 64,
                strict: false
            })
        );
        assert_eq!(
            table("ab4ot:tt=strict:hash=2"),
            Some(TableOptions {
                mb: 2,
                strict: true
            })
        );
        // The weights are read beside the table's parts, in any order.
        assert_eq!(
            spec("idqrot:n20000:hash=64:w=texel1").weights(),
            Some(Weights::TEXEL1)
        );
        for name in [
            "ab6ot",
            "ab3qrot",
            "idqrot:n20000",
            "idqrot:n20000:w=texel1:hash=64",
            "ab4ot:tt=strict",
            "idt:d3:hash=1:tt=strict",
        ] {
            assert_eq!(spec(name).to_string(), name);
        }
        assert_eq!(spec("ab6ot:hash=16").to_string(), "ab6ot");
        assert_eq!(
            spec("ab6ot:tt=strict:king=3:hash=8").to_string(),
            "ab6ot:king=3:hash=8:tt=strict"
        );
        let table_error =
            |s: &str| matches!(s.parse::<AgentSpec>(), Err(ParseAgentError::Table(..)));
        for s in [
            "ab6o:hash=16",
            "idqr:n2000:tt=strict",
            "tuned:hash=1",
            "random:hash=1",
            "ab6ot:hash=0",
            "ab6ot:hash=65537",
            "ab6ot:hash=+4",
            "ab6ot:hash=x",
            "ab6ot:tt=loose",
            "ab6ot:hash=1:hash=2",
            "ab6ot:tt=strict:tt=strict",
        ] {
            assert!(table_error(s), "{s} should be a table error");
        }
        assert!(matches!(
            "ab6to".parse::<AgentSpec>(),
            Err(ParseAgentError::Depth(_))
        ));
        assert!(matches!(
            "idto:d3".parse::<AgentSpec>(),
            Err(ParseAgentError::Unknown(_))
        ));
    }

    #[test]
    fn ordered_agents_play_the_same_moves_from_other_nodes() {
        let board = Board::start();
        let legal = board.legal_moves(Color::White);
        for name in ["ab4", "ab3qr", "idq:d4", "idqr:d3:w=texel1"] {
            let ordered = format!(
                "{}o{}",
                &name[..name.find(':').unwrap_or(name.len())],
                &name[name.find(':').unwrap_or(name.len())..]
            );
            let (mut plain, mut fast) = (spec(name).build(), spec(&ordered).build());
            for seed in [1, 2, 3, 2026] {
                plain.new_game(seed).unwrap();
                fast.new_game(seed).unwrap();
                plain.observe(&board, Color::White);
                fast.observe(&board, Color::White);
                assert_eq!(
                    plain.choose(&board, Color::White, &legal),
                    fast.choose(&board, Color::White, &legal),
                    "{name} and {ordered}, seed {seed}"
                );
            }
        }
    }
}
