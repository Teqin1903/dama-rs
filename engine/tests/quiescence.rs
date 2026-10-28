//! The quiescence referee, on the 141 positions of testdata/decisions.json (with their weights)
//! and the 400 of testdata/movegen.json (with the tuned weights):
//!
//! - alpha-beta with quiescence makes the decision of minimax with quiescence at the same depth,
//!   from at most as many nodes (the checks of tests/alpha_beta.rs);
//! - when no position at the depth limit has a capture to play, quiescence changes nothing;
//! - iterative deepening with quiescence and a depth limit N makes the decision of `alpha_beta_q`.

mod common;

use engine::{
    Board, Color, Decision, Limits, U_EPSILON, Weights, alpha_beta, alpha_beta_q,
    iterative_deepening, minimax_q,
};
use serde::Deserialize;

/// A position of decisions.json or movegen.json. Only decisions.json has `name` and `weights`.
#[derive(Deserialize)]
struct Position {
    name: Option<String>,
    board: String,
    side: String,
    weights: Option<JsWeights>,
}

#[derive(Deserialize)]
struct JsWeights {
    man: f64,
    king: f64,
    mob: f64,
}

/// Every position of both files, with a label for messages, the side to move and its weights.
fn positions() -> Vec<(String, Board, Color, Weights)> {
    let mut all = Vec::new();
    for file in ["decisions.json", "movegen.json"] {
        let entries: Vec<Position> = serde_json::from_str(&common::read_testdata(file)).unwrap();
        for (i, p) in entries.into_iter().enumerate() {
            let weights = p.weights.map_or(Weights::TUNED, |w| Weights {
                man: w.man,
                king: w.king,
                mob: w.mob,
            });
            let label = format!("{file} #{i} {}", p.name.unwrap_or_default());
            all.push((
                label,
                p.board.parse().unwrap(),
                common::side(&p.side),
                weights,
            ));
        }
    }
    assert_eq!(all.len(), 541);
    all
}

/// Checks alpha-beta against minimax on one position (as in tests/alpha_beta.rs) and returns the
/// messages of what differs.
fn compare(label: &str, mm: &Decision, ab: &Decision) -> Vec<String> {
    let mut errors = Vec::new();
    if ab.top != mm.top {
        errors.push(format!("{label}: top {:?}, minimax {:?}", ab.top, mm.top));
    }
    if ab.nodes > mm.nodes || ab.q_nodes > ab.nodes {
        errors.push(format!(
            "{label}: {} nodes ({} quiescence), minimax {}",
            ab.nodes, ab.q_nodes, mm.nodes
        ));
    }
    let best = mm
        .candidates
        .iter()
        .map(|c| c.u)
        .fold(f64::NEG_INFINITY, f64::max);
    for (i, (a, m)) in ab.candidates.iter().zip(&mm.candidates).enumerate() {
        let what = format!("{label}: move {} ({i})", m.mv);
        if a.mv != m.mv || a.features != m.features {
            errors.push(format!("{what}: move or features differ"));
        }
        if mm.top.contains(&i) && !a.exact {
            errors.push(format!("{what}: a best move without an exact value"));
        }
        if a.exact && a.u != m.u {
            errors.push(format!("{what}: exact value {}, minimax {}", a.u, m.u));
        }
        if !a.exact && !(a.u >= m.u && best - a.u > U_EPSILON) {
            errors.push(format!(
                "{what}: bound {}, minimax {}, best {best}",
                a.u, m.u
            ));
        }
    }
    if ab.candidates.len() != mm.candidates.len() {
        errors.push(format!("{label}: number of candidates differs"));
    }
    errors
}

/// Whether two decisions choose alike: the same moves, the same `top`, and bit for bit the same
/// values and exactness.
fn same_choice(a: &Decision, b: &Decision) -> bool {
    a.top == b.top
        && a.candidates.len() == b.candidates.len()
        && a.candidates
            .iter()
            .zip(&b.candidates)
            .all(|(x, y)| x.mv == y.mv && x.u.to_bits() == y.u.to_bits() && x.exact == y.exact)
}

