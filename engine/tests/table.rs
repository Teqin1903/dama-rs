//! The transposition-table referee: in strict mode (values used only from a search of the same
//! draft and, with repetitions, only where they cannot depend on the path) a search with a table
//! makes the decision of the same search without one: on the 541 fixed positions (with their
//! weights and with texel1's), with and without quiescence, repetitions and ordering, with an
//! empty table and with one left full by the search before; after repetitions in the game; and in
//! iterative deepening. A win found through the table keeps its distance. In normal mode a search
//! is repeatable and the table only finds what the search would allow.

mod common;

use engine::{
    Board, Color, Decision, Limits, Move, SearchOptions, Table, TableOptions, U_EPSILON, WIN,
    Weights, alpha_beta_with, iterative_deepening_with,
};
use serde::Deserialize;

/// A position of decisions.json or movegen.json. Only decisions.json has `weights`.
#[derive(Deserialize)]
struct Position {
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

/// Every position of both files, with the side to move and its weights.
fn positions() -> Vec<(usize, Board, Color, Weights)> {
    let mut all = Vec::new();
    for file in ["decisions.json", "movegen.json"] {
        let entries: Vec<Position> = serde_json::from_str(&common::read_testdata(file)).unwrap();
        for p in entries {
            let weights = p.weights.map_or(Weights::TUNED, |w| Weights {
                man: w.man,
                king: w.king,
                mob: w.mob,
                ..Weights::ZERO
            });
            all.push((
                all.len(),
                p.board.parse().unwrap(),
                common::side(&p.side),
                weights,
            ));
        }
    }
    assert_eq!(all.len(), 541);
    all
}

/// A small table (1 MB, 32768 entries), so that entries are also replaced.
fn table(strict: bool) -> Table {
    Table::new(TableOptions { mb: 1, strict })
}

/// The differences between `with` (a search with a strict table) and `without`: the same `top`,
/// the same value for every best move, the same value wherever both are exact, and a bound only
/// where it rules the move out.
fn compare(label: &str, without: &Decision, with: &Decision) -> Vec<String> {
    let mut errors = Vec::new();
    if with.top != without.top {
        errors.push(format!(
            "{label}: top {:?}, without a table {:?}",
            with.top, without.top
        ));
    }
    let best = (without.candidates.iter())
        .map(|c| c.u)
        .fold(f64::NEG_INFINITY, f64::max);
    for (i, (w, p)) in with.candidates.iter().zip(&without.candidates).enumerate() {
        let what = format!("{label}: move {} ({i})", p.mv);
        if w.mv != p.mv {
            errors.push(format!("{what}: another move"));
        }
        if without.top.contains(&i) && !(w.exact && w.u == p.u) {
            errors.push(format!("{what}: best move worth {}, without {}", w.u, p.u));
        }
        if w.exact && p.exact && w.u != p.u {
            errors.push(format!("{what}: exact value {}, without {}", w.u, p.u));
        }
        if !w.exact && (best - w.u <= U_EPSILON || (p.exact && w.u < p.u)) {
            errors.push(format!(
                "{what}: bound {}, without {} (exact: {})",
                w.u, p.u, p.exact
            ));
        }
    }
    errors
}

/// Strict tables against no table at `depth`, for every combination of quiescence, repetitions
/// (an empty game) and ordering: once with an empty table, once with the table of the search
/// before (another position or the same one). A table belongs to one agent: its values depend on
/// the weights and on quiescence, which the key does not see, so each combination has its own.
fn strict_decisions_at(depth: u32) {
    let mut errors = Vec::new();
    let mut tables: Vec<Vec<Table>> = (0..2)
        .map(|_| (0..8).map(|_| table(true)).collect())
        .collect();
    let mut last_weights = None;
    for (i, board, side, weights) in positions() {
        // The positions of decisions.json come with their own weights: another agent, so the
        // tables start empty.
        if last_weights.replace(weights) != Some(weights) {
            tables.iter_mut().flatten().for_each(Table::clear);
        }
        for (weights, tables) in [weights, Weights::TEXEL1].into_iter().zip(&mut tables) {
            for (k, table) in tables.iter_mut().enumerate() {
                let options = SearchOptions {
                    quiescence: k & 1 != 0,
                    order: k & 2 != 0,
                };
                let history = (k & 4 != 0).then_some(&[][..]);
                let what =
                    format!("depth {depth}, position {i}, {weights}, {options:?} {history:?}");
                let without = alpha_beta_with(&board, side, weights, depth, options, history, None);
                // Full from the position before, then full from this one.
                for pass in ["full", "again"] {
                    let with = alpha_beta_with(
                        &board,
                        side,
                        weights,
                        depth,
                        options,
                        history,
                        Some(&mut *table),
                    );
                    errors.extend(compare(&format!("{what}, {pass}"), &without, &with));
                }
            }
        }
    }
    assert!(
        errors.is_empty(),
        "{}",
        errors[..errors.len().min(30)].join("\n")
    );
}

#[test]
fn strict_tables_make_the_same_decisions_at_depth_1() {
    strict_decisions_at(1);
}

#[test]
fn strict_tables_make_the_same_decisions_at_depth_2() {
    strict_decisions_at(2);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn strict_tables_make_the_same_decisions_at_depth_3() {
    strict_decisions_at(3);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn strict_tables_make_the_same_decisions_at_depth_4() {
    strict_decisions_at(4);
}

fn find<'m>(legal: &'m [Move], notation: &str) -> &'m Move {
    legal
        .iter()
        .find(|mv| mv.to_string() == notation)
        .unwrap_or_else(|| panic!("{notation} is not legal here"))
}

/// The positions of tests/repetition.rs (White a man ahead, a man behind) after the kings went
/// a2-b2, h8-g8, b2-a2, g8-h8, and then a king roaming: the game so far, the last position to
/// move in.
fn repetition_games() -> Vec<Vec<(Board, Color)>> {
    let mut games = Vec::new();
    for start in [
        ".......B/.......b/......../......../......../......../W......./w.w.....",
        ".......B/.......b/.......b/......../......../......../W......./w.......",
    ] {
        for moves in [
            &["a2-b2", "h8-g8", "b2-a2", "g8-h8"][..],
            &["a2-b2", "h8-g8", "b2-a2", "g8-h8", "a2-d2", "h8-f8"][..],
        ] {
            let (mut board, mut side): (Board, Color) = (start.parse().unwrap(), Color::White);
            let mut history = vec![(board, side)];
            for notation in moves {
                board = board.apply(find(&board.legal_moves(side), notation));
                side = side.opponent();
                history.push((board, side));
            }
            games.push(history);
        }
    }
    games
}

#[test]
fn strict_tables_make_the_same_decisions_after_repetitions_in_the_game() {
    let mut errors = Vec::new();
    for history in repetition_games() {
        let (board, side) = *history.last().unwrap();
        // One table per agent: with and without quiescence.
        let mut tables = [table(true), table(true)];
        for depth in 1..=6 {
            for (quiescence, table) in [false, true].into_iter().zip(&mut tables) {
                let options = SearchOptions {
                    quiescence,
                    order: true,
                };
                let without = alpha_beta_with(
                    &board,
                    side,
                    Weights::TUNED,
                    depth,
                    options,
                    Some(&history),
                    None,
                );
                let with = alpha_beta_with(
                    &board,
                    side,
                    Weights::TUNED,
                    depth,
                    options,
                    Some(&history),
                    Some(table),
                );
                let what = format!("{board}, depth {depth}, quiescence {quiescence}");
                errors.extend(compare(&what, &without, &with));
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn strict_iterative_deepening_decides_as_without_a_table() {
    let mut errors = Vec::new();
    let mut table = table(true);
    for (i, board, side, weights) in positions().into_iter().step_by(3) {
        for depth in [3, 4] {
            let limits = Limits {
                depth: Some(depth),
                ..Limits::default()
            };
            let options = SearchOptions {
                quiescence: true,
                order: true,
            };
            let history = Some(&[][..]);
            let without =
                iterative_deepening_with(&board, side, weights, limits, options, history, None);
            let with = iterative_deepening_with(
                &board,
                side,
                weights,
                limits,
                options,
                history,
                Some(&mut table),
            );
            let what = format!("position {i}, id:d{depth}");
            errors.extend(compare(&what, &without.decision, &with.decision));
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

/// White to move wins with f6-a6 in three plies (tests/alpha_beta.rs).
const WIN_IN_THREE: &str =
    "......../......../.....W../b..b..../.w....../..B.w.../.b....../........";

#[test]
fn a_win_found_through_the_table_keeps_its_distance() {
    let board: Board = WIN_IN_THREE.parse().unwrap();
    for strict in [true, false] {
        let mut table = table(strict);
        for depth in 3..=7 {
            for order in [false, true] {
                let options = SearchOptions {
                    quiescence: true,
                    order,
                };
                let decision = alpha_beta_with(
                    &board,
                    Color::White,
                    Weights::TUNED,
                    depth,
                    options,
                    None,
                    Some(&mut table),
                );
                let best = &decision.candidates[decision.top[0]];
                assert_eq!(decision.top.len(), 1);
                assert_eq!(
                    (best.mv.to_string(), best.u),
                    ("f6-a6".to_string(), WIN - 2.0),
                    "strict {strict}, depth {depth}"
                );
            }
        }
    }
}

#[test]
fn normal_tables_are_repeatable_and_save_nodes() {
    let (mut with_nodes, mut without_nodes) = (0, 0);
    for (_, board, side, weights) in positions().into_iter().step_by(5) {
        let limits = Limits {
            depth: Some(4),
            ..Limits::default()
        };
        let options = SearchOptions {
            quiescence: true,
            order: true,
        };
        let run = || {
            let mut table = table(false);
            iterative_deepening_with(
                &board,
                side,
                weights,
                limits,
                options,
                Some(&[]),
                Some(&mut table),
            )
        };
        let (a, b) = (run(), run());
        assert_eq!(a.decision.top, b.decision.top);
        assert_eq!((a.depth, a.decision.nodes), (b.depth, b.decision.nodes));
        with_nodes += a.decision.nodes;
        without_nodes +=
            iterative_deepening_with(&board, side, weights, limits, options, Some(&[]), None)
                .decision
                .nodes;
    }
    assert!(
        with_nodes < without_nodes,
        "{with_nodes} nodes with a table, {without_nodes} without"
    );
}
