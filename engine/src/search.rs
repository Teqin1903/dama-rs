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
//! Iterative deepening has no move ordering yet, so its iterations do not depend on each other: a
//! completed iteration at depth d is exactly `alpha_beta` at depth d.
//!
//! Quiescence (`minimax_q`, `alpha_beta_q`, and iterative deepening with `quiescence`) changes only
//! the depth limit: a position there whose side to move must capture is not scored, and every
//! capture is searched instead, ply after ply, until a position without a capture (or without a
//! move) is reached. Capturing is mandatory, so the side to move cannot decline ("stand pat") as in
//! chess, and since every capture removes a piece, the captures run out by themselves. The nodes
//! beyond the depth limit are counted in `nodes` and, separately, in `q_nodes`.

use std::time::{Duration, Instant};

use crate::board::{Board, Color};
use crate::eval::{Features, U_EPSILON, WIN, Weights};
use crate::movegen::Move;
use crate::rng::Mulberry32;

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

/// Evaluates every legal move of `side` on `board` with U.
pub fn decide(board: &Board, side: Color, weights: Weights) -> Decision {
    let candidates: Vec<Candidate> = board
        .legal_moves(side)
        .into_iter()
        .map(|mv| {
            let features = Features::of(&board.apply(&mv), side);
            Candidate {
                u: features.u(weights),
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
    unlimited_alpha_beta(board, side, weights, depth, false)
}

/// `alpha_beta` with quiescence at the depth limit: the decision of `minimax_q`.
pub fn alpha_beta_q(board: &Board, side: Color, weights: Weights, depth: u32) -> Decision {
    unlimited_alpha_beta(board, side, weights, depth, true)
}

fn unlimited_alpha_beta(
    board: &Board,
    side: Color,
    weights: Weights,
    depth: u32,
    quiescence: bool,
) -> Decision {
    assert!(depth >= 1, "alpha-beta needs a depth of at least 1");
    let context = Context {
        root: side,
        weights,
        depth,
        quiescence,
    };
    root_alpha_beta(board, &context, &mut Counter::unlimited())
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
        quiescence,
    };
    let mut counter = Counter::unlimited();
    let mut decision = root_alpha_beta(board, &context, &mut counter)
        .expect("a search without a budget is never interrupted");
    counter.max_nodes = limits.nodes;
    counter.deadline = deadline;
    while context.depth < max_depth && counter.horizon {
        counter.horizon = false;
        context.depth += 1;
        // `?` cannot help here: an interrupted iteration is not an error of this function, so the
        // `Option` is matched and `None` ends the loop.
        match root_alpha_beta(board, &context, &mut counter) {
            Some(deeper) => decision = deeper,
            None => {
                context.depth -= 1;
                break;
            }
        }
    }
    decision.nodes = counter.nodes;
    decision.q_nodes = counter.q_nodes;
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

    /// U at a leaf where `to_move` has `moves`, from the root side's view.
    fn leaf_value(&self, board: &Board, to_move: Color, moves: &[Move]) -> f64 {
        let others = board.legal_moves(to_move.opponent()).len();
        let (ours, theirs) = if to_move == self.root {
            (moves.len(), others)
        } else {
            (others, moves.len())
        };
        Features::leaf(board, self.root, ours, theirs).u(self.weights)
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

/// Alpha-beta at the root, as `alpha_beta` describes, counting in `counter`: `None` if the budget
/// ran out. The decision's `nodes` and `q_nodes` are the counter's at the end.
fn root_alpha_beta(board: &Board, context: &Context, counter: &mut Counter) -> Option<Decision> {
    let side = context.root;
    let mut best = f64::NEG_INFINITY;
    let mut candidates = Vec::new();
    for mv in board.legal_moves(side) {
        let after = board.apply(&mv);
        let alpha = best - 2.0 * U_EPSILON;
        let u = bounded_value(
            &after,
            side.opponent(),
            1,
            (alpha, f64::INFINITY),
            context,
            counter,
        )?;
        best = best.max(u);
        candidates.push(Candidate {
            features: Features::of(&after, side),
            u,
            exact: u > alpha,
            mv,
        });
    }
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
/// result `>= beta` a lower bound. Counts every board it visits in `counter`, as `value` does, and
/// gives `None` as soon as the counter's budget runs out.
fn bounded_value(
    board: &Board,
    to_move: Color,
    ply: u32,
    (mut alpha, mut beta): (f64, f64),
    context: &Context,
    counter: &mut Counter,
) -> Option<f64> {
    counter.visit(ply, context)?;
    let moves = board.legal_moves(to_move);
    if moves.is_empty() {
        return Some(context.no_move_value(to_move, ply));
    }
    if context.is_leaf(ply, &moves) {
        counter.horizon = true;
        return Some(context.leaf_value(board, to_move, &moves));
    }
    let maximising = to_move == context.root;
    let mut best = if maximising {
        f64::NEG_INFINITY
    } else {
        f64::INFINITY
    };
    for mv in &moves {
        let v = bounded_value(
            &board.apply(mv),
            to_move.opponent(),
            ply + 1,
            (alpha, beta),
            context,
            counter,
        )?;
        if maximising {
            best = best.max(v);
            alpha = alpha.max(v);
        } else {
            best = best.min(v);
            beta = beta.min(v);
        }
        if alpha >= beta {
            break;
        }
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

    #[test]
    fn no_legal_move_no_choice_and_no_draw() {
        let decision = tied(0);
        let mut rng = Mulberry32::new(1);
        assert_eq!(decision.choose(Some(&mut rng)), None);
        assert_eq!(rng.next_u32(), Mulberry32::new(1).next_u32());
    }
}
