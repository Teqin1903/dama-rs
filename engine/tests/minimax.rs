//! The minimax referee: a position whose only win is three plies deep, and depth 1 against the
//! baseline's decisions in testdata/decisions.json.

mod common;

use engine::{Board, Color, Decision, WIN, Weights, decide, minimax};
use serde::Deserialize;

/// White to move: 20 legal moves, and only f6-a6 wins, at ply 3.
const WIN_IN_THREE: &str =
    "......../......../.....W../b..b..../.w....../..B.w.../.b....../........";

/// The moves of `decision` that minimax values as a win, with their values.
fn wins(decision: &Decision) -> Vec<(String, f64)> {
    decision
        .candidates
        .iter()
        // A win is worth WIN - (ply - 1); nothing else comes close.
        .filter(|c| c.u >= WIN - 100.0)
        .map(|c| (c.mv.to_string(), c.u))
        .collect()
}

#[test]
fn only_f6_a6_wins_and_only_at_depth_three() {
    let board: Board = WIN_IN_THREE.parse().unwrap();
    assert_eq!(board.legal_moves(Color::White).len(), 20);
    for depth in [1, 2] {
        let decision = minimax(&board, Color::White, Weights::TUNED, depth);
        assert_eq!(wins(&decision), [], "depth {depth} should see no win");
    }
    let decision = minimax(&board, Color::White, Weights::TUNED, 3);
    // Our move, theirs, ours, and then Black has no legal move: ply 3, worth WIN - 2.
    assert_eq!(wins(&decision), [("f6-a6".to_string(), WIN - 2.0)]);
    let best = &decision.candidates[decision.top[0]];
    assert_eq!(
        (decision.top.len(), best.mv.to_string()),
        (1, "f6-a6".into())
    );
}

#[test]
fn nodes_of_the_win_in_three() {
    // Counted with the same minimax over reference/js/engine.js: every position `apply` produced,
    // the root not included.
    let board: Board = WIN_IN_THREE.parse().unwrap();
    let nodes: Vec<u64> = (1..=3)
        .map(|depth| minimax(&board, Color::White, Weights::TUNED, depth).nodes)
        .collect();
    assert_eq!(nodes, [20, 108, 990]);
}

#[derive(Deserialize)]
struct Entry {
    name: String,
    board: String,
    side: String,
    weights: JsWeights,
}

#[derive(Deserialize)]
struct JsWeights {
    man: f64,
    king: f64,
    mob: f64,
}

#[test]
fn depth_one_is_the_baseline_on_every_decision() {
    let entries: Vec<Entry> =
        serde_json::from_str(&common::read_testdata("decisions.json")).unwrap();
    assert!(!entries.is_empty(), "decisions.json has no entries");
    for entry in &entries {
        let board: Board = entry.board.parse().unwrap();
        let side = common::side(&entry.side);
        let w = &entry.weights;
        let weights = Weights {
            man: w.man,
            king: w.king,
            mob: w.mob,
        };
        let (one_ply, mm1) = (
            decide(&board, side, weights),
            minimax(&board, side, weights, 1),
        );
        let values = |d: &Decision| d.candidates.iter().map(|c| c.u).collect::<Vec<_>>();
        assert_eq!(values(&mm1), values(&one_ply), "{}", entry.name);
        assert_eq!(mm1.top, one_ply.top, "{}", entry.name);
    }
}
