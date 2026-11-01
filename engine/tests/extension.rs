//! The referee of the single-reply extension (`x`). On the 141 positions of testdata/decisions.json
//! (with their weights) and the 400 of testdata/movegen.json (with the tuned weights), and again with
//! the texel1 weights:
//!
//! - `abNx` makes the decision of a minimax without pruning that makes the same extension. That
//!   minimax is written here, sharing no search code with the engine: only the move generator, `apply`
//!   and `static_value`, which have referees of their own. Values must match bit for bit.
//! - Ordering, a strict table and principal variation search stay pure speed-ups with `x`.
//! - Every search ends: on the positions of random games, a line is at most depth + 31 plies long.

mod common;

use std::collections::HashSet;
use std::sync::Mutex;

use common::{compare, positions};
use engine::{
    AgentSpec, Board, Color, DRAW, Decision, DrawRules, Enter, Limits, MatchPlan, Move, Mulberry32,
    Schedule, SearchOptions, Table, TableOptions, Tracer, U_EPSILON, ValueBound, WIN, Weights,
    alpha_beta_traced, alpha_beta_with, iterative_deepening_with, play_match, static_value,
};

const STRICT: TableOptions = TableOptions {
    mb: 1,
    strict: true,
};

/// A minimax with the extension, from the rules of docs: the depth limit is per line.
struct Minimax {
    root: Color,
    weights: Weights,
    quiescence: bool,
    extend: bool,
    /// With repetitions: the positions on the path from the root, the root included.
    path: Option<Vec<(Board, Color)>>,
    extended: u64,
}

