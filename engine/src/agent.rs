//! The agents that play in the arena: the random player, the baseline agent (`randomPolicy` and
//! `agentPolicy` in reference/js/agent.js), the minimax agent, the alpha-beta agent and the
//! iterative-deepening agent.
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
    Decision, Limits, MAX_ID_DEPTH, alpha_beta, alpha_beta_q, decide, iterative_deepening, minimax,
};

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
/// depth limit (`search::alpha_beta_q`).
#[derive(Clone, Debug)]
pub struct AlphaBetaAgent {
    weights: Weights,
    depth: u32,
    quiescence: bool,
    rng: Mulberry32,
    nodes: u64,
}

impl AlphaBetaAgent {
    pub fn new(weights: Weights, depth: u32, quiescence: bool, seed: u32) -> AlphaBetaAgent {
        AlphaBetaAgent {
            weights,
            depth,
            quiescence,
            rng: Mulberry32::new(seed),
            nodes: 0,
        }
    }
}

impl Agent for AlphaBetaAgent {
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
        let search = if self.quiescence {
            alpha_beta_q
        } else {
            alpha_beta
        };
        let decision = search(board, side, self.weights, self.depth);
        self.nodes += decision.nodes;
        Ok(play(&decision, &mut self.rng, legal))
    }

    fn nodes(&self) -> u64 {
        self.nodes
    }
}

/// Iterative deepening (`search::iterative_deepening`): alpha-beta 1, 2, 3, ... plies deep within
/// `limits`, the move of the deepest completed iteration, with or without quiescence. With only a
/// depth limit N it plays the moves of `AlphaBetaAgent` at depth N.
#[derive(Clone, Debug)]
pub struct DeepeningAgent {
    weights: Weights,
    limits: Limits,
    quiescence: bool,
    rng: Mulberry32,
    nodes: u64,
}

impl DeepeningAgent {
    pub fn new(weights: Weights, limits: Limits, quiescence: bool, seed: u32) -> DeepeningAgent {
        DeepeningAgent {
            weights,
            limits,
            quiescence,
            rng: Mulberry32::new(seed),
            nodes: 0,
        }
    }
}

impl Agent for DeepeningAgent {
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
        let deepening =
            iterative_deepening(board, side, self.weights, self.limits, self.quiescence);
        self.nodes += deepening.decision.nodes;
        Ok(play(&deepening.decision, &mut self.rng, legal))
    }

    fn nodes(&self) -> u64 {
        self.nodes
    }
}

/// An agent as the command line names it: `random`, a preset name (`material`, `first`,
/// `tuned`), `u:MAN/KING/MOB` for the baseline agent with any weights, `mmN` for minimax N
/// plies deep with the tuned weights, `abN` for alpha-beta N plies deep with the tuned weights,
/// or `id:LIMITS` for iterative deepening with the tuned weights. LIMITS are one or more of `dN`
/// (at most N plies), `nK` (at most K nodes per move) and `tMS` (at most MS milliseconds per move),
/// separated by `:`, for example `id:n20000` or `id:d5:t50`. `abNq` and `idq:LIMITS` add
/// quiescence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AgentSpec {
    Random,
    /// The baseline agent with these weights.
    U(Weights),
    /// Minimax this many plies deep, 1 to `MAX_DEPTH`, with the tuned weights.
    Minimax(u32),
    /// Alpha-beta `depth` plies deep, 1 to `MAX_DEPTH`, with the tuned weights.
    AlphaBeta {
        depth: u32,
        quiescence: bool,
    },
    /// Iterative deepening within `limits`, with the tuned weights.
    Deepening {
        limits: Limits,
        quiescence: bool,
    },
}

