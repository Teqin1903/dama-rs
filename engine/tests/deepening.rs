//! The iterative-deepening referee: with only a depth limit N it makes alpha-beta's decision at
//! depth N, on the 141 positions of testdata/decisions.json (with their weights) and the 400 of
//! testdata/movegen.json (with the tuned weights), from the nodes of depths 1 to N together; with a
//! node budget it never spends more than the budget once depth 1 is done.

mod common;

use std::time::Duration;

use engine::{Board, Color, Decision, Limits, Weights, alpha_beta, iterative_deepening};
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

fn depth(depth: u32) -> Limits {
    Limits {
        depth: Some(depth),
        ..Limits::default()
    }
}

fn nodes(nodes: u64) -> Limits {
    Limits {
        nodes: Some(nodes),
        ..Limits::default()
    }
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

/// Iterative deepening to `max` against `alpha_beta` at `max` on every position.
fn same_decisions_as_alpha_beta_at(max: u32) {
    let mut errors = Vec::new();
    for (label, board, side, weights) in positions() {
        let id = iterative_deepening(&board, side, weights, depth(max), false);
        let ab = alpha_beta(&board, side, weights, max);
        if !same_choice(&id.decision, &ab) {
            errors.push(format!(
                "depth {max}, {label}: decision differs from ab{max}"
            ));
        }
        // Fewer iterations only when every line ended before the depth limit.
        let sum: u64 = (1..=id.depth)
            .map(|d| alpha_beta(&board, side, weights, d).nodes)
            .sum();
        if id.depth > max || id.decision.nodes != sum {
            errors.push(format!(
                "depth {max}, {label}: reached {} with {} nodes, alpha-beta 1..={} {sum}",
                id.depth, id.decision.nodes, id.depth
            ));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn same_decisions_as_alpha_beta_at_depth_1() {
    same_decisions_as_alpha_beta_at(1);
}

#[test]
fn same_decisions_as_alpha_beta_at_depth_2() {
    same_decisions_as_alpha_beta_at(2);
}

#[test]
fn same_decisions_as_alpha_beta_at_depth_3() {
    same_decisions_as_alpha_beta_at(3);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn same_decisions_as_alpha_beta_at_depth_4() {
    same_decisions_as_alpha_beta_at(4);
}

#[test]
fn a_node_budget_is_kept_and_buys_depth() {
    let mut errors = Vec::new();
    for (label, board, side, weights) in positions().into_iter().step_by(7) {
        let first = alpha_beta(&board, side, weights, 1).nodes;
        let mut deepest = 0;
        for budget in [50, 300, 2000, 8000] {
            let id = iterative_deepening(&board, side, weights, nodes(budget), false);
            let again = iterative_deepening(&board, side, weights, nodes(budget), false);
            let what = format!("{label}, budget {budget}");
            if id.decision.nodes > budget.max(first) {
                errors.push(format!("{what}: spent {} nodes", id.decision.nodes));
            }
            if id.depth < deepest {
                errors.push(format!("{what}: depth {} after {deepest}", id.depth));
            }
            deepest = id.depth;
            if !same_choice(&id.decision, &alpha_beta(&board, side, weights, id.depth)) {
                errors.push(format!("{what}: not the decision of ab{}", id.depth));
            }
            if (id.depth, id.decision.nodes) != (again.depth, again.decision.nodes) {
                errors.push(format!("{what}: two runs differ"));
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn a_time_limit_always_gives_a_decision() {
    let board = Board::start();
    let limits = Limits {
        time: Some(Duration::from_millis(1)),
        ..Limits::default()
    };
    let id = iterative_deepening(&board, Color::White, Weights::TUNED, limits, false);
    assert!(id.depth >= 1);
    assert!(!id.decision.top.is_empty());
    // A time limit that is never reached changes nothing.
    let limits = Limits {
        depth: Some(3),
        time: Some(Duration::from_secs(3600)),
        ..Limits::default()
    };
    let id = iterative_deepening(&board, Color::White, Weights::TUNED, limits, false);
    assert_eq!(id.depth, 3);
    assert!(same_choice(
        &id.decision,
        &alpha_beta(&board, Color::White, Weights::TUNED, 3)
    ));
}

#[test]
fn deepening_stops_when_every_line_has_ended() {
    // White's man on d4 must take Black's last man on d5, and Black has no move left: depth 1
    // reaches no leaf at the depth limit, so no deeper iteration is run.
    let board: Board = "......../......../......../...b..../...w..../......../......../W......."
        .parse()
        .unwrap();
    let id = iterative_deepening(&board, Color::White, Weights::TUNED, depth(10), false);
    assert_eq!((id.depth, id.decision.nodes), (1, 1));
    let id = iterative_deepening(
        &board,
        Color::White,
        Weights::TUNED,
        nodes(1_000_000),
        false,
    );
    assert_eq!((id.depth, id.decision.nodes), (1, 1));
}