impl Minimax {
    fn value(&mut self, board: &Board, to_move: Color, ply: u32, left: i32) -> f64 {
        if self
            .path
            .as_ref()
            .is_some_and(|p| p.contains(&(*board, to_move)))
        {
            return DRAW;
        }
        let moves = board.legal_moves(to_move);
        if moves.is_empty() {
            let win = WIN - f64::from(ply - 1);
            return if to_move == self.root { -win } else { win };
        }
        if left <= 0 && !(self.quiescence && moves[0].is_capture()) {
            return static_value(board, to_move, self.root, self.weights);
        }
        let forced = self.extend && left >= 1 && moves.len() == 1 && moves[0].is_capture();
        self.extended += u64::from(forced);
        let child_left = if forced { left } else { left - 1 };
        if let Some(path) = &mut self.path {
            path.push((*board, to_move));
        }
        let values = moves
            .iter()
            .map(|mv| self.value(&board.apply(mv), to_move.opponent(), ply + 1, child_left))
            .collect::<Vec<f64>>();
        if let Some(path) = &mut self.path {
            path.pop();
        }
        let start = if to_move == self.root {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
        values.into_iter().fold(start, |a, b| {
            if to_move == self.root {
                a.max(b)
            } else {
                a.min(b)
            }
        })
    }

    /// The value of every legal move at the root, which never extends.
    fn root_values(&mut self, board: &Board, depth: u32) -> Vec<f64> {
        if let Some(path) = &mut self.path {
            path.clear();
            path.push((*board, self.root));
        }
        let root = self.root;
        board
            .legal_moves(root)
            .iter()
            .map(|mv| self.value(&board.apply(mv), root.opponent(), 1, depth as i32 - 1))
            .collect()
    }
}

/// The differences between `decision` and the minimax `values`.
fn against_minimax(label: &str, decision: &Decision, values: &[f64]) -> Vec<String> {
    let mut errors = Vec::new();
    let best = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let top: Vec<usize> = (0..values.len())
        .filter(|&i| best - values[i] <= U_EPSILON)
        .collect();
    if decision.top != top {
        errors.push(format!("{label}: top {:?}, minimax {top:?}", decision.top));
    }
    if decision.candidates.len() != values.len() {
        errors.push(format!("{label}: number of moves differs"));
        return errors;
    }
    for (c, &v) in decision.candidates.iter().zip(values) {
        let what = format!("{label}: {}", c.mv);
        if c.exact && c.u.to_bits() != v.to_bits() {
            errors.push(format!("{what}: exact value {}, minimax {v}", c.u));
        }
        if !c.exact && (c.u < v || best - c.u <= U_EPSILON) {
            errors.push(format!("{what}: bound {}, minimax {v}, best {best}", c.u));
        }
    }
    errors
}

fn same_decisions_as_minimax_at(depth: u32) {
    let (mut errors, mut extended, mut changed) = (Vec::new(), 0, 0);
    for (label, board, side, weights) in positions() {
        if board.legal_moves(side).is_empty() {
            continue;
        }
        for weights in [weights, Weights::TEXEL1] {
            for quiescence in [false, true] {
                for repetition in [false, true] {
                    let options = SearchOptions {
                        quiescence,
                        extend: true,
                        ..SearchOptions::default()
                    };
                    let history = repetition.then_some(&[][..]);
                    let ab = alpha_beta_with(&board, side, weights, depth, options, history, None);
                    let mut minimax = Minimax {
                        root: side,
                        weights,
                        quiescence,
                        extend: true,
                        path: repetition.then(Vec::new),
                        extended: 0,
                    };
                    let values = minimax.root_values(&board, depth);
                    extended += minimax.extended;
                    let what = format!(
                        "depth {depth}, {label}, {weights}, q {quiescence}, r {repetition}"
                    );
                    errors.extend(against_minimax(&what, &ab, &values));
                    let plain = SearchOptions {
                        extend: false,
                        ..options
                    };
                    let without =
                        alpha_beta_with(&board, side, weights, depth, plain, history, None);
                    changed += u32::from(without.top != ab.top);
                }
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    // Depth 1 has no node to extend: the root's children are at the limit. Above it the referee must
    // have seen the rule at work, or it checked nothing.
    if depth == 1 {
        assert_eq!((extended, changed), (0, 0));
    } else {
        assert!(extended > 0, "no extension at depth {depth}");
    }
    if depth >= 3 {
        assert!(changed > 0, "x changed no decision at depth {depth}");
    }
}

#[test]
fn same_decisions_as_minimax_at_depth_1() {
    same_decisions_as_minimax_at(1);
}

#[test]
fn same_decisions_as_minimax_at_depth_2() {
    same_decisions_as_minimax_at(2);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn same_decisions_as_minimax_at_depth_3() {
    same_decisions_as_minimax_at(3);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn same_decisions_as_minimax_at_depth_4() {
    same_decisions_as_minimax_at(4);
}

/// The draw rules to check: none, and counts with and without progress pressure.
const DRAWS: [Option<DrawRules>; 4] = [
    None,
    Some(DrawRules {
        quiet: 7,
        progress: true,
    }),
    Some(DrawRules {
        quiet: 45,
        progress: false,
    }),
    Some(DrawRules {
        quiet: 45,
        progress: true,
    }),
];

/// `abNox`, `abNotx:tt=strict` and both with `p` against `abNx`, with q, r and f on and off.
fn speed_ups_stay_speed_ups_at(depth: u32, step: usize) {
    let mut errors = Vec::new();
    for (label, board, side, weights) in positions().into_iter().step_by(step) {
        for weights in [weights, Weights::TEXEL1] {
            for quiescence in [false, true] {
                for history in [None, Some(&[][..])] {
                    for draws in DRAWS {
                        let plain = SearchOptions {
                            quiescence,
                            draws,
                            extend: true,
                            ..SearchOptions::default()
                        };
                        let p = alpha_beta_with(&board, side, weights, depth, plain, history, None);
                        let what =
                            format!("depth {depth}, {label}, {weights}, {plain:?} {history:?}");
                        for (order, table, pvs) in [
                            (true, false, false),
                            (true, true, false),
                            (false, false, true),
                            (true, false, true),
                            (true, true, true),
                        ] {
                            let options = SearchOptions {
                                order,
                                pvs,
                                ..plain
                            };
                            let mut strict = table.then(|| Table::new(STRICT));
                            let o = alpha_beta_with(
                                &board,
                                side,
                                weights,
                                depth,
                                options,
                                history,
                                strict.as_mut(),
                            );
                            let what = format!("{what}, o {order}, t {table}, p {pvs}");
                            errors.extend(compare(&what, &p, &o));
                        }
                    }
                }
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn speed_ups_stay_speed_ups_at_depth_1_to_2() {
    speed_ups_stay_speed_ups_at(1, 1);
    speed_ups_stay_speed_ups_at(2, 1);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn speed_ups_stay_speed_ups_at_depth_3() {
    speed_ups_stay_speed_ups_at(3, 1);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn speed_ups_stay_speed_ups_at_depth_4() {
    speed_ups_stay_speed_ups_at(4, 1);
}

#[test]
fn iterative_deepening_decides_as_alpha_beta_at_its_depth() {
    let mut errors = Vec::new();
    for (label, board, side, weights) in positions().into_iter().step_by(7) {
        for draws in [None, DRAWS[1]] {
            let plain = SearchOptions {
                quiescence: true,
                order: true,
                draws,
                extend: true,
                ..SearchOptions::default()
            };
            let limits = Limits {
                depth: Some(3),
                ..Limits::default()
            };
            let id =
                iterative_deepening_with(&board, side, weights, limits, plain, Some(&[]), None);
            let ab = alpha_beta_with(&board, side, weights, 3, plain, Some(&[]), None);
            errors.extend(compare(&label, &ab, &id.decision));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn at_depth_1_x_changes_nothing() {
    for (label, board, side, weights) in positions() {
        for quiescence in [false, true] {
            let plain = SearchOptions {
                quiescence,
                ..SearchOptions::default()
            };
            let extend = SearchOptions {
                extend: true,
                ..plain
            };
            let a = alpha_beta_with(&board, side, weights, 1, plain, None, None);
            let b = alpha_beta_with(&board, side, weights, 1, extend, None, None);
            assert_eq!((a.nodes, a.q_nodes), (b.nodes, b.q_nodes), "{label}");
            let bits = |d: &Decision| {
                d.candidates
                    .iter()
                    .map(|c| c.u.to_bits())
                    .collect::<Vec<_>>()
            };
            assert_eq!(bits(&a), bits(&b), "{label}");
        }
    }
}

/// The root's single move is not extended: its child has one ply less, as without `x`.
#[test]
fn the_root_does_not_extend() {
    for (label, board, side, weights) in positions() {
        let moves = board.legal_moves(side);
        if moves.len() != 1 || !moves[0].is_capture() {
            continue;
        }
        let extend = SearchOptions {
            extend: true,
            ..SearchOptions::default()
        };
        let mut lines = Lines::default();
        alpha_beta_traced(
            &board, side, weights, 2, extend, None, None, None, &mut lines,
        );
        assert!(lines.first_ply_draft.iter().all(|&d| d == 1), "{label}");
    }
}

/// The games of `a` against `b`, with their moves.
fn games(a: &str, b: &str, n: u32) -> Vec<Vec<Move>> {
    let (a, b): (AgentSpec, AgentSpec) = (a.parse().unwrap(), b.parse().unwrap());
    let plan = MatchPlan {
        games: n,
        seed: 1,
        schedule: Schedule::Paired,
        threads: 2,
        sprt: None,
    };
    play_match(&|| a.build(), &|| b.build(), &plan)
        .unwrap()
        .games
        .into_iter()
        .map(|g| g.record.moves)
        .collect()
}

#[test]
fn x_changes_games_and_ab1x_is_ab1() {
    assert_eq!(games("ab1x", "tuned", 20), games("ab1", "tuned", 20));
    assert_ne!(games("ab3qx", "tuned", 20), games("ab3q", "tuned", 20));
}

#[test]
fn deepening_with_a_depth_limit_plays_alpha_betas_games() {
    let plain = games("ab3qrx", "ab2q", 10);
    for a in [
        "idqrx:d3",
        "idqrox:d3",
        "idqrotx:d3:tt=strict",
        "idqrotpx:d3:tt=strict",
    ] {
        assert_eq!(games(a, "ab2q", 10), plain, "{a}");
    }
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn deepening_with_a_depth_limit_plays_alpha_betas_games_at_depth_5() {
    let plain = games("ab5qrfx", "ab3q", 10);
    for a in [
        "ab5qrfpx",
        "ab5qrotfx:tt=strict",
        "idqrfx:d5",
        "idqrofx:d5",
        "idqrotfx:d5:tt=strict",
        "idqrotfpx:d5:tt=strict",
    ] {
        assert_eq!(games(a, "ab3q", 10), plain, "{a}");
    }
}

/// The deepest line of a search, the nodes extended, and the draft of the root's children.
#[derive(Default)]
struct Lines {
    line: Vec<Option<Move>>,
    deepest: Vec<Option<Move>>,
    extended: u64,
    first_ply_draft: Vec<u32>,
}

impl Tracer for Lines {
    const ON: bool = true;
    fn enter(&mut self, node: &Enter) {
        self.line.push(node.mv.cloned());
        self.extended += u64::from(node.extended);
        if node.ply == 1 {
            self.first_ply_draft.push(node.draft);
        }
        if self.line.len() > self.deepest.len() {
            self.deepest.clone_from(&self.line);
        }
    }
    fn exit(&mut self, _value: f64, _bound: ValueBound, _cutoff: bool) {
        self.line.pop();
    }
}

/// Its length in plies, its position and side to move, and its moves.
type DeepestLine = (usize, Board, Color, Vec<Move>);

/// The positions of `games` games of random moves, each once, by the arena's rules (without the third
/// repetition).
fn random_positions(games: u32) -> Vec<(Board, Color)> {
    let mut seen = HashSet::new();
    let mut all = Vec::new();
    let mut rng = Mulberry32::new(1);
    for _ in 0..games {
        let (mut board, mut side, mut quiet) = (Board::start(), Color::White, 0);
        for _ in 0..400 {
            let moves = board.legal_moves(side);
            let text = board.to_string();
            let count = |pieces: [char; 2]| text.chars().filter(|c| pieces.contains(c)).count();
            let one_each = count(['w', 'W']) == 1 && count(['b', 'B']) == 1;
            if moves.is_empty() || one_each || quiet >= 50 {
                break;
            }
            if seen.insert((board.to_string(), side)) {
                all.push((board, side));
            }
            let mv = &moves[rng.next_index(moves.len())];
            quiet = if mv.is_capture() { 0 } else { quiet + 1 };
            board = board.apply(mv);
            side = side.opponent();
        }
    }
    all
}

#[test]
#[ignore = "slow; run with --release -- --ignored --nocapture"]
fn searches_end_on_the_positions_of_random_games() {
    const DEPTH: u32 = 6;
    let positions = random_positions(200);
    let next = Mutex::new(positions.iter());
    let deepest: Mutex<Vec<DeepestLine>> = Mutex::new(Vec::new());
    let extended = Mutex::new(0u64);
    std::thread::scope(|scope| {
        for _ in 0..6 {
            scope.spawn(|| {
                let mut table = Table::new(TableOptions::default());
                loop {
                    let Some(&(board, side)) = next.lock().unwrap().next() else {
                        break;
                    };
                    table.clear();
                    let options = SearchOptions {
                        quiescence: true,
                        order: true,
                        extend: true,
                        ..SearchOptions::default()
                    };
                    let mut lines = Lines::default();
                    alpha_beta_traced(
                        &board,
                        side,
                        Weights::TEXEL1,
                        DEPTH,
                        options,
                        Some(&[]),
                        Some(&mut table),
                        None,
                        &mut lines,
                    );
                    *extended.lock().unwrap() += lines.extended;
                    let ply = lines.deepest.len() - 1;
                    let mut best = deepest.lock().unwrap();
                    if best.first().is_none_or(|b| ply > b.0) {
                        let line = lines.deepest.iter().flatten().cloned().collect();
                        *best = vec![(ply, board, side, line)];
                    }
                }
            });
        }
    });
    let deepest = deepest.into_inner().unwrap();
    let (ply, board, side, line) = &deepest[0];
    let line: Vec<String> = line.iter().map(|m| m.to_string()).collect();
    println!(
        "{} positions, {} extended nodes, deepest line {ply} plies from {board} with {side:?} to move: {}",
        positions.len(),
        extended.into_inner().unwrap(),
        line.join(" ")
    );
    // Quiet moves spend draft, and a line holds at most 31 captures.
    assert!(*ply as u32 <= DEPTH + 31, "{ply}");
    assert!(*ply as u32 > DEPTH, "no line went past the depth");
}
