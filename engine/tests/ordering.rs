//! The move-ordering referee: ordering is a pure speed-up. On the 141 positions of
//! testdata/decisions.json (with their weights) and the 400 of testdata/movegen.json (with the
//! tuned weights), and again with the texel1 weights, ordered alpha-beta makes the decision of
//! alpha-beta in legal order, with quiescence and repetitions too: the same `top`, the same values
//! for the best moves, the same value wherever both found it exactly, and a bound only where it
//! is a true bound out of `top`. Ordered iterative deepening makes the decision of alpha-beta at
//! the depth it completed.

mod common;

use engine::{
    Board, Color, Decision, Limits, Move, SearchOptions, U_EPSILON, Weights, alpha_beta_with,
    iterative_deepening_with,
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

/// The differences between `ordered` and `plain`, the decision of alpha-beta in legal order.
fn compare(label: &str, plain: &Decision, ordered: &Decision) -> Vec<String> {
    let mut errors = Vec::new();
    if ordered.top != plain.top {
        errors.push(format!(
            "{label}: top {:?}, in legal order {:?}",
            ordered.top, plain.top
        ));
    }
    if ordered.candidates.len() != plain.candidates.len() {
        errors.push(format!("{label}: number of candidates differs"));
        return errors;
    }
    let best = plain
        .candidates
        .iter()
        .map(|c| c.u)
        .fold(f64::NEG_INFINITY, f64::max);
    for (i, (o, p)) in ordered.candidates.iter().zip(&plain.candidates).enumerate() {
        let what = format!("{label}: move {} ({i})", p.mv);
        if o.mv != p.mv || o.features != p.features {
            errors.push(format!("{what}: move or features differ"));
        }
        if plain.top.contains(&i) && !o.exact {
            errors.push(format!("{what}: a best move without an exact value"));
        }
        if o.exact && p.exact && o.u != p.u {
            errors.push(format!(
                "{what}: exact value {}, in legal order {}",
                o.u, p.u
            ));
        }
        // An exact value in legal order is the minimax value: the bound must be above it, and
        // far enough below the best to rule the move out.
        if !o.exact && (best - o.u <= U_EPSILON || (p.exact && o.u < p.u)) {
            errors.push(format!(
                "{what}: bound {}, in legal order {} (exact: {}), best {best}",
                o.u, p.u, p.exact
            ));
        }
    }
    errors
}

/// Ordered against legal-order alpha-beta at `depth`, with and without quiescence and
/// repetitions (an empty game: only the search path repeats), with the positions' weights and
/// with texel1's.
fn same_decisions_at(depth: u32) {
    let mut errors = Vec::new();
    for (label, board, side, weights) in positions() {
        for weights in [weights, Weights::TEXEL1] {
            for (quiescence, history) in [
                (false, None),
                (true, None),
                (false, Some(&[][..])),
                (true, Some(&[][..])),
            ] {
                let plain = SearchOptions {
                    quiescence,
                    order: false,
                };
                let ordered = SearchOptions {
                    order: true,
                    ..plain
                };
                let what = format!("depth {depth}, {label}, {weights}, {plain:?} {history:?}");
                let p = alpha_beta_with(&board, side, weights, depth, plain, history, None);
                let o = alpha_beta_with(&board, side, weights, depth, ordered, history, None);
                errors.extend(compare(&what, &p, &o));
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn same_decisions_at_depth_1() {
    same_decisions_at(1);
}

#[test]
fn same_decisions_at_depth_2() {
    same_decisions_at(2);
}

#[test]
fn same_decisions_at_depth_3() {
    same_decisions_at(3);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn same_decisions_at_depth_4() {
    same_decisions_at(4);
}

fn find<'m>(legal: &'m [Move], notation: &str) -> &'m Move {
    legal
        .iter()
        .find(|mv| mv.to_string() == notation)
        .unwrap_or_else(|| panic!("{notation} is not legal here"))
}

#[test]
fn same_decisions_after_a_repetition_in_the_game() {
    // The positions of tests/repetition.rs: White a man ahead and a man behind, after the kings
    // went a2-b2, h8-g8, b2-a2, g8-h8.
    for start in [
        ".......B/.......b/......../......../......../......../W......./w.w.....",
        ".......B/.......b/.......b/......../......../......../W......./w.......",
    ] {
        let (mut board, mut side): (Board, Color) = (start.parse().unwrap(), Color::White);
        let mut history = vec![(board, side)];
        for notation in ["a2-b2", "h8-g8", "b2-a2", "g8-h8"] {
            board = board.apply(find(&board.legal_moves(side), notation));
            side = side.opponent();
            history.push((board, side));
        }
        for depth in 1..=5 {
            for quiescence in [false, true] {
                let plain = SearchOptions {
                    quiescence,
                    order: false,
                };
                let ordered = SearchOptions {
                    order: true,
                    ..plain
                };
                let p = alpha_beta_with(
                    &board,
                    side,
                    Weights::TUNED,
                    depth,
                    plain,
                    Some(&history),
                    None,
                );
                let o = alpha_beta_with(
                    &board,
                    side,
                    Weights::TUNED,
                    depth,
                    ordered,
                    Some(&history),
                    None,
                );
                let errors = compare(&format!("{start}, depth {depth}"), &p, &o);
                assert!(errors.is_empty(), "{}", errors.join("\n"));
            }
        }
    }
}

/// Ordered iterative deepening with `limits` makes the decision of alpha-beta in legal order at
/// the depth it completed (at most the depth limit; fewer only if every line ended first).
fn deepening_decides_as_alpha_beta(limits: Limits, quiescence: bool, step: usize) {
    let mut errors = Vec::new();
    for (label, board, side, weights) in positions().into_iter().step_by(step) {
        let ordered = SearchOptions {
            quiescence,
            order: true,
        };
        let plain = SearchOptions {
            order: false,
            ..ordered
        };
        let id =
            iterative_deepening_with(&board, side, weights, limits, ordered, Some(&[][..]), None);
        let ab = alpha_beta_with(&board, side, weights, id.depth, plain, Some(&[][..]), None);
        let what = format!("{limits:?}, {label}, reached {}", id.depth);
        errors.extend(compare(&what, &ab, &id.decision));
        if let Some(max) = limits.nodes {
            let first = alpha_beta_with(&board, side, weights, 1, plain, None, None).nodes;
            if id.decision.nodes > max.max(first) {
                errors.push(format!("{what}: spent {} nodes", id.decision.nodes));
            }
        }
        if let Some(max) = limits.depth {
            let at_max = alpha_beta_with(&board, side, weights, max, plain, Some(&[][..]), None);
            errors.extend(compare(&format!("{what}, ab{max}"), &at_max, &id.decision));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn deepening_with_a_depth_limit_decides_as_alpha_beta() {
    let depth = |d| Limits {
        depth: Some(d),
        ..Limits::default()
    };
    deepening_decides_as_alpha_beta(depth(3), false, 1);
    deepening_decides_as_alpha_beta(depth(3), true, 1);
}

#[test]
fn deepening_with_a_node_budget_decides_as_alpha_beta_at_its_depth() {
    for budget in [300, 2000, 8000] {
        let limits = Limits {
            nodes: Some(budget),
            ..Limits::default()
        };
        deepening_decides_as_alpha_beta(limits, true, 7);
    }
}

#[test]
fn ordering_needs_fewer_nodes_on_the_whole() {
    // Not a rule for every position, but over all of them at depth 3 ordering must pay off.
    let (mut plain_nodes, mut ordered_nodes) = (0, 0);
    for (_, board, side, weights) in positions() {
        let plain = SearchOptions::default();
        let ordered = SearchOptions {
            order: true,
            ..plain
        };
        plain_nodes += alpha_beta_with(&board, side, weights, 3, plain, None, None).nodes;
        ordered_nodes += alpha_beta_with(&board, side, weights, 3, ordered, None, None).nodes;
    }
    assert!(
        ordered_nodes < plain_nodes,
        "{ordered_nodes} nodes ordered, {plain_nodes} in legal order"
    );
}
