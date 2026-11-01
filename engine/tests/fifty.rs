//! The game's other draw rules in the search (the letter `f`): the names, a 50-ply draw and a
//! draw by one piece each below the root, the count that starts from the game's (`Agent::observe`)
//! and is reset by a capture, progress pressure (`:fs=1`), and the referee: a strict table makes
//! the decisions of the same search without one, with the rules on, near the 50-ply limit too.

mod common;

use engine::{
    AgentSpec, Board, Color, DRAW, Decision, DrawRules, Recorder, SearchOptions, Table,
    TableOptions, U_EPSILON, Weights, alpha_beta_with, native_clock,
};

fn board(s: &str) -> Board {
    s.parse().unwrap()
}

fn spec(s: &str) -> AgentSpec {
    s.parse().unwrap()
}

fn options(quiet: u32, progress: bool) -> SearchOptions {
    SearchOptions {
        quiescence: true,
        order: true,
        draws: Some(DrawRules { quiet, progress }),
    }
}

/// White is a king and a man ahead and nothing can capture: a king on a2 and men on a1 and c1
/// against a king on h8 and a man on h7.
const AHEAD: &str = ".......B/.......b/......../......../......../......../W......./w.w.....";

#[test]
fn names_with_f_print_as_they_read() {
    for name in [
        "idqrotf:n20000:w=texel1",
        "idqrotf:n20000:w=texel1:fs=1",
        "ab3qf",
        "ab6otf:fs=1",
        "idf:d3",
        "idqrotf:n20000:w=texel1:fs=1:hash=64",
    ] {
        assert_eq!(spec(name).to_string(), name);
    }
    assert_eq!(spec("idqrotf:n2000:fs=0"), spec("idqrotf:n2000"));
    assert_eq!(
        spec("idqrotf:n2000:hash=64:fs=1").to_string(),
        "idqrotf:n2000:fs=1:hash=64"
    );
    // f comes last among the letters, and fs needs it.
    for bad in [
        "idfq:n2000",
        "abf3",
        "ab3fq",
        "idqrot:n2000:fs=1",
        "tuned:fs=1",
        "idqrotf:n2000:fs=2",
        "idqrotf:n2000:fs=1:fs=1",
    ] {
        assert!(bad.parse::<AgentSpec>().is_err(), "{bad}");
    }
}

#[test]
fn after_49_quiet_plies_every_quiet_move_is_a_draw() {
    let ahead = board(AHEAD);
    let plain = alpha_beta_with(
        &ahead,
        Color::White,
        Weights::TUNED,
        3,
        options(0, false),
        None,
        None,
    );
    assert!(plain.candidates.iter().any(|c| c.u > 0.5), "{plain:?}");
    let last = alpha_beta_with(
        &ahead,
        Color::White,
        Weights::TUNED,
        3,
        options(49, false),
        None,
        None,
    );
    assert!(
        last.candidates.iter().all(|c| c.u == DRAW && c.exact),
        "{last:?}"
    );
    // With 48, Black's reply is the 50th quiet ply.
    let one_before = alpha_beta_with(
        &ahead,
        Color::White,
        Weights::TUNED,
        3,
        options(48, false),
        None,
        None,
    );
    assert!(
        one_before.candidates.iter().all(|c| c.u == DRAW),
        "{one_before:?}"
    );
    // Without the rules the count does not matter.
    let without = SearchOptions {
        draws: None,
        ..options(49, false)
    };
    let fresh = SearchOptions {
        draws: None,
        ..options(0, false)
    };
    let without = alpha_beta_with(&ahead, Color::White, Weights::TUNED, 3, without, None, None);
    let fresh = alpha_beta_with(&ahead, Color::White, Weights::TUNED, 3, fresh, None, None);
    assert_eq!(values(&without), values(&fresh));
}

#[test]
fn a_capture_resets_the_count() {
    // White must take d5 (d4xd6); then two men against one.
    let capture = board("......../.......b/......../...b..../...w..../......../......../w.......");
    let decision = alpha_beta_with(
        &capture,
        Color::White,
        Weights::TUNED,
        3,
        options(49, false),
        None,
        None,
    );
    assert!(decision.candidates[0].mv.is_capture());
    assert!(
        decision.candidates.iter().all(|c| c.u > 0.5),
        "{decision:?}"
    );
}

#[test]
fn one_piece_each_is_a_draw_whatever_the_count() {
    // White's only move takes d5 and leaves a man against h8's king.
    let last = board(".......B/......../......../...b..../...w..../......../......../........");
    let without = SearchOptions {
        draws: None,
        ..options(0, false)
    };
    let plain = alpha_beta_with(&last, Color::White, Weights::TUNED, 1, without, None, None);
    assert_ne!(plain.candidates[0].u, DRAW);
    let ruled = alpha_beta_with(
        &last,
        Color::White,
        Weights::TUNED,
        1,
        options(0, false),
        None,
        None,
    );
    assert_eq!(values(&ruled), vec![DRAW]);
}

