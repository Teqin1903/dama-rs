//! The referee of principal variation search and aspiration windows: both are pure speed-ups. On
//! the 141 positions of testdata/decisions.json (with their weights) and the 400 of
//! testdata/movegen.json (with the tuned weights), and again with the texel1 weights, alpha-beta
//! with `p` makes the decision of alpha-beta without it: the same `top`, the same values for the
//! best moves, the same value wherever both found it exactly, and a bound only where it is a true
//! bound out of `top`. Iterative deepening with a window makes the decision of alpha-beta at the
//! depth it completed, with a window that nearly always fails and with one that never does.

mod common;

use common::{compare, positions};
use engine::{
    AgentSpec, Aspiration, DrawRules, Limits, MatchPlan, Move, Recorder, Schedule, SearchOptions,
    Table, TableOptions, WIN, Weights, alpha_beta_with, iterative_deepening_traced,
    iterative_deepening_with, native_clock, play_match,
};

const STRICT: TableOptions = TableOptions {
    mb: 1,
    strict: true,
};

/// The draw rules to check: none, a count that matters with progress pressure, and a count near
/// the 50-ply limit with and without it.
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

/// `p` against no `p` at `depth`, with `q`, `r`, `o` and `f` on and off, without a table and with
/// a strict one, on every `step`-th position. A strict table with `p` faces the search with neither.
fn pvs_decides_as_alpha_beta(depth: u32, step: usize) {
    let mut errors = Vec::new();
    for (label, board, side, weights) in positions().into_iter().step_by(step) {
        for weights in [weights, Weights::TEXEL1] {
            for quiescence in [false, true] {
                for history in [None, Some(&[][..])] {
                    for order in [false, true] {
                        for draws in DRAWS {
                            let plain = SearchOptions {
                                quiescence,
                                order,
                                draws,
                                ..SearchOptions::default()
                            };
                            let pvs = SearchOptions { pvs: true, ..plain };
                            let what =
                                format!("depth {depth}, {label}, {weights}, {plain:?} {history:?}");
                            let p =
                                alpha_beta_with(&board, side, weights, depth, plain, history, None);
                            let o =
                                alpha_beta_with(&board, side, weights, depth, pvs, history, None);
                            errors.extend(compare(&what, &p, &o));
                            let mut table = Table::new(STRICT);
                            let t = alpha_beta_with(
                                &board,
                                side,
                                weights,
                                depth,
                                pvs,
                                history,
                                Some(&mut table),
                            );
                            errors.extend(compare(&format!("{what}, strict"), &p, &t));
                        }
                    }
                }
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn pvs_decides_as_alpha_beta_at_depth_1() {
    pvs_decides_as_alpha_beta(1, 1);
}

#[test]
fn pvs_decides_as_alpha_beta_at_depth_2() {
    pvs_decides_as_alpha_beta(2, 1);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn pvs_decides_as_alpha_beta_at_depth_3() {
    pvs_decides_as_alpha_beta(3, 1);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn pvs_decides_as_alpha_beta_at_depth_4() {
    pvs_decides_as_alpha_beta(4, 1);
}

#[test]
fn pvs_with_a_normal_table_keeps_true_values() {
    // A normal table may use a deeper value, so it is not the referee's mode; the search must still
    // complete with a move, a best move with an exact value.
    for (label, board, side, weights) in positions().into_iter().step_by(5) {
        let options = SearchOptions {
            quiescence: true,
            order: true,
            pvs: true,
            ..SearchOptions::default()
        };
        let mut table = Table::new(TableOptions::default());
        let d = alpha_beta_with(
            &board,
            side,
            weights,
            4,
            options,
            Some(&[]),
            Some(&mut table),
        );
        for &t in &d.top {
            assert!(
                d.candidates[t].exact,
                "{label}: a best move without an exact value"
            );
        }
    }
}

/// `half_width`s with a window from depth 2 on: nearly always failing, usual, never failing.
const WIDTHS: [f64; 4] = [1e-6, 0.25, 0.5, 1000.0];

fn windowed(half_width: f64) -> Option<Aspiration> {
    Some(Aspiration {
        half_width,
        from_depth: 2,
    })
}

/// Iterative deepening with a window, and `p` on and off, within `limits`, against alpha-beta at
/// the depth it completed, on every `step`-th position.
fn windows_decide_as_alpha_beta(limits: Limits, step: usize) {
    let mut errors = Vec::new();
    let mut tries = 0;
    for (label, board, side, weights) in positions().into_iter().step_by(step) {
        for weights in [weights, Weights::TEXEL1] {
            for half_width in WIDTHS {
                for (pvs, table) in [(false, false), (true, false), (true, true), (false, true)] {
                    for draws in [
                        None,
                        Some(DrawRules {
                            quiet: 7,
                            progress: true,
                        }),
                    ] {
                        let plain = SearchOptions {
                            quiescence: true,
                            order: true,
                            draws,
                            ..SearchOptions::default()
                        };
                        let options = SearchOptions {
                            pvs,
                            aspiration: windowed(half_width),
                            ..plain
                        };
                        let mut strict = table.then(|| Table::new(STRICT));
                        let mut recorder = Recorder::new(native_clock, None);
                        let id = iterative_deepening_traced(
                            &board,
                            side,
                            weights,
                            limits,
                            options,
                            Some(&[][..]),
                            strict.as_mut(),
                            None,
                            &mut recorder,
                            native_clock,
                        );
                        let ab = alpha_beta_with(
                            &board,
                            side,
                            weights,
                            id.depth,
                            plain,
                            Some(&[][..]),
                            None,
                        );
                        let what = format!(
                            "{limits:?}, {label}, W {half_width}, {options:?}, reached {}",
                            id.depth
                        );
                        errors.extend(compare(&what, &ab, &id.decision));
                        let mut previous = None;
                        for iteration in &recorder.iterations {
                            let failed = iteration.attempts.len();
                            tries += failed;
                            // An open side cannot fail: W, 4W, open on each side.
                            if failed >= 5 {
                                errors.push(format!(
                                    "{what}: depth {} failed {failed} times",
                                    iteration.depth
                                ));
                            }
                            // A win or a loss before it leaves the window open.
                            let win_before = previous.is_some_and(|v: f64| v.abs() > WIN / 2.0);
                            if failed > 0 && (win_before || iteration.depth < 2) {
                                errors.push(format!(
                                    "{what}: depth {} had a window",
                                    iteration.depth
                                ));
                            }
                            previous = iteration.root.iter().map(|m| m.value).reduce(f64::max);
                        }
                    }
                }
            }
        }
    }
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    assert!(tries > 0, "no window ever failed: the test tests nothing");
}

#[test]
fn windows_with_a_depth_limit_decide_as_alpha_beta() {
    let limits = Limits {
        depth: Some(3),
        ..Limits::default()
    };
    windows_decide_as_alpha_beta(limits, 3);
}

#[test]
fn windows_with_a_node_budget_decide_as_alpha_beta_at_their_depth() {
    for budget in [300, 2000] {
        let limits = Limits {
            nodes: Some(budget),
            ..Limits::default()
        };
        windows_decide_as_alpha_beta(limits, 13);
    }
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn windows_decide_as_alpha_beta_on_every_position() {
    for limits in [
        Limits {
            depth: Some(4),
            ..Limits::default()
        },
        Limits {
            nodes: Some(8000),
            ..Limits::default()
        },
    ] {
        windows_decide_as_alpha_beta(limits, 1);
    }
}

#[test]
fn a_budget_that_runs_out_in_a_research_falls_back_on_the_last_iteration() {
    // With every budget in steps, a window that nearly always fails is interrupted in every place:
    // the decision is always that of the last completed iteration, and the budget is kept.
    let (label, board, side, weights) = positions().into_iter().nth(30).unwrap();
    let options = SearchOptions {
        quiescence: true,
        order: true,
        pvs: true,
        aspiration: windowed(1e-6),
        ..SearchOptions::default()
    };
    let plain = SearchOptions {
        pvs: false,
        aspiration: None,
        ..options
    };
    // The first iteration has no budget, and with `p` it may search a move twice.
    let depth_one = alpha_beta_with(&board, side, weights, 1, options, None, None).nodes;
    for budget in (depth_one..3000).step_by(37) {
        let limits = Limits {
            nodes: Some(budget),
            ..Limits::default()
        };
        let id = iterative_deepening_with(&board, side, weights, limits, options, None, None);
        assert!(
            id.decision.nodes <= budget.max(depth_one),
            "{label}, {budget}"
        );
        let ab = alpha_beta_with(&board, side, weights, id.depth, plain, None, None);
        let errors = compare(&format!("{label}, budget {budget}"), &ab, &id.decision);
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }
}

/// The games of `a` against `b`, with their moves: a pair must play the same games.
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
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn pvs_and_windows_at_a_fixed_depth_play_the_games_of_alpha_beta() {
    // Depth 6 reaches the depth a window of `:asp=W` starts from. The strict table's games are those
    // without a table, so p must not change them either.
    let plain = games("ab6qr", "ab3q", 20);
    for a in [
        "ab6qrp",
        "ab6qrtp:tt=strict",
        "idqr:d6:asp=0.5",
        "idqrp:d6:asp=0.5",
        "idqrp:d6:asp=1e-6",
        "idqrtp:d6:tt=strict:asp=0.25",
    ] {
        assert_eq!(games(a, "ab3q", 20), plain, "{a}");
    }
}
