//! The Elo ladder (engine/src/ladder.rs): its pairings, the rows of its games files, and the
//! thinking per game that the rows come from.

use engine::{
    ABLATION, ANCHOR, AgentSpec, Color, EndReason, GameRow, LADDER, MID, MatchPlan, Role, SEED,
    Schedule, SideStats, Thinking, plan, play_match, rate, rows_of,
};

/// A pairing as `(a, b, roles, games)` with the ids of its rungs.
fn named(p: &engine::Pairing) -> (&'static str, &'static str, Vec<Role>, u32) {
    (LADDER[p.a].id, LADDER[p.b].id, p.roles.clone(), p.games)
}

#[test]
fn the_ladder_plays_25_pairings_and_6800_games() {
    use Role::*;
    let want = [
        ("V1", "V0", vec![Previous], 400),
        ("V2", "V1", vec![Previous, Anchor], 400),
        ("V3", "V2", vec![Previous], 400),
        ("V4", "V3", vec![Previous], 400),
        ("V6", "V4", vec![Previous], 400),
        ("V8", "V6", vec![Previous, Mid], 400),
        ("V9", "V8", vec![Previous], 400),
        ("V10", "V9", vec![Previous], 400),
        ("V8b", "V8", vec![Side], 400),
        ("V3", "V1", vec![Anchor, TwoBack], 200),
        ("V4", "V1", vec![Anchor], 200),
        ("V6", "V1", vec![Anchor], 200),
        ("V8", "V1", vec![Anchor], 200),
        ("V8b", "V1", vec![Anchor], 200),
        ("V9", "V1", vec![Anchor], 200),
        ("V10", "V1", vec![Anchor], 200),
        ("V1", "V1", vec![SelfPlay], 200),
        ("V2", "V0", vec![TwoBack], 200),
        ("V4", "V2", vec![TwoBack], 200),
        ("V6", "V3", vec![TwoBack], 200),
        ("V8", "V4", vec![TwoBack], 200),
        ("V9", "V6", vec![TwoBack, Mid], 200),
        ("V10", "V8", vec![TwoBack], 200),
        ("V8b", "V6", vec![Mid], 200),
        ("V10", "V6", vec![Mid], 200),
    ];
    let pairings = plan(LADDER, ANCHOR, MID);
    let got: Vec<_> = pairings.iter().map(named).collect();
    let want: Vec<_> = want.into_iter().collect();
    assert_eq!(got, want);
    assert_eq!(pairings.iter().map(|p| p.games).sum::<u32>(), 6800);
    // Every spec is an agent `dama arena` knows.
    for rung in LADDER {
        assert!(rung.spec.parse::<AgentSpec>().is_ok(), "{}", rung.spec);
    }
}

fn row(game: u32, winner: Option<Color>, reason: EndReason) -> GameRow {
    let stats = |x: f64| SideStats {
        moves: 40 + u64::from(game),
        nodes_per_move: 19_876.5 + x,
        depth_per_move: 5.125,
        ms_per_move: 31.25 + x,
    };
    let (white, black) = if game.is_multiple_of(2) {
        ("a1", "b1")
    } else {
        ("b1", "a1")
    };
    GameRow {
        game,
        pair: game / 2,
        opening_seed: 4_000_000_000 + game,
        white: white.to_string(),
        black: black.to_string(),
        winner,
        plies: 90 + game,
        reason,
        white_stats: stats(0.0),
        black_stats: stats(1.0),
        version: "v10-transposition-table-2-gabc".to_string(),
    }
}

#[test]
fn a_game_row_reads_back_as_written() {
    for (k, reason) in EndReason::ALL.into_iter().enumerate() {
        let winner = [Some(Color::White), Some(Color::Black), None][k % 3];
        let original = row(k as u32, winner, reason);
        let text = original.to_string();
        assert_eq!(text.parse::<GameRow>(), Ok(original), "{text}");
    }
    assert!("1,2,3".parse::<GameRow>().is_err());
}

#[test]
fn rows_of_a_match_carry_its_thinking_game_by_game() {
    let (a, b) = ("ab3", "tuned");
    let (sa, sb) = (
        a.parse::<AgentSpec>().unwrap(),
        b.parse::<AgentSpec>().unwrap(),
    );
    let plan = MatchPlan {
        games: 6,
        seed: SEED,
        schedule: Schedule::Paired,
        threads: 3,
        sprt: None,
    };
    let record = play_match(&|| sa.build(), &|| sb.build(), &plan).unwrap();
    assert_eq!(record.per_game.len(), 6);
    let mut sums = [Thinking::default(); 2];
    for [ta, tb] in &record.per_game {
        sums[0] += *ta;
        sums[1] += *tb;
        // ab3 searches 3 plies for every move, the baseline agent one.
        assert_eq!(ta.depths, 3 * ta.moves);
        assert_eq!(tb.depths, tb.moves);
    }
    assert_eq!(
        (sums[0].moves, sums[0].nodes, sums[0].depths),
        (record.a.moves, record.a.nodes, record.a.depths)
    );
    assert_eq!(
        (sums[1].moves, sums[1].nodes, sums[1].depths),
        (record.b.moves, record.b.nodes, record.b.depths)
    );
    let rows = rows_of(&record, a, b, SEED, "test");
    for (g, (row, game)) in rows.iter().zip(&record.games).enumerate() {
        assert_eq!(row.game as usize, g);
        assert_eq!(row.white, if g.is_multiple_of(2) { a } else { b });
        assert_eq!(row.a_score(), game.a_score());
        assert_eq!(row.plies as usize, game.record.plies());
        assert_eq!(row.reason, game.record.reason);
        // Both games of a pair start from the same opening.
        assert_eq!(row.opening_seed, rows[g / 2 * 2].opening_seed);
        assert_eq!(
            row.white_stats.moves + row.black_stats.moves + 4,
            u64::from(row.plies)
        );
    }
}

