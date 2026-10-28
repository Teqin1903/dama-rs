//! The alpha-beta referee: on the 141 positions of testdata/decisions.json (with their weights) and
//! the 400 of testdata/movegen.json (with the tuned weights), alpha-beta must make minimax's
//! decision at the same depth, from at most as many nodes.

mod common;

use engine::{Board, Color, Decision, U_EPSILON, WIN, Weights, alpha_beta, minimax};
use serde::Deserialize;

/// White to move: only f6-a6 wins, at ply 3 (the same position as in tests/minimax.rs).
const WIN_IN_THREE: &str =
    "......../......../.....W../b..b..../.w....../..B.w.../.b....../........";

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
        assert!(!entries.is_empty(), "{file} has no positions");
        for (i, p) in entries.into_iter().enumerate() {
            let weights = p.weights.map_or(Weights::TUNED, |w| Weights {
                man: w.man,
                king: w.king,
                mob: w.mob,
                ..Weights::ZERO
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

/// Checks alpha-beta against minimax on one position and returns the messages of what differs.
fn compare(label: &str, mm: &Decision, ab: &Decision) -> Vec<String> {
    let mut errors = Vec::new();
    if ab.top != mm.top {
        errors.push(format!("{label}: top {:?}, minimax {:?}", ab.top, mm.top));
    }
    if ab.nodes > mm.nodes {
        errors.push(format!("{label}: {} nodes, minimax {}", ab.nodes, mm.nodes));
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
        // A bound must be a true upper bound, and far enough below the best to rule the move out.
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

/// Runs `compare` on every position at `depth` and reports all differences together.
fn same_decisions_at(depth: u32) {
    let mut errors = Vec::new();
    for (label, board, side, weights) in positions() {
        let mm = minimax(&board, side, weights, depth);
        let ab = alpha_beta(&board, side, weights, depth);
        errors.extend(compare(&format!("depth {depth}, {label}"), &mm, &ab));
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn same_decisions_as_minimax_at_depth_1() {
    same_decisions_at(1);
}

#[test]
fn same_decisions_as_minimax_at_depth_2() {
    same_decisions_at(2);
}

#[test]
fn same_decisions_as_minimax_at_depth_3() {
    same_decisions_at(3);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn same_decisions_as_minimax_at_depth_4() {
    same_decisions_at(4);
}

#[test]
fn only_f6_a6_wins_at_depth_three() {
    let board: Board = WIN_IN_THREE.parse().unwrap();
    let decision = alpha_beta(&board, Color::White, Weights::TUNED, 3);
    let best = &decision.candidates[decision.top[0]];
    assert_eq!(decision.top.len(), 1);
    assert_eq!(
        (best.mv.to_string(), best.u),
        ("f6-a6".to_string(), WIN - 2.0)
    );
    assert!(decision.nodes <= 990, "minimax needs 990 nodes here");
}
