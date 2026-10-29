//! The agents' search: the baseline's one ply (`decide` in reference/js/agent.js),
//! depth-limited minimax (`minimax`), alpha-beta (`alpha_beta`), which makes the same decisions
//! as minimax at the same depth from fewer nodes, and iterative deepening (`iterative_deepening`),
//! which runs alpha-beta 1, 2, 3, ... plies deep until a node or time budget runs out.
//!
//! Minimax values every position from the root side's view:
//!
//! - A side without a legal move loses. Found at ply p (the root is ply 0), that is worth
//!   `WIN - (p - 1)` to the winner, so a shorter win is worth more, and a win right after our move
//!   is worth `WIN`, as in the baseline's U.
//! - At the depth limit, U with the given weights, but never the win flag: the side to move has
//!   moves there, and when it is the root side the opponent's lack of moves is not yet a win.
//!
//! Depth 1 therefore gives every move the same U as `decide`.
//!
//! Alpha-beta searches the same tree in the same move order with the same scores, but skips the
//! subtrees that cannot change a value it needs. It only selects among leaf values (no arithmetic),
//! so every value it finds exactly is bit for bit the minimax value. At the root it needs exact
//! values only for the moves that can be within `U_EPSILON` of the best (see `alpha_beta`).
//!
//! Without move ordering, iterative deepening's iterations do not depend on each other: a
//! completed iteration at depth d is exactly `alpha_beta` at depth d.
//!
//! Move ordering (`SearchOptions::order`) searches likely refutations first, so that alpha-beta
//! cuts off sooner: at the root of an iteration the previous iteration's best move, below the root
//! promotions first, then the killer moves of the ply, then the quiet moves by the history
//! heuristic (`Heuristics`); ties keep the legal order. It changes the nodes, never the decision:
//! every value alpha-beta finds exactly is the minimax value whatever the order, and a root move
//! that gets only a bound is out of `top` whatever the order (see `alpha_beta`), so `top` and the
//! values in it are the same, and `candidates` stay in legal-move order for `pick`.
//!
//! Quiescence (`minimax_q`, `alpha_beta_q`, and iterative deepening with `quiescence`) changes only
//! the depth limit: a position there whose side to move must capture is not scored, and every
//! capture is searched instead, ply after ply, until a position without a capture (or without a
//! move) is reached. Capturing is mandatory, so the side to move cannot decline ("stand pat") as in
//! chess, and since every capture removes a piece, the captures run out by themselves. The nodes
//! beyond the depth limit are counted in `nodes` and, separately, in `q_nodes`.
//!
//! Repetitions (`alpha_beta_rep`, `iterative_deepening_rep`): a position below the root that
//! occurred before, in the game (`history`) or on the path from the root, is a draw worth `DRAW`
//! (0), and is not searched further. The game is drawn only at the third occurrence, but a side
//! that can repeat once can repeat again, so the first repetition already decides. With U from
//! the root side's view, a side ahead (U > 0) avoids the repetition and a side behind (U < 0)
//! welcomes it. Without repetitions, the search is the one above, node for node.

use std::cmp::Reverse;
use std::ops::Range;
use std::time::{Duration, Instant};

use crate::board::{Board, Color};
use crate::eval::{DRAW, Features, U_EPSILON, WIN, Weights, evaluate};
use crate::movegen::Move;
use crate::rng::Mulberry32;
use crate::square::Square;

/// How an alpha-beta search goes beyond its depth limit and weights.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchOptions {
    /// Search captures past the depth limit until the position is quiet.
    pub quiescence: bool,
    /// Order the moves (see the module documentation); the decision stays the same.
    pub order: bool,
}

/// A legal move with the features and U of the board after it.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub mv: Move,
    pub features: Features,
    /// The move's value. When `exact` is false it is only an upper bound, lower than the best
    /// value by more than `U_EPSILON`, so the move is not among the best.
    pub u: f64,
    /// Whether `u` is the exact value: always for `decide` and `minimax`, and for `alpha_beta`
    /// at least for every move within `U_EPSILON` of the best.
    pub exact: bool,
}