#[test]
fn rate_counts_from_the_newer_rungs_view() {
    let pairings = plan(LADDER, ANCHOR, MID);
    // Only V2–V1 is played: V2 wins both games of pair 0, draws both of pair 1, loses one of pair 2.
    let k = pairings
        .iter()
        .position(|p| (LADDER[p.a].id, LADDER[p.b].id) == ("V2", "V1"))
        .unwrap();
    let w = Some(Color::White);
    let b = Some(Color::Black);
    let games = vec![
        row(0, w, EndReason::NoMove),
        row(1, b, EndReason::NoMove),
        row(2, None, EndReason::Repetition),
        row(3, None, EndReason::QuietPlies),
        row(4, b, EndReason::NoMove),
        row(5, None, EndReason::Cap),
    ];
    let mut rows = vec![Vec::new(); pairings.len()];
    rows[k] = games;
    let zero = LADDER.iter().position(|r| r.id == ANCHOR).unwrap();
    let report = rate(LADDER, &pairings, &rows, zero, 100, 1);
    assert_eq!(report.pairings.len(), 1);
    let p = &report.pairings[0];
    assert_eq!((p.wins, p.draws, p.losses), (2, 3, 1));
    assert_eq!(p.score, 3.5 / 6.0);
    assert_eq!(p.draws_by, [1, 1, 0, 1]);
    assert_eq!(p.no_move, 3);
    let v2 = report.versions.iter().find(|v| v.rung.id == "V2").unwrap();
    assert!((v2.elo.unwrap() - p.elo_diff.unwrap()).abs() < 1e-9);
    assert_eq!(report.versions[zero].elo, Some(0.0));
    assert_eq!(report.versions[0].elo, None);
    let json = report.json(
        &engine::Meta {
            commit: "x".to_string(),
            date: "d".to_string(),
            threads: 6,
            wall_s: None,
        },
        &engine::LADDER_SET,
    );
    assert!(!json.contains("\"removed\""));
    assert!(
        json.contains("\"roles\": [\"previous\", \"anchor\"]"),
        "{json}"
    );
    assert!(json.contains("\"wall_s\": null"));
}

#[test]
fn the_ablation_plays_v10_against_five_parts_off_and_the_baseline() {
    let set = engine::ABLATION_SET;
    let pairings = set.plan();
    let got: Vec<_> = pairings
        .iter()
        .map(|p| (ABLATION[p.a].id, ABLATION[p.b].id, p.games, p.reused))
        .collect();
    let want = vec![
        ("V10", "V10-q", 400, false),
        ("V10", "V10-r", 400, false),
        ("V10", "V10-o", 400, false),
        ("V10", "V9", 400, true),
        ("V10", "V10-w", 400, false),
        ("V10", "V0", 400, false),
    ];
    assert_eq!(got, want);
    let played: u32 = pairings.iter().filter(|p| !p.reused).map(|p| p.games).sum();
    assert_eq!(played, 2000);
    // The reused pairing is the ladder's file, with the ladder's agents.
    let v9 = &pairings[3];
    assert!(
        plan(LADDER, ANCHOR, MID)
            .iter()
            .any(|p| p.name(LADDER) == v9.name(ABLATION)
                && LADDER[p.a].spec == ABLATION[v9.a].spec
                && LADDER[p.b].spec == ABLATION[v9.b].spec)
    );
    // V10 is the ladder's V10, V9 the ladder's V9, V0 the ladder's V0.
    for id in ["V10", "V9", "V0"] {
        let spec = |rungs: &[engine::Rung]| rungs.iter().find(|r| r.id == id).unwrap().spec;
        assert_eq!(spec(ABLATION), spec(LADDER), "{id}");
    }
    for rung in ABLATION {
        assert!(rung.spec.parse::<AgentSpec>().is_ok(), "{}", rung.spec);
        assert_eq!(rung.removed.is_none(), rung.id == "V10", "{}", rung.id);
    }
    assert_eq!(set.zero_index(), 0);
}

#[test]
fn in_the_ablation_each_rating_is_its_own_pairings_elo() {
    let set = engine::ABLATION_SET;
    let pairings = set.plan();
    let (w, b) = (Some(Color::White), Some(Color::Black));
    // Pairing k: V10 wins k + 1 of 8 pairs (both games), the rest drawn.
    let rows: Vec<Vec<GameRow>> = (0..pairings.len())
        .map(|k| {
            (0..16)
                .map(|g| {
                    let winner = if g / 2 <= k as u32 {
                        if g % 2 == 0 { w } else { b }
                    } else {
                        None
                    };
                    let reason = if winner.is_some() {
                        EndReason::NoMove
                    } else {
                        EndReason::Repetition
                    };
                    row(g, winner, reason)
                })
                .collect()
        })
        .collect();
    let report = rate(ABLATION, &pairings, &rows, set.zero_index(), 50, 1);
    for p in &report.pairings {
        let lacking = &report.versions[p.pairing.b];
        assert!(
            (lacking.elo.unwrap() + p.elo_diff.unwrap()).abs() < 1e-6,
            "{}: {:?} against {:?}",
            lacking.rung.id,
            lacking.elo,
            p.elo_diff
        );
    }
    let json = report.json(
        &engine::Meta {
            commit: "x".to_string(),
            date: "d".to_string(),
            threads: 6,
            wall_s: None,
        },
        &set,
    );
    assert!(json.contains("\"removed\": \"quiescence\""), "{json}");
    assert!(json.contains("\"reused\": true"));
    assert!(json.contains("\"mid_opponent\": null"));
    assert!(json.contains("\"roles\": [\"ablation\"]"));
}
