//! Monte Carlo tree search (AIMA 6.4), in four forms that add one thing each:
//!
//! - `mc`: flat Monte Carlo. The budget goes to the root moves in turn; after each move a random
//!   game is played, and the move with the highest score (a win 1, a draw ½) is played.
//! - `uct`: UCT. Each iteration descends the tree by UCB1, expands one new position, plays a
//!   random game from it and adds the result to every position on the way back.
//! - `ucte`: UCT whose new position is scored with U instead of a random game: sigmoid(K · U),
//!   the probability Texel tuning fits U to (`texel.rs`), with K = `TEXEL1_K`.
//! - `uctq`: `ucte` with the value of quiescence (`search::quiescence_value`) in place of U, so
//!   that a position in the middle of a capture sequence is scored after the captures.
//!
//! A random game ends by the arena's rules (`arena::game_end`) without the third repetition: a
//! side without a legal move loses, one piece each, 50 quiet plies and the ply limit are draws.
//! The game's own ply and quiet-ply counts carry into the tree and the random games, learnt from
//! `Agent::observe`. The third repetition is left out: it needs the positions of every random
//! game, and a random game that circles ends at 50 quiet plies anyway.
//!
//! The budget is counted as alpha-beta counts it (`search::Counter`): every position produced
//! with `apply`, the root not included, here in the tree, in the random games and in quiescence.
//! A visit to a position of the tree where the game is over produces none, and is counted as one,
//! so that a tree whose every line has ended still uses up the budget. An iteration (or a random
//! game of `mc`) that the budget interrupts is thrown away.
//!
//! The move played is the root move visited most (AIMA's choice), and among those the first in
//! legal order with the highest mean; for `mc` the first with the highest mean.

use std::time::Instant;

use crate::board::{Board, Color};
use crate::eval::Weights;
use crate::movegen::Move;
use crate::rng::Mulberry32;
use crate::search::{Limits, quiescence_value, static_value};

/// The four forms (see the module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MctsKind {
    Flat,
    Uct,
    UctEval,
    UctQuiescence,
}

impl MctsKind {
    /// Every form, by its name in an agent's name.
    pub const NAMES: [(&'static str, MctsKind); 4] = [
        ("mc", MctsKind::Flat),
        ("uct", MctsKind::Uct),
        ("ucte", MctsKind::UctEval),
        ("uctq", MctsKind::UctQuiescence),
    ];

    pub fn name(self) -> &'static str {
        let (name, _) = Self::NAMES.iter().find(|(_, k)| *k == self).expect("named");
        name
    }

    /// Whether its positions are scored with U, so that it has weights.
    pub fn evaluates(self) -> bool {
        matches!(self, MctsKind::UctEval | MctsKind::UctQuiescence)
    }
}

/// The K of the first Texel tuning (`Weights::TEXEL1`): sigmoid(K · U) is the expected score.
pub const TEXEL1_K: f64 = 0.2353;

/// The exploration constant of UCB1, √2 as in AIMA.
pub const EXPLORATION: f64 = std::f64::consts::SQRT_2;

/// The ply at which a game is drawn: `MatchSettings::PAIRED.max_plies`.
const MAX_PLIES: u32 = 400;
/// Plies without a capture that draw a game, as in the arena.
const QUIET_PLIES: u32 = 50;

/// Where the game stands: what the draw rules need besides the board.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Clock {
    /// Plies played in the game.
    pub ply: u32,
    /// Plies in a row without a capture.
    pub quiet: u32,
}

impl Clock {
    fn after(self, mv: &Move) -> Clock {
        Clock {
            ply: self.ply + 1,
            quiet: if mv.is_capture() { 0 } else { self.quiet + 1 },
        }
    }
}

/// The score of a finished game for `side`: 1 for a win, ½ for a draw, 0 for a loss.
fn score(winner: Option<Color>, side: Color) -> f64 {
    match winner {
        None => 0.5,
        Some(w) if w == side => 1.0,
        Some(_) => 0.0,
    }
}

/// How the game stands at `board`, `to_move` having `moves`: `Some(winner)` if it is over.
fn game_over(board: &Board, to_move: Color, moves: &[Move], clock: Clock) -> Option<Option<Color>> {
    if moves.is_empty() {
        return Some(Some(to_move.opponent()));
    }
    let one_each = board.pieces(Color::White).len() == 1 && board.pieces(Color::Black).len() == 1;
    (one_each || clock.quiet >= QUIET_PLIES || clock.ply >= MAX_PLIES).then_some(None)
}

/// The positions produced so far, and the budget.
#[derive(Clone, Debug)]
struct Budget {
    nodes: u64,
    max_nodes: Option<u64>,
    deadline: Option<Instant>,
}

impl Budget {
    fn new(limits: Limits) -> Budget {
        Budget {
            nodes: 0,
            max_nodes: limits.nodes,
            deadline: limits.time.map(|t| Instant::now() + t),
        }
    }