/// Every legal move evaluated one ply deep, and the ones with the highest U.
#[derive(Clone, Debug)]
pub struct Decision {
    /// One candidate per legal move, in legal-move order (the order of engine.js).
    pub candidates: Vec<Candidate>,
    /// Indices into `candidates` of the moves whose U is within `U_EPSILON` of the highest, in
    /// order.
    pub top: Vec<usize>,
    /// The positions the search produced with `apply`, the root not included: one per legal move
    /// for `decide`.
    pub nodes: u64,
    /// The part of `nodes` beyond the depth limit, produced by quiescence: 0 without it.
    pub q_nodes: u64,
}

/// Evaluates every legal move of `side` on `board` with U (`evaluate`: with the positional
/// features when their weights are set).
pub fn decide(board: &Board, side: Color, weights: Weights) -> Decision {
    let candidates: Vec<Candidate> = board
        .legal_moves(side)
        .into_iter()
        .map(|mv| {
            let after = board.apply(&mv);
            let features = Features::of(&after, side);
            Candidate {
                u: evaluate(&after, side, &features, weights),
                features,
                mv,
                exact: true,
            }
        })
        .collect();
    let nodes = candidates.len() as u64;
    Decision::new(candidates, nodes, 0)
}

/// Values every legal move of `side` on `board` with minimax `depth` plies deep (at least 1), the
/// leaves scored with U and `weights`. A candidate's `features` are those one ply deep, as in
/// `decide`; its `u` is the minimax value.
pub fn minimax(board: &Board, side: Color, weights: Weights, depth: u32) -> Decision {
    root_minimax(board, side, weights, depth, false)
}

/// `minimax` with quiescence at the depth limit.
pub fn minimax_q(board: &Board, side: Color, weights: Weights, depth: u32) -> Decision {
    root_minimax(board, side, weights, depth, true)
}

fn root_minimax(
    board: &Board,
    side: Color,
    weights: Weights,
    depth: u32,
    quiescence: bool,
) -> Decision {
    assert!(depth >= 1, "minimax needs a depth of at least 1");
    let context = Context {
        root: side,
        weights,
        depth,
        quiescence,
    };
    let mut counter = Counter::unlimited();
    let candidates: Vec<Candidate> = board
        .legal_moves(side)
        .into_iter()
        .map(|mv| {
            let after = board.apply(&mv);
            Candidate {
                features: Features::of(&after, side),
                u: value(&after, side.opponent(), 1, &context, &mut counter),
                mv,
                exact: true,
            }
        })
        .collect();
    Decision::new(candidates, counter.nodes, counter.q_nodes)
}

/// The same decision as `minimax(board, side, weights, depth)` from at most as many nodes: the
/// same `top`, and the same `u` for every move in it, bit for bit.
///
/// Root moves are searched in legal-move order, each with the window (best − 2·`U_EPSILON`, +∞),
/// where best is the highest value found so far. A value above the window's lower end is exact. A
/// value at or below it is an upper bound (fail-soft) and the true value is at most best − 2ε, so
/// that move cannot end up within `U_EPSILON` of the best: best only grows. The margin of 2ε,
/// instead of ε, keeps rounding in `best - ε` from deciding whether a tied move is searched exactly.
pub fn alpha_beta(board: &Board, side: Color, weights: Weights, depth: u32) -> Decision {
    alpha_beta_with(board, side, weights, depth, SearchOptions::default(), None)
}

/// `alpha_beta` with quiescence at the depth limit: the decision of `minimax_q`.
pub fn alpha_beta_q(board: &Board, side: Color, weights: Weights, depth: u32) -> Decision {
    let options = SearchOptions {
        quiescence: true,
        ..SearchOptions::default()
    };
    alpha_beta_with(board, side, weights, depth, options, None)
}

/// `alpha_beta` (or `alpha_beta_q` with `quiescence`) that scores a repetition as a draw: a
/// position below the root that is in `history` (the positions of the game so far, with their
/// side to move) or on the path from the root is worth `DRAW`.
pub fn alpha_beta_rep(
    board: &Board,
    side: Color,
    weights: Weights,
    depth: u32,
    quiescence: bool,
    history: &[(Board, Color)],
) -> Decision {
    let options = SearchOptions {
        quiescence,
        ..SearchOptions::default()
    };
    alpha_beta_with(board, side, weights, depth, options, Some(history))
}