fn same_decisions_as_minimax_q_at(depth: u32) {
    let mut errors = Vec::new();
    let mut with_captures = 0;
    for (label, board, side, weights) in positions() {
        let mm = minimax_q(&board, side, weights, depth);
        let ab = alpha_beta_q(&board, side, weights, depth);
        with_captures += usize::from(mm.q_nodes > 0);
        errors.extend(compare(&format!("depth {depth}, {label}"), &mm, &ab));
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    assert!(with_captures > 0, "no quiescence search at depth {depth}");
}

#[test]
fn same_decisions_as_minimax_q_at_depth_1() {
    same_decisions_as_minimax_q_at(1);
}

#[test]
fn same_decisions_as_minimax_q_at_depth_2() {
    same_decisions_as_minimax_q_at(2);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn same_decisions_as_minimax_q_at_depth_3() {
    same_decisions_as_minimax_q_at(3);
}

/// Where alpha-beta with quiescence searched no capture past the depth limit, it is alpha-beta:
/// the same decision from the same nodes. Both kinds of position must occur.
fn quiet_positions_are_unchanged_at(depth: u32) {
    let (mut errors, mut quiet, mut loud) = (Vec::new(), 0, 0);
    for (label, board, side, weights) in positions() {
        let q = alpha_beta_q(&board, side, weights, depth);
        if q.q_nodes > 0 {
            loud += 1;
            continue;
        }
        quiet += 1;
        let plain = alpha_beta(&board, side, weights, depth);
        if !same_choice(&q, &plain) || q.nodes != plain.nodes {
            errors.push(format!(
                "depth {depth}, {label}: quiet, but not alpha-beta's decision"
            ));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    assert!(quiet > 0 && loud > 0, "quiet {quiet}, with captures {loud}");
}

#[test]
fn quiet_positions_are_unchanged_at_depth_1() {
    quiet_positions_are_unchanged_at(1);
}

#[test]
fn quiet_positions_are_unchanged_at_depth_2() {
    quiet_positions_are_unchanged_at(2);
}

#[test]
fn quiet_positions_are_unchanged_at_depth_3() {
    quiet_positions_are_unchanged_at(3);
}

#[test]
fn iterative_deepening_with_quiescence_is_alpha_beta_q() {
    let limits = Limits {
        depth: Some(3),
        ..Limits::default()
    };
    let mut errors = Vec::new();
    for (label, board, side, weights) in positions() {
        let id = iterative_deepening(&board, side, weights, limits, true);
        if !same_choice(&id.decision, &alpha_beta_q(&board, side, weights, id.depth)) {
            errors.push(format!("{label}: not the decision of ab{}q", id.depth));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn a_man_left_to_be_taken_at_the_depth_limit() {
    // White to move. d4-d5 lets Black's man on d6 take it (d6xd4); one ply deep, plain alpha-beta
    // scores d4-d5 before the capture, with the men still equal. Quiescence plays the capture:
    // White is a man down. A quiet move such as d4-c4 leaves Black nothing to take, so its value
    // does not change.
    let board: Board = "......../......../...b..../......../...w..../......../......../W......."
        .parse()
        .unwrap();
    let plain = alpha_beta(&board, Color::White, Weights::TUNED, 1);
    let q = alpha_beta_q(&board, Color::White, Weights::TUNED, 1);
    let u = |d: &Decision, mv: &str| {
        d.candidates
            .iter()
            .find(|c| c.mv.to_string() == mv)
            .map(|c| (c.u, c.exact))
            .unwrap()
    };
    assert_eq!(plain.q_nodes, 0);
    assert!(q.q_nodes > 0);
    assert_eq!(u(&q, "d4-c4"), u(&plain, "d4-c4"));
    let (before, after) = (u(&plain, "d4-d5").0, u(&q, "d4-d5").0);
    assert!(
        before - after >= Weights::TUNED.man - 0.5,
        "d4-d5: {before} without quiescence, {after} with"
    );
    let chosen = |d: &Decision| d.candidates[d.top[0]].mv.to_string();
    assert_ne!(chosen(&q), "d4-d5");
}