#[test]
fn progress_pressure_scales_u_toward_zero() {
    let ahead = board(AHEAD);
    let at = |quiet| {
        alpha_beta_with(
            &ahead,
            Color::White,
            Weights::TUNED,
            1,
            options(quiet, true),
            None,
            None,
        )
    };
    let plain = alpha_beta_with(
        &ahead,
        Color::White,
        Weights::TUNED,
        1,
        options(0, false),
        None,
        None,
    );
    // A leaf one ply below the root has the count plus one.
    for quiet in [0, 20, 40] {
        let scaled = at(quiet);
        for (s, p) in scaled.candidates.iter().zip(&plain.candidates) {
            let expected = p.u * f64::from(100 - quiet - 1) / 100.0;
            assert!(
                (s.u - expected).abs() < 1e-12,
                "{quiet}: {} against {expected}",
                s.u
            );
        }
    }
    // The same for Black in the position turned upside down, colours swapped.
    let flipped = flip(&ahead);
    let black = alpha_beta_with(
        &flipped,
        Color::Black,
        Weights::TUNED,
        1,
        options(30, true),
        None,
        None,
    );
    let mut ours = values(&at(30));
    let mut theirs = values(&black);
    ours.sort_by(f64::total_cmp);
    theirs.sort_by(f64::total_cmp);
    assert_eq!(ours, theirs);
}

#[test]
fn an_agent_with_f_counts_the_quiet_plies_it_observes() {
    // Kings and men that only shuffle: 47 quiet plies, then White to move.
    let mut agent = spec("ab3qf").build();
    agent.new_game(1).unwrap();
    let (mut position, mut side) = (board(AHEAD), Color::White);
    let mut plies = 0;
    agent.observe(&position, side);
    while plies < 47 {
        let legal = position.legal_moves(side);
        let mv = legal.iter().find(|m| !m.is_capture()).unwrap();
        position = position.apply(mv);
        side = side.opponent();
        plies += 1;
        agent.observe(&position, side);
    }
    let legal = position.legal_moves(side);
    let mut recorder = Recorder::new(native_clock, None);
    let thought = agent
        .think(&position, side, &legal, &mut recorder, None)
        .unwrap();
    let opts = SearchOptions {
        order: false,
        ..options(47, false)
    };
    let expected = alpha_beta_with(&position, side, Weights::TUNED, 3, opts, None, None);
    assert_eq!(
        values(thought.decision.as_ref().unwrap()),
        values(&expected)
    );
    let fresh = SearchOptions {
        order: false,
        ..options(0, false)
    };
    let fresh = alpha_beta_with(&position, side, Weights::TUNED, 3, fresh, None, None);
    assert_ne!(values(&expected), values(&fresh), "the count matters here");
}

fn values(decision: &Decision) -> Vec<f64> {
    decision.candidates.iter().map(|c| c.u).collect()
}

/// The board turned upside down (rank r to rank 9 − r) with the colours swapped.
fn flip(board: &Board) -> Board {
    let mut flipped = Board::empty();
    for file in 0..8 {
        for rank in 0..8 {
            let piece = board
                .piece_at(engine::Square::new(file, rank))
                .map(|p| engine::Piece {
                    color: p.color.opponent(),
                    kind: p.kind,
                });
            flipped.set(engine::Square::new(file, 7 - rank), piece);
        }
    }
    flipped
}

/// The differences between a search with a strict table and one without: the same `top`, the
/// same value for every best move and wherever both are exact, a bound only where it rules the
/// move out (as in tests/table.rs).
fn compare(label: &str, without: &Decision, with: &Decision) -> Vec<String> {
    let mut errors = Vec::new();
    if with.top != without.top {
        errors.push(format!(
            "{label}: top {:?}, without {:?}",
            with.top, without.top
        ));
    }
    let best = (without.candidates.iter())
        .map(|c| c.u)
        .fold(f64::NEG_INFINITY, f64::max);
    for (i, (w, p)) in with.candidates.iter().zip(&without.candidates).enumerate() {
        let what = format!("{label}: move {} ({i})", p.mv);
        if without.top.contains(&i) && !(w.exact && w.u == p.u) {
            errors.push(format!("{what}: best move worth {}, without {}", w.u, p.u));
        }
        if w.exact && p.exact && w.u != p.u {
            errors.push(format!("{what}: exact value {}, without {}", w.u, p.u));
        }
        if !w.exact && (best - w.u <= U_EPSILON || (p.exact && w.u < p.u)) {
            errors.push(format!("{what}: bound {}, without {}", w.u, p.u));
        }
    }
    errors
}

/// Strict tables against none, with the rules on, at counts far from and near the limit, with
/// and without progress pressure and repetitions (an empty game); the table is kept from one
/// position to the next and searched twice, so that entries from other counts are found.
fn strict_decisions_at(depth: u32, every: usize) {
    let mut errors = Vec::new();
    let mut tables: Vec<Table> = (0..4)
        .map(|_| {
            Table::new(TableOptions {
                mb: 1,
                strict: true,
            })
        })
        .collect();
    let mut last_weights = None;
    for (label, board, side, weights) in common::positions().into_iter().step_by(every) {
        if last_weights.replace(weights) != Some(weights) {
            tables.iter_mut().for_each(Table::clear);
        }
        for quiet in [0, 44, 46, 48, 49] {
            for (k, table) in tables.iter_mut().enumerate() {
                let opts = options(quiet, k & 1 != 0);
                let history = (k & 2 != 0).then_some(&[][..]);
                let what = format!("depth {depth}, {label}, {opts:?}, {history:?}");
                let without = alpha_beta_with(&board, side, weights, depth, opts, history, None);
                for pass in ["full", "again"] {
                    let with = alpha_beta_with(
                        &board,
                        side,
                        weights,
                        depth,
                        opts,
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
fn strict_tables_make_the_same_decisions_with_f_at_depth_3() {
    strict_decisions_at(3, 5);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn strict_tables_make_the_same_decisions_with_f_at_depth_5() {
    strict_decisions_at(5, 1);
}