/// Alpha-beta with any `options`, and with `history` (the positions of the game so far) a
/// repetition is a draw, as in `alpha_beta_rep`. With `options.order` the same decision as without,
/// from other nodes.
pub fn alpha_beta_with(
    board: &Board,
    side: Color,
    weights: Weights,
    depth: u32,
    options: SearchOptions,
    history: Option<&[(Board, Color)]>,
) -> Decision {
    assert!(depth >= 1, "alpha-beta needs a depth of at least 1");
    let context = Context {
        root: side,
        weights,
        depth,
        quiescence: options.quiescence,
    };
    let mut search = Search::new(options, history);
    root_alpha_beta(board, &context, &mut search, None)
        .expect("a search without a budget is never interrupted")
}

/// The deepest iteration `iterative_deepening` runs, whatever its limits.
pub const MAX_ID_DEPTH: u32 = 64;

/// When iterative deepening stops. Every limit that is `Some` applies; at least one must be.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Limits {
    /// The deepest iteration, 1 to `MAX_ID_DEPTH`.
    pub depth: Option<u32>,
    /// The most nodes per move, all iterations together.
    pub nodes: Option<u64>,
    /// The most time per move, all iterations together.
    pub time: Option<Duration>,
}

/// The result of `iterative_deepening`.
#[derive(Clone, Debug)]
pub struct Deepening {
    /// The decision of the deepest completed iteration; its `nodes` are those of every iteration,
    /// the interrupted one included.
    pub decision: Decision,
    /// The depth of that iteration.
    pub depth: u32,
}

/// Alpha-beta 1, 2, 3, ... plies deep until `limits` stop it, and the decision of the deepest
/// iteration that was completed. An iteration that runs out of nodes or time is thrown away.
///
/// Depth 1 always completes (it costs one node per legal move), so there is always a decision.
/// The search also stops when an iteration reached no leaf at the depth limit: every line ended
/// in a position without a legal move first, so a deeper iteration would visit the same nodes and
/// make the same decision.
///
/// With `quiescence`, every iteration searches captures past its depth limit (`alpha_beta_q`), and
/// the quiescence nodes count against the node budget.
pub fn iterative_deepening(
    board: &Board,
    side: Color,
    weights: Weights,
    limits: Limits,
    quiescence: bool,
) -> Deepening {
    let options = SearchOptions {
        quiescence,
        ..SearchOptions::default()
    };
    iterative_deepening_with(board, side, weights, limits, options, None)
}

/// `iterative_deepening` whose iterations score a repetition as a draw, as `alpha_beta_rep` does.
pub fn iterative_deepening_rep(
    board: &Board,
    side: Color,
    weights: Weights,
    limits: Limits,
    quiescence: bool,
    history: &[(Board, Color)],
) -> Deepening {
    let options = SearchOptions {
        quiescence,
        ..SearchOptions::default()
    };
    iterative_deepening_with(board, side, weights, limits, options, Some(history))
}

/// Iterative deepening with any `options`, and with `history` a repetition is a draw, as in
/// `iterative_deepening_rep`. With `options.order`, each iteration searches the best move of the
/// one before first, and the killer moves and history heuristic carry over from iteration to
/// iteration; an iteration at depth d makes the decision of `alpha_beta` at depth d, from other
/// nodes.
pub fn iterative_deepening_with(
    board: &Board,
    side: Color,
    weights: Weights,
    limits: Limits,
    options: SearchOptions,
    history: Option<&[(Board, Color)]>,
) -> Deepening {
    assert!(
        limits != Limits::default(),
        "iterative deepening needs a depth, node or time limit"
    );
    let max_depth = limits.depth.unwrap_or(MAX_ID_DEPTH);
    assert!(
        (1..=MAX_ID_DEPTH).contains(&max_depth),
        "iterative deepening needs a depth from 1 to {MAX_ID_DEPTH}"
    );
    let deadline = limits.time.map(|time| Instant::now() + time);
    let mut context = Context {
        root: side,
        weights,
        depth: 1,
        quiescence: options.quiescence,
    };
    let mut search = Search::new(options, history);
    let mut decision = root_alpha_beta(board, &context, &mut search, None)
        .expect("a search without a budget is never interrupted");
    search.counter.max_nodes = limits.nodes;
    search.counter.deadline = deadline;
    while context.depth < max_depth && search.counter.horizon {
        search.counter.horizon = false;
        context.depth += 1;
        // The first of the best moves, in legal order: the move the agent would play without rng.
        let first = options
            .order
            .then(|| decision.top.first().copied())
            .flatten();
        // `?` cannot help here: an interrupted iteration is not an error of this function, so the
        // `Option` is matched and `None` ends the loop.
        match root_alpha_beta(board, &context, &mut search, first) {
            Some(deeper) => decision = deeper,
            None => {
                context.depth -= 1;
                break;
            }
        }
    }
    decision.nodes = search.counter.nodes;
    decision.q_nodes = search.counter.q_nodes;
    Deepening {
        decision,
        depth: context.depth,
    }
}