    /// Counts one more position, or `None` if the budget is spent.
    fn visit(&mut self) -> Option<()> {
        self.take(1)
    }

    /// Counts `n` more positions, or `None` (counting none) if the budget does not have them.
    fn take(&mut self, n: u64) -> Option<()> {
        let out_of_nodes = self.max_nodes.is_some_and(|max| self.nodes + n > max);
        let out_of_time = self.deadline.is_some_and(|d| Instant::now() >= d);
        if out_of_nodes || out_of_time {
            return None;
        }
        self.nodes += n;
        Some(())
    }

    /// The positions left, for quiescence.
    fn left(&self) -> u64 {
        self.max_nodes.map_or(u64::MAX - 1, |max| max - self.nodes)
    }
}

/// A random game from `board`, `to_move` having `moves`, to its end: the winner, or `None` if it
/// is drawn or the budget runs out first (`Err`).
fn random_game(
    board: &Board,
    to_move: Color,
    moves: Vec<Move>,
    clock: Clock,
    budget: &mut Budget,
    rng: &mut Mulberry32,
) -> Result<Option<Color>, ()> {
    let (mut board, mut to_move, mut moves, mut clock) = (*board, to_move, moves, clock);
    loop {
        if let Some(winner) = game_over(&board, to_move, &moves, clock) {
            return Ok(winner);
        }
        budget.visit().ok_or(())?;
        let mv = &moves[rng.next_index(moves.len())];
        clock = clock.after(mv);
        board = board.apply(mv);
        to_move = to_move.opponent();
        moves = board.legal_moves(to_move);
    }
}

/// What a search found, per root move in legal order.
#[derive(Clone, Debug, PartialEq)]
pub struct MctsResult {
    /// The index in the legal moves of the move to play.
    pub best: usize,
    /// Per root move: its visits and its mean score for the root side (0 if not visited).
    pub visits: Vec<u32>,
    pub means: Vec<f64>,
    /// The positions produced, as the budget counts them.
    pub nodes: u64,
    /// Iterations (random games for `mc`) completed.
    pub iterations: u64,
    /// The deepest ply of the tree (1 for `mc`).
    pub depth: u32,
}

/// The search of `kind` on `board`, `side` to move with `legal` (at least one move), within
/// `limits` (nodes and/or time), the game at `clock`, scored with `weights` where `kind`
/// evaluates.
#[allow(clippy::too_many_arguments)]
pub fn mcts(
    kind: MctsKind,
    board: &Board,
    side: Color,
    legal: &[Move],
    clock: Clock,
    limits: Limits,
    weights: Weights,
    rng: &mut Mulberry32,
) -> MctsResult {
    assert!(!legal.is_empty(), "the search needs a legal move");
    assert!(
        limits.nodes.is_some() || limits.time.is_some(),
        "the search needs a node or time limit"
    );
    let mut budget = Budget::new(limits);
    match kind {
        MctsKind::Flat => flat(board, side, legal, clock, &mut budget, rng),
        _ => uct(kind, board, side, legal, clock, weights, &mut budget, rng),
    }
}

/// Flat Monte Carlo: root moves in turn, a random game after each.
fn flat(
    board: &Board,
    side: Color,
    legal: &[Move],
    clock: Clock,
    budget: &mut Budget,
    rng: &mut Mulberry32,
) -> MctsResult {
    let n = legal.len();
    let (mut visits, mut totals) = (vec![0u32; n], vec![0.0; n]);
    let mut iterations = 0;
    'search: for i in (0..n).cycle() {
        if budget.visit().is_none() {
            break 'search;
        }
        let after = board.apply(&legal[i]);
        let moves = after.legal_moves(side.opponent());
        match random_game(
            &after,
            side.opponent(),
            moves,
            clock.after(&legal[i]),
            budget,
            rng,
        ) {
            Ok(winner) => {
                visits[i] += 1;
                totals[i] += score(winner, side);
                iterations += 1;
            }
            Err(()) => break 'search,
        }
    }
    let means = means(&visits, &totals);
    let best = first_max(&means, |i| visits[i] > 0).unwrap_or(0);
    MctsResult {
        best,
        visits,
        means,
        nodes: budget.nodes,
        iterations,
        depth: 1,
    }
}

/// `total / visits` per move, 0 where there are no visits.
fn means(visits: &[u32], totals: &[f64]) -> Vec<f64> {
    (visits.iter().zip(totals))
        .map(|(&v, &t)| if v == 0 { 0.0 } else { t / f64::from(v) })
        .collect()
}

/// The first index among those `allowed` with the highest value.
fn first_max(values: &[f64], allowed: impl Fn(usize) -> bool) -> Option<usize> {
    (0..values.len())
        .filter(|&i| allowed(i))
        .fold(None, |best: Option<usize>, i| match best {
            Some(b) if values[b] >= values[i] => Some(b),
            _ => Some(i),
        })
}