impl AgentSpec {
    /// The weights that have a name.
    pub const PRESETS: [(&'static str, Weights); 3] = [
        ("material", Weights::MATERIAL),
        ("first", Weights::FIRST_GUESS),
        ("tuned", Weights::TUNED),
    ];

    /// The deepest search (`mmN`, `abN`) the command line accepts: plain minimax grows about
    /// tenfold per ply.
    pub const MAX_DEPTH: u32 = 9;

    /// The name of the preset with these weights, if there is one.
    pub fn preset(&self) -> Option<&'static str> {
        match self {
            AgentSpec::Random
            | AgentSpec::Minimax(_)
            | AgentSpec::AlphaBeta { .. }
            | AgentSpec::Deepening { .. } => None,
            AgentSpec::U(weights) => AgentSpec::PRESETS
                .iter()
                .find(|(_, w)| w == weights)
                .map(|(name, _)| *name),
        }
    }

    /// A new agent of this kind. The arena reseeds it before every game.
    pub fn build(&self) -> Box<dyn Agent> {
        match *self {
            AgentSpec::Random => Box::new(RandomAgent::new(0)),
            AgentSpec::U(weights) => Box::new(UAgent::new(weights, 0)),
            AgentSpec::Minimax(depth) => Box::new(MinimaxAgent::new(Weights::TUNED, depth, 0)),
            AgentSpec::AlphaBeta { depth, quiescence } => {
                Box::new(AlphaBetaAgent::new(Weights::TUNED, depth, quiescence, 0))
            }
            AgentSpec::Deepening { limits, quiescence } => {
                Box::new(DeepeningAgent::new(Weights::TUNED, limits, quiescence, 0))
            }
        }
    }
}

impl fmt::Display for AgentSpec {
    /// The canonical name: `random`, `mmN`, `abN` or `abNq`, `id:` or `idq:` with the limits in the
    /// order d, n, t, the preset's name, or else `u:MAN/KING/MOB`.
    /// `"u:1/3/0".parse()` therefore prints as `material`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self, self.preset()) {
            (AgentSpec::Random, _) => f.write_str("random"),
            (AgentSpec::U(_), Some(name)) => f.write_str(name),
            (AgentSpec::U(weights), None) => write!(f, "u:{weights}"),
            (AgentSpec::Minimax(depth), _) => write!(f, "mm{depth}"),
            (AgentSpec::AlphaBeta { depth, quiescence }, _) => {
                write!(f, "ab{depth}{}", if *quiescence { "q" } else { "" })
            }
            (AgentSpec::Deepening { limits, quiescence }, _) => {
                f.write_str(if *quiescence { "idq" } else { "id" })?;
                if let Some(depth) = limits.depth {
                    write!(f, ":d{depth}")?;
                }
                if let Some(nodes) = limits.nodes {
                    write!(f, ":n{nodes}")?;
                }
                if let Some(time) = limits.time {
                    write!(f, ":t{}", time.as_millis())?;
                }
                Ok(())
            }
        }
    }
}

/// Why a name is not an agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseAgentError {
    /// Neither `random`, a preset name, `u:...`, `mmN`, `abN`, `abNq`, `id:...` nor `idq:...`.
    Unknown(String),
    /// `mm` or `ab` is not followed by a depth from 1 to `AgentSpec::MAX_DEPTH` (and for `ab`, an
    /// optional `q`).
    Depth(String),
    /// `u:` is not followed by three finite numbers separated by `/`.
    Weights(String),
    /// `id` or `idq` is not followed by one to three different limits `:dN`, `:nK`, `:tMS`, each a
    /// positive whole number (N at most `MAX_ID_DEPTH`).
    Limits(String),
}

impl fmt::Display for ParseAgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseAgentError::Unknown(name) => write!(
                f,
                "unknown agent {name:?}: expected random, material, first, tuned, u:MAN/KING/MOB, \
                 mmN, abN, abNq, id:LIMITS or idq:LIMITS"
            ),
            ParseAgentError::Depth(name) => write!(
                f,
                "agent {name:?}: expected a depth from 1 to {} after mm or ab, for example mm3, ab5 \
                 or ab3q",
                AgentSpec::MAX_DEPTH
            ),
            ParseAgentError::Weights(name) => write!(
                f,
                "agent {name:?}: expected three numbers after u:, separated by '/', \
                 for example u:1/2.5/-0.05"
            ),
            ParseAgentError::Limits(name) => write!(
                f,
                "agent {name:?}: expected limits after id or idq, separated by ':': dN (plies, 1 to \
                 {MAX_ID_DEPTH}), nK (nodes per move), tMS (milliseconds per move), \
                 for example id:n20000 or id:d5:t50"
            ),
        }
    }
}