/// What a search is for: the root side, the weights of U, the depth limit (in plies from the
/// root) and whether to search captures past it.
#[derive(Clone, Copy, Debug)]
struct Context {
    root: Color,
    weights: Weights,
    depth: u32,
    quiescence: bool,
}

impl Context {
    /// Whether a position at `ply` with `moves` to play (not empty) is scored with U, rather than
    /// searched further.
    fn is_leaf(&self, ply: u32, moves: &[Move]) -> bool {
        ply >= self.depth && !(self.quiescence && moves[0].is_capture())
    }

    /// U (`evaluate`) at a leaf where `to_move` has `moves`, from the root side's view.
    fn leaf_value(&self, board: &Board, to_move: Color, moves: &[Move]) -> f64 {
        let others = board.legal_moves(to_move.opponent()).len();
        let (ours, theirs) = if to_move == self.root {
            (moves.len(), others)
        } else {
            (others, moves.len())
        };
        let features = Features::leaf(board, self.root, ours, theirs);
        evaluate(board, self.root, &features, self.weights)
    }

    /// The value of a position at `ply` where `to_move` has no legal move: a loss for `to_move`.
    fn no_move_value(&self, to_move: Color, ply: u32) -> f64 {
        let win = WIN - f64::from(ply - 1);
        if to_move == self.root { -win } else { win }
    }
}

/// The nodes a search has produced, and its budget.
#[derive(Clone, Debug)]
struct Counter {
    /// The positions produced with `apply`, the root not included.
    nodes: u64,
    /// The part of `nodes` beyond the depth limit.
    q_nodes: u64,
    /// No more nodes than this.
    max_nodes: Option<u64>,
    /// No new node from this moment on.
    deadline: Option<Instant>,
    /// Whether a leaf at the depth limit was scored (set, never cleared, by the search).
    horizon: bool,
}

impl Counter {
    fn unlimited() -> Counter {
        Counter {
            nodes: 0,
            q_nodes: 0,
            max_nodes: None,
            deadline: None,
            horizon: false,
        }
    }

    /// Counts one more node at `ply`, or `None` if the budget is spent. The clock is read only
    /// when there is a deadline.
    fn visit(&mut self, ply: u32, context: &Context) -> Option<()> {
        let out_of_nodes = self.max_nodes.is_some_and(|max| self.nodes >= max);
        let out_of_time = self.deadline.is_some_and(|d| Instant::now() >= d);
        if out_of_nodes || out_of_time {
            return None;
        }
        self.nodes += 1;
        if ply > context.depth {
            self.q_nodes += 1;
        }
        Some(())
    }
}

/// The positions a search with repetitions compares against: those of the game, and those on the
/// path from the root to the current node (the root included).
#[derive(Clone, Debug)]
struct Repetitions<'h> {
    history: &'h [(Board, Color)],
    path: Vec<(Board, Color)>,
}