/// A position of the UCT tree.
#[derive(Clone, Debug)]
struct Node {
    board: Board,
    to_move: Color,
    clock: Clock,
    moves: Vec<Move>,
    /// `Some(winner)` if the game is over here.
    over: Option<Option<Color>>,
    /// The tree's indices of the children, those of `moves[0]`, `moves[1]`, ... as expanded.
    children: Vec<usize>,
    visits: u32,
    /// The scores of the visits for the side that moved into this position.
    total: f64,
    ply: u32,
}

impl Node {
    fn new(board: Board, to_move: Color, clock: Clock, ply: u32) -> Node {
        let moves = board.legal_moves(to_move);
        let over = game_over(&board, to_move, &moves, clock);
        Node {
            board,
            to_move,
            clock,
            moves,
            over,
            children: Vec::new(),
            visits: 0,
            total: 0.0,
            ply,
        }
    }
}

/// UCT with a random game, U or quiescence at the new position.
#[allow(clippy::too_many_arguments)]
fn uct(
    kind: MctsKind,
    board: &Board,
    side: Color,
    legal: &[Move],
    clock: Clock,
    weights: Weights,
    budget: &mut Budget,
    rng: &mut Mulberry32,
) -> MctsResult {
    // The tree lives in one vector and refers to positions by index, not by pointer.
    let mut tree = vec![Node::new(*board, side, clock, 0)];
    debug_assert_eq!(tree[0].moves, legal);
    let (mut iterations, mut depth) = (0, 0);
    let mut path = Vec::new();
    loop {
        // Selection: down the fully expanded positions by UCB1.
        path.clear();
        path.push(0);
        let mut at = 0;
        while tree[at].over.is_none() && tree[at].children.len() == tree[at].moves.len() {
            let parent = &tree[at];
            let log_n = f64::from(parent.visits).ln();
            let ucb = |&c: &usize| {
                let child = &tree[c];
                let n = f64::from(child.visits);
                child.total / n + EXPLORATION * (log_n / n).sqrt()
            };
            let scores: Vec<f64> = parent.children.iter().map(ucb).collect();
            at = parent.children[first_max(&scores, |_| true).expect("a child")];
            path.push(at);
        }
        // Expansion and the score, for the side that moved into the last position of the path.
        let result = if let Some(winner) = tree[at].over {
            budget
                .visit()
                .map(|()| score(winner, tree[at].to_move.opponent()))
        } else {
            budget.visit().and_then(|()| {
                let parent = &tree[at];
                let mv = &parent.moves[parent.children.len()];
                let child = Node::new(
                    parent.board.apply(mv),
                    parent.to_move.opponent(),
                    parent.clock.after(mv),
                    parent.ply + 1,
                );
                let r = leaf_score(kind, &child, weights, budget, rng)?;
                tree.push(child);
                let c = tree.len() - 1;
                tree[at].children.push(c);
                path.push(c);
                depth = depth.max(tree[c].ply);
                Some(r)
            })
        };
        let Some(r) = result else { break };
        // Backpropagation: the score alternates between the sides.
        let mover = tree[*path.last().expect("the root")].to_move.opponent();
        for &p in &path {
            let node = &mut tree[p];
            node.visits += 1;
            node.total += if node.to_move.opponent() == mover {
                r
            } else {
                1.0 - r
            };
        }
        iterations += 1;
    }
    let root = &tree[0];
    let mut visits = vec![0; legal.len()];
    let mut totals = vec![0.0; legal.len()];
    for (i, &c) in root.children.iter().enumerate() {
        (visits[i], totals[i]) = (tree[c].visits, tree[c].total);
    }
    let means = means(&visits, &totals);
    let most = visits.iter().copied().max().unwrap_or(0);
    let best = first_max(&means, |i| visits[i] == most).unwrap_or(0);
    MctsResult {
        best,
        visits,
        means,
        nodes: budget.nodes,
        iterations,
        depth,
    }
}

/// The score of a new position for the side that moved into it, or `None` if the budget runs
/// out.
fn leaf_score(
    kind: MctsKind,
    node: &Node,
    weights: Weights,
    budget: &mut Budget,
    rng: &mut Mulberry32,
) -> Option<f64> {
    let mover = node.to_move.opponent();
    if let Some(winner) = node.over {
        return Some(score(winner, mover));
    }
    let sigmoid = |v: f64| 1.0 / (1.0 + (-TEXEL1_K * v).exp());
    match kind {
        MctsKind::Flat => unreachable!("flat Monte Carlo has no tree"),
        MctsKind::Uct => {
            let moves = node.moves.clone();
            random_game(&node.board, node.to_move, moves, node.clock, budget, rng)
                .ok()
                .map(|winner| score(winner, mover))
        }
        MctsKind::UctEval => Some(sigmoid(static_value(
            &node.board,
            node.to_move,
            mover,
            weights,
        ))),
        MctsKind::UctQuiescence => {
            let (v, nodes) =
                quiescence_value(&node.board, node.to_move, mover, weights, budget.left())?;
            budget.take(nodes)?;
            Some(sigmoid(v))
        }
    }
}