impl std::error::Error for ParseAgentError {}

impl FromStr for AgentSpec {
    type Err = ParseAgentError;

    fn from_str(s: &str) -> Result<AgentSpec, ParseAgentError> {
        if s == "random" {
            return Ok(AgentSpec::Random);
        }
        if let Some((_, weights)) = AgentSpec::PRESETS.iter().find(|(name, _)| *name == s) {
            return Ok(AgentSpec::U(*weights));
        }
        // `idq` before `id`, since "idq:..." also starts with "id".
        let deepening = [("idq", true), ("id", false)]
            .into_iter()
            .find_map(|(prefix, quiescence)| Some((s.strip_prefix(prefix)?, quiescence)));
        if let Some((limits, quiescence)) = deepening
            && (limits.is_empty() || limits.starts_with(':'))
        {
            return parse_limits(limits)
                .map(|limits| AgentSpec::Deepening { limits, quiescence })
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
            return depth(digits).map(AgentSpec::Minimax);
        }
        if let Some(rest) = s.strip_prefix("ab") {
            let (digits, quiescence) = match rest.strip_suffix('q') {
                Some(digits) => (digits, true),
                None => (rest, false),
            };
            return depth(digits).map(|depth| AgentSpec::AlphaBeta { depth, quiescence });
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
            Some(&[man, king, mob]) => Ok(AgentSpec::U(Weights { man, king, mob })),
            _ => Err(ParseAgentError::Weights(s.to_string())),
        }
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
        assert_eq!(spec("mm3"), AgentSpec::Minimax(3));
        assert_eq!(
            spec("ab5"),
            AgentSpec::AlphaBeta {
                depth: 5,
                quiescence: false
            }
        );
        assert_eq!(
            spec("ab3q"),
            AgentSpec::AlphaBeta {
                depth: 3,
                quiescence: true
            }
        );
        let nodes = Limits {
            nodes: Some(20000),
            ..Limits::default()
        };
        assert_eq!(
            spec("id:n20000"),
            AgentSpec::Deepening {
                limits: nodes,
                quiescence: false
            }
        );
        assert_eq!(
            spec("idq:n20000"),
            AgentSpec::Deepening {
                limits: nodes,
                quiescence: true
            }
        );
        assert_eq!(
            spec("id:t50:d5"),
            AgentSpec::Deepening {
                limits: Limits {
                    depth: Some(5),
                    nodes: None,
                    time: Some(Duration::from_millis(50)),
                },
                quiescence: false
            }
        );
        assert_eq!(
            spec("u:1/2/0.2"),
            AgentSpec::U(Weights {
                man: 1.0,
                king: 2.0,
                mob: 0.2
            })
        );
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
            "", "Tuned", "minimax", "1/2/3", "u", "idd3", "idqd3", "idx:d3",
        ] {
            assert_eq!(s.parse::<AgentSpec>(), unknown(s));
        }
        for s in [
            "mm", "mm0", "mm10", "mm-1", "mm3x", "mm 3", "mm3q", "ab", "ab0", "ab10", "ab3x",
            "abq", "ab0q", "ab3qq", "ab10q",
        ] {
            assert_eq!(s.parse::<AgentSpec>(), depth(s));
        }
        let limits = |s: &str| Err(ParseAgentError::Limits(s.to_string()));
        for s in [
            "id", "id:", "id:d", "id:d0", "id:d65", "id:n0", "id:x5", "id:d3:d4", "id:d3:",
            "id::d3", "id:n+5", "id:t-1", "id:t5ms", "id:n 5", "idq", "idq:", "idq:d0", "idq::n5",
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
}