impl<'h> Repetitions<'h> {
    fn new(history: &'h [(Board, Color)]) -> Repetitions<'h> {
        Repetitions {
            history,
            path: Vec::new(),
        }
    }

    /// Whether `board` with `to_move` to move occurred before, in the game or on the path.
    fn seen(&self, board: &Board, to_move: Color) -> bool {
        let position = (*board, to_move);
        self.path.contains(&position) || self.history.contains(&position)
    }
}

/// Move ordering below the root. A killer move is one of the last two quiet moves that caused a
/// cutoff at the same ply; the history heuristic adds up, per side and per from and to square,
/// the remaining depth squared of every cutoff a quiet move caused. Both are kept for all the
/// iterations of one search. Captures are not scored by either: when a side can capture it must,
/// so the moves of a node are all captures or all quiet, and among captures only a promotion is
/// put first.
struct Heuristics {
    killers: Vec<[Option<(Square, Square)>; 2]>,
    /// Indexed by side, from square, to square.
    history: Box<[[[u32; 64]; 64]; 2]>,
}

impl Heuristics {
    fn new() -> Heuristics {
        Heuristics {
            killers: Vec::new(),
            history: Box::new([[[0; 64]; 64]; 2]),
        }
    }

    /// The indices of `moves` (`side` to move at `ply`) in the order to search them: promotions,
    /// then the killers (the newer first), then by history. Equal moves keep the legal order.
    fn order(&self, moves: &[Move], side: Color, ply: u32) -> Vec<usize> {
        let killers = self.killers.get(ply as usize).copied().unwrap_or([None; 2]);
        let score = |mv: &Move| -> (u8, u32) {
            if mv.promotes() {
                return (3, 0);
            }
            if mv.is_capture() {
                return (0, 0);
            }
            let key = Some((mv.from(), mv.to()));
            let class = match () {
                _ if key == killers[0] => 2,
                _ if key == killers[1] => 1,
                _ => 0,
            };
            (
                class,
                self.history[side as usize][mv.from().index()][mv.to().index()],
            )
        };
        let mut order: Vec<usize> = (0..moves.len()).collect();
        // `sort_by_key` is stable: moves with the same score stay in legal order.
        order.sort_by_key(|&i| Reverse(score(&moves[i])));
        order
    }

    /// `mv`, played by `side` at `ply` of a search `depth` plies deep, caused a cutoff.
    fn cutoff(&mut self, mv: &Move, side: Color, ply: u32, depth: u32) {
        if mv.is_capture() {
            return;
        }
        let ply = ply as usize;
        if self.killers.len() <= ply {
            self.killers.resize(ply + 1, [None; 2]);
        }
        let key = Some((mv.from(), mv.to()));
        let killers = &mut self.killers[ply];
        if killers[0] != key {
            killers[1] = killers[0];
            killers[0] = key;
        }
        // A quiet move is searched only above the depth limit, so at least 1 ply remains.
        let remaining = depth.saturating_sub(ply as u32);
        let count = &mut self.history[side as usize][mv.from().index()][mv.to().index()];
        *count = count.saturating_add(remaining * remaining);
    }
}

/// The order in which a node's moves are searched, as indices into its legal moves.
enum MoveOrder {
    Legal(Range<usize>),
    Sorted(std::vec::IntoIter<usize>),
}

impl Iterator for MoveOrder {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        match self {
            MoveOrder::Legal(indices) => indices.next(),
            MoveOrder::Sorted(indices) => indices.next(),
        }
    }
}

/// What a search changes as it goes: the node counter and budget, the positions a repetition is
/// compared against (with repetitions), and the move-ordering heuristics (with ordering).
struct Search<'h> {
    counter: Counter,
    repetitions: Option<Repetitions<'h>>,
    heuristics: Option<Heuristics>,
}

impl<'h> Search<'h> {
    fn new(options: SearchOptions, history: Option<&'h [(Board, Color)]>) -> Search<'h> {
        Search {
            counter: Counter::unlimited(),
            repetitions: history.map(Repetitions::new),
            heuristics: options.order.then(Heuristics::new),
        }
    }
}

/// Alpha-beta at the root, as `alpha_beta` describes, counting in `search.counter`: `None` if the
/// budget ran out. The root moves are searched in legal order, or with `first` that move first and
/// then the others in legal order; `candidates` are in legal order either way. The decision's
/// `nodes` and `q_nodes` are the counter's at the end. With repetitions, a repetition below the
/// root is a draw.
fn root_alpha_beta(
    board: &Board,
    context: &Context,
    search: &mut Search,
    first: Option<usize>,
) -> Option<Decision> {
    let side = context.root;
    if let Some(repetitions) = &mut search.repetitions {
        // An interrupted search leaves its path behind; every search starts from the root.
        repetitions.path.clear();
        repetitions.path.push((*board, side));
    }
    let moves = board.legal_moves(side);
    let order = first
        .into_iter()
        .chain((0..moves.len()).filter(|&i| Some(i) != first));
    let mut best = f64::NEG_INFINITY;
    let mut candidates: Vec<Option<Candidate>> = vec![None; moves.len()];
    for i in order {
        let mv = &moves[i];
        let after = board.apply(mv);
        let alpha = best - 2.0 * U_EPSILON;
        let u = bounded_value(
            &after,
            side.opponent(),
            1,
            (alpha, f64::INFINITY),
            context,
            search,
        )?;
        best = best.max(u);
        candidates[i] = Some(Candidate {
            features: Features::of(&after, side),
            u,
            exact: u > alpha,
            mv: mv.clone(),
        });
    }
    let candidates = candidates
        .into_iter()
        .map(|c| c.expect("every root move is searched"))
        .collect();
    let counter = &search.counter;
    Some(Decision::new(candidates, counter.nodes, counter.q_nodes))
}

/// The minimax value for the root side of `board` at ply `ply` (the root is ply 0), with `to_move`
/// to move. Counts `board` in `counter`.
fn value(board: &Board, to_move: Color, ply: u32, context: &Context, counter: &mut Counter) -> f64 {
    counter.visit(ply, context).expect("minimax has no budget");
    let moves = board.legal_moves(to_move);
    if moves.is_empty() {
        return context.no_move_value(to_move, ply);
    }
    if context.is_leaf(ply, &moves) {
        return context.leaf_value(board, to_move, &moves);
    }
    let children = moves.iter().map(|mv| {
        value(
            &board.apply(mv),
            to_move.opponent(),
            ply + 1,
            context,
            counter,
        )
    });
    if to_move == context.root {
        children.fold(f64::NEG_INFINITY, f64::max)
    } else {
        children.fold(f64::INFINITY, f64::min)
    }
}

/// `value` searched with alpha-beta in the window `(alpha, beta)`, fail-soft: a result strictly
/// inside the window is the exact minimax value; a result `<= alpha` is an upper bound of it and a
/// result `>= beta` a lower bound. Counts every board it visits in `search.counter`, as `value`
/// does, and gives `None` as soon as the counter's budget runs out. With repetitions, a position
/// seen before is worth `DRAW`; with heuristics, the moves are ordered.
fn bounded_value(
    board: &Board,
    to_move: Color,
    ply: u32,
    (mut alpha, mut beta): (f64, f64),
    context: &Context,
    search: &mut Search,
) -> Option<f64> {
    search.counter.visit(ply, context)?;
    if (search.repetitions.as_ref()).is_some_and(|r| r.seen(board, to_move)) {
        return Some(DRAW);
    }
    let moves = board.legal_moves(to_move);
    if moves.is_empty() {
        return Some(context.no_move_value(to_move, ply));
    }
    if context.is_leaf(ply, &moves) {
        search.counter.horizon = true;
        return Some(context.leaf_value(board, to_move, &moves));
    }
    let maximising = to_move == context.root;
    let mut best = if maximising {
        f64::NEG_INFINITY
    } else {
        f64::INFINITY
    };
    if let Some(repetitions) = &mut search.repetitions {
        repetitions.path.push((*board, to_move));
    }
    let order = match &search.heuristics {
        Some(heuristics) => MoveOrder::Sorted(heuristics.order(&moves, to_move, ply).into_iter()),
        None => MoveOrder::Legal(0..moves.len()),
    };
    for i in order {
        let mv = &moves[i];
        let v = bounded_value(
            &board.apply(mv),
            to_move.opponent(),
            ply + 1,
            (alpha, beta),
            context,
            search,
        )?;
        if maximising {
            best = best.max(v);
            alpha = alpha.max(v);
        } else {
            best = best.min(v);
            beta = beta.min(v);
        }
        if alpha >= beta {
            if let Some(heuristics) = &mut search.heuristics {
                heuristics.cutoff(mv, to_move, ply, context.depth);
            }
            break;
        }
    }
    if let Some(repetitions) = &mut search.repetitions {
        repetitions.path.pop();
    }
    Some(best)
}

impl Decision {
    /// A decision over `candidates`: `top` holds the ones within `U_EPSILON` of the highest U.
    fn new(candidates: Vec<Candidate>, nodes: u64, q_nodes: u64) -> Decision {
        // f64 has no total order (NaN), so the highest U is a fold rather than `max()`.
        let best = candidates
            .iter()
            .map(|c| c.u)
            .fold(f64::NEG_INFINITY, f64::max);
        let top = (0..candidates.len())
            .filter(|&i| best - candidates[i].u <= U_EPSILON)
            .collect();
        Decision {
            candidates,
            top,
            nodes,
            q_nodes,
        }
    }

    /// The index in `candidates` (so also in the legal move list) of the move to play, or `None`
    /// if there is no legal move. Without `rng` it is the first move with the highest U. With
    /// `rng` it is picked as agent.js does, `top[Math.floor(rnd() * top.length)]`, which draws a
    /// number even when one move is best.
    pub fn pick(&self, rng: Option<&mut Mulberry32>) -> Option<usize> {
        if self.top.is_empty() {
            return None;
        }
        let i = match rng {
            Some(rng) => rng.next_index(self.top.len()),
            None => 0,
        };
        Some(self.top[i])
    }

    /// The move to play, or `None` if there is no legal move: the candidate that `pick` gives.
    pub fn choose(&self, rng: Option<&mut Mulberry32>) -> Option<&Move> {
        self.pick(rng).map(|i| &self.candidates[i].mv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A decision over the first `n` moves of the start position, all with the same U.
    fn tied(n: usize) -> Decision {
        let features = Features {
            men: 0,
            kings: 0,
            mob: 0,
            win: false,
        };
        let candidates: Vec<Candidate> = Board::start()
            .legal_moves(Color::White)
            .into_iter()
            .take(n)
            .map(|mv| Candidate {
                mv,
                features,
                u: 0.0,
                exact: true,
            })
            .collect();
        Decision {
            top: (0..candidates.len()).collect(),
            nodes: candidates.len() as u64,
            q_nodes: 0,
            candidates,
        }
    }

    #[test]
    fn choose_picks_top_at_floor_of_rnd_times_len() {
        let decision = tied(3);
        assert_eq!(decision.choose(None), Some(&decision.candidates[0].mv));
        // Seed 2026 draws 0.455... first, and floor(0.455... * 3) = 1.
        let mut rng = Mulberry32::new(2026);
        assert_eq!(
            decision.choose(Some(&mut rng)),
            Some(&decision.candidates[1].mv)
        );
    }

    #[test]
    fn pick_gives_the_index_of_the_chosen_move() {
        let decision = tied(3);
        assert_eq!(decision.pick(None), Some(0));
        // Seed 2026 draws 0.455... and then 0.308...: floor(0.455 * 3) = 1, floor(0.308 * 3) = 0.
        let mut rng = Mulberry32::new(2026);
        assert_eq!(decision.pick(Some(&mut rng)), Some(1));
        assert_eq!(decision.pick(Some(&mut rng)), Some(0));
    }

    #[test]
    fn choose_draws_a_number_even_with_one_top_move() {
        let decision = tied(1);
        let mut rng = Mulberry32::new(1);
        let mut reference = Mulberry32::new(1);
        decision.choose(Some(&mut rng));
        reference.next_f64();
        assert_eq!(rng.next_u32(), reference.next_u32());
    }

    #[test]
    fn minimax_one_ply_is_decide() {
        let board = Board::start();
        for side in [Color::White, Color::Black] {
            let one_ply = decide(&board, side, Weights::TUNED);
            let mm1 = minimax(&board, side, Weights::TUNED, 1);
            let values = |d: &Decision| d.candidates.iter().map(|c| c.u).collect::<Vec<_>>();
            assert_eq!(values(&mm1), values(&one_ply));
            assert_eq!(mm1.top, one_ply.top);
            assert_eq!(mm1.nodes, one_ply.nodes);
        }
    }

    #[test]
    fn a_win_at_once_is_worth_win_at_any_depth() {
        // Black's last man is on d5 and White's man on d4 takes it: a win at ply 1, worth WIN even
        // when the search looks three plies deep (tests/minimax.rs has a win at ply 3, WIN - 2).
        let board: Board =
            "......../......../......../...b..../...w..../......../......../W......."
                .parse()
                .unwrap();
        let mm3 = minimax(&board, Color::White, Weights::TUNED, 3);
        let capture = mm3.candidates.iter().find(|c| c.mv.is_capture()).unwrap();
        assert_eq!(capture.u, WIN);
        assert_eq!(mm3.top.len(), 1);
        assert!(mm3.candidates[mm3.top[0]].mv.is_capture());
    }

    /// White: king a2, man a1. Black: king h8, man h7 (`KINGS` in tests/arena.rs).
    const KINGS: &str = ".......B/.......b/......../......../......../......../W......./w.......";

    #[test]
    fn a_position_on_the_path_is_a_draw_and_the_path_is_restored() {
        let board: Board = KINGS.parse().unwrap();
        let context = Context {
            root: Color::White,
            weights: Weights::TUNED,
            depth: 3,
            quiescence: false,
        };
        let mut search = Search::new(SearchOptions::default(), Some(&[][..]));
        let path = |search: &Search| search.repetitions.as_ref().unwrap().path.clone();
        search
            .repetitions
            .as_mut()
            .unwrap()
            .path
            .push((board, Color::Black));
        let window = (f64::NEG_INFINITY, f64::INFINITY);
        let draw = bounded_value(&board, Color::Black, 1, window, &context, &mut search);
        // A repetition is a node, but nothing below it is searched.
        assert_eq!((draw, search.counter.nodes), (Some(DRAW), 1));
        // The same side to move is part of the position.
        let searched = bounded_value(&board, Color::White, 1, window, &context, &mut search);
        assert_ne!(searched, Some(DRAW));
        assert!(search.counter.nodes > 2);
        assert_eq!(path(&search), [(board, Color::Black)]);
        // A root search leaves only the root on the path.
        root_alpha_beta(&board, &context, &mut search, None).unwrap();
        assert_eq!(path(&search), [(board, Color::White)]);
    }

    #[test]
    fn heuristics_put_promotions_then_killers_then_history_first() {
        // White men on b2, d7 and g3: d7-d8 promotes. Black king on a8 keeps Black in the game.
        let board: Board =
            "B......./...w..../......../......../......../......w./.w....../........"
                .parse()
                .unwrap();
        let moves = board.legal_moves(Color::White);
        let index = |name: &str| moves.iter().position(|m| m.to_string() == name).unwrap();
        let mut heuristics = Heuristics::new();
        let legal: Vec<usize> = (0..moves.len()).collect();
        let promotion = index("d7-d8=K");
        let mut expected = vec![promotion];
        expected.extend(legal.iter().filter(|&&i| i != promotion));
        assert_eq!(heuristics.order(&moves, Color::White, 2), expected);
        // g3-g4 cut off at ply 2 of a 4-ply search: a killer at ply 2 and 4 in the history.
        let killer = index("g3-g4");
        heuristics.cutoff(&moves[killer], Color::White, 2, 4);
        assert_eq!(
            heuristics.history[0][moves[killer].from().index()][moves[killer].to().index()],
            4
        );
        let order = heuristics.order(&moves, Color::White, 2);
        assert_eq!(order[..2], [promotion, killer]);
        // At another ply it is not a killer, but its history still puts it before the rest.
        let order = heuristics.order(&moves, Color::White, 3);
        assert_eq!(order[..2], [promotion, killer]);
        // A newer killer comes first; the older stays second.
        let newer = index("b2-b3");
        heuristics.cutoff(&moves[newer], Color::White, 2, 4);
        let order = heuristics.order(&moves, Color::White, 2);
        assert_eq!(order[..3], [promotion, newer, killer]);
        // Black's history is its own.
        assert!(heuristics.history[1].iter().flatten().all(|&n| n == 0));
    }

    #[test]
    fn no_legal_move_no_choice_and_no_draw() {
        let decision = tied(0);
        let mut rng = Mulberry32::new(1);
        assert_eq!(decision.choose(Some(&mut rng)), None);
        assert_eq!(rng.next_u32(), Mulberry32::new(1).next_u32());
    }
}
