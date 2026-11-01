//! The web API (engine/src/api.rs, docs/web-api.md): a game played through a `Session` is the
//! arena's game, tracing never changes a search, `eval` scores a position as the search does,
//! `legal` is our move generator, and a failed call changes nothing.

mod common;

use engine::{
    AgentSpec, Aspiration, Board, Color, Decision, DrawRules, Json, Limits, MatchPlan, Pass,
    Recorder, Schedule, SearchOptions, Session, Table, TableOptions, TraceRequest, Weights,
    alpha_beta_traced, alpha_beta_with, decide, decide_traced, iterative_deepening_traced,
    iterative_deepening_with, minimax, minimax_traced, native_clock, paired_agent_seeds,
    play_match, static_value,
};

fn clock() -> f64 {
    0.0
}

/// A field of an answer, which must not be an error.
fn field<'a>(answer: &'a Json, key: &str) -> &'a Json {
    assert!(answer.get("error").is_none(), "{answer}");
    answer
        .get(key)
        .unwrap_or_else(|| panic!("no {key} in {answer}"))
}

fn text<'a>(answer: &'a Json, key: &str) -> &'a str {
    field(answer, key)
        .as_str()
        .unwrap_or_else(|| panic!("{key} in {answer}"))
}

fn is_error(answer: &Json) -> bool {
    answer.get("error").and_then(Json::as_str).is_some()
}

/// Plays a paired match and replays each game through a session with the arena's seeds and
/// opening, thinking with and without a trace: every move must be the arena's.
fn replay_arena_games(a: &str, b: &str, games: u32) {
    let (sa, sb): (AgentSpec, AgentSpec) = (a.parse().unwrap(), b.parse().unwrap());
    let plan = MatchPlan {
        games,
        seed: 1,
        schedule: Schedule::Paired,
        threads: 2,
        sprt: None,
    };
    let record = play_match(&|| sa.build(), &|| sb.build(), &plan).unwrap();
    for (g, game) in record.games.iter().enumerate() {
        let (seed_a, seed_b) = paired_agent_seeds(1, g as u32);
        let (white, black) = if game.a_white {
            ((a, seed_a), (b, seed_b))
        } else {
            ((b, seed_b), (a, seed_a))
        };
        let moves = &game.record.moves;
        for options in ["{}", r#"{"trace": {"max_nodes": 3000}}"#] {
            let mut session = Session::new(clock);
            field(&session.set_agent("white", white.0, white.1), "ok");
            field(&session.set_agent("black", black.0, black.1), "ok");
            // The random opening plies are the arena's own.
            for mv in &moves[..4] {
                field(&session.play(&mv.to_string()), "board");
            }
            let mut ply = 4;
            let state = loop {
                let state = session.state();
                if text(&state, "status") != "playing" {
                    break state;
                }
                let side = text(&state, "turn").to_string();
                let report = session.think(&side, options);
                let best = text(&report, "best").to_string();
                assert_eq!(
                    best,
                    moves[ply].to_string(),
                    "{a} vs {b}, game {g}, ply {ply}, {options}"
                );
                field(&session.play(&best), "board");
                ply += 1;
            };
            assert_eq!(ply, moves.len(), "{a} vs {b}, game {g}");
            let winner = game.record.winner.map_or("draw", |c| match c {
                Color::White => "white",
                Color::Black => "black",
            });
            assert_eq!(text(&state, "status"), winner, "{a} vs {b}, game {g}");
        }
    }
}

#[test]
fn a_session_plays_the_arenas_games() {
    replay_arena_games("idqrot:n2000:w=texel1", "idqr:n2000", 2);
    replay_arena_games("ab3o", "tuned", 2);
    replay_arena_games("mm2", "idq:n1000", 2);
    replay_arena_games("idqrotf:n2000:w=texel1:fs=1", "ab3qf", 2);
    replay_arena_games("idqrotfp:n2000:w=texel1:fs=1:asp=0.5", "ab3qrp", 2);
}

#[test]
#[ignore = "slow in a debug build; run with --release -- --ignored"]
fn a_session_plays_the_arenas_games_at_the_ladders_budgets() {
    replay_arena_games("idqrot:n20000:w=texel1", "idqr:n20000:w=texel1", 4);
    replay_arena_games("idqr:n20000", "ab5", 4);
    replay_arena_games("idqrotf:n20000:w=texel1", "tuned", 4);
}

/// A position with the side to move, and the positions of the game up to it.
type GamePosition = (Board, Color, Vec<(Board, Color)>);

/// Positions of a few arena games, with the game so far.
fn arena_positions() -> Vec<GamePosition> {
    let spec: AgentSpec = "idqr:n1000:w=texel1".parse().unwrap();
    let plan = MatchPlan {
        games: 4,
        seed: 3,
        schedule: Schedule::Paired,
        threads: 2,
        sprt: None,
    };
    let record = play_match(&|| spec.build(), &|| spec.build(), &plan).unwrap();
    let mut all = Vec::new();
    for game in &record.games {
        let (mut board, mut side) = (Board::start(), Color::White);
        let mut history = vec![(board, side)];
        for mv in &game.record.moves {
            all.push((board, side, history.clone()));
            board = board.apply(mv);
            side = side.opponent();
            history.push((board, side));
        }
    }
    all
}

/// The differences between two decisions of the same search, one of them traced.
fn same(label: &str, plain: &Decision, traced: &Decision) {
    assert_eq!(plain.top, traced.top, "{label}");
    assert_eq!(
        (plain.nodes, plain.q_nodes),
        (traced.nodes, traced.q_nodes),
        "{label}"
    );
    assert_eq!(plain.candidates.len(), traced.candidates.len(), "{label}");
    for (p, t) in plain.candidates.iter().zip(&traced.candidates) {
        assert_eq!(p.mv, t.mv, "{label}");
        assert_eq!(
            (p.u.to_bits(), p.exact, p.index),
            (t.u.to_bits(), t.exact, t.index),
            "{label}"
        );
    }
}

/// What the recorder must say about the decision it watched: the last iteration's root values are
/// the decision's, and its tree holds every node the iteration counted, the root too.
fn recorded(label: &str, decision: &Decision, recorder: &Recorder) {
    let last = recorder.iterations.last().expect("an iteration");
    // Nodes so far: they only grow, and the decision may have counted more in an iteration it
    // did not complete.
    let nodes: Vec<u64> = recorder.iterations.iter().map(|i| i.nodes).collect();
    assert!(nodes.is_sorted(), "{label}: {nodes:?}");
    assert!(last.nodes <= decision.nodes, "{label}");
    assert_eq!(last.root.len(), decision.candidates.len(), "{label}");
    for r in &last.root {
        let c = decision
            .candidates
            .iter()
            .find(|c| c.index == r.index)
            .unwrap();
        assert_eq!(r.value.to_bits(), c.u.to_bits(), "{label}: {}", r.mv);
        assert_eq!(
            r.bound == engine::ValueBound::Exact,
            c.exact,
            "{label}: {}",
            r.mv
        );
        assert_eq!(r.pv[0], c.mv, "{label}");
    }
    let tree = recorder.tree.as_ref().expect("a tree was asked for");
    assert!(!tree.truncated, "{label}");
    // The iteration's own nodes in the try that counted, and its root.
    let before = last
        .attempts
        .last()
        .map_or(nodes.len().checked_sub(2).map_or(0, |i| nodes[i]), |a| {
            a.nodes
        });
    assert_eq!(tree.nodes_total, last.nodes - before + 1, "{label}");
    assert_eq!(tree.nodes.len() as u64, tree.nodes_total, "{label}");
    assert_eq!(tree.nodes[0].sub, tree.nodes_total, "{label}");
    assert_eq!(tree.nodes[0].exit, 2 * tree.nodes_total - 1, "{label}");
    // A move that enters twice under its parent is a scout, then a research.
    for node in &tree.nodes {
        for pair in node.children.windows(2) {
            let (a, b) = (&tree.nodes[pair[0]], &tree.nodes[pair[1]]);
            if b.pass == Pass::Research {
                assert_eq!((a.pass, &a.mv), (Pass::Scout, &b.mv), "{label}");
            }
        }
        let first = node.children.first().map(|&c| tree.nodes[c].pass);
        assert_ne!(
            first,
            Some(Pass::Scout),
            "{label}: the first move has no test"
        );
        assert_ne!(
            first,
            Some(Pass::Research),
            "{label}: the first move has no test"
        );
    }
    let mut root_moves: Vec<usize> = last.root.iter().map(|r| r.index).collect();
    root_moves.sort_unstable();
    root_moves.dedup();
    assert_eq!(
        root_moves.len(),
        last.root.len(),
        "{label}: a root move twice"
    );
    let orders: Vec<u32> = last.root.iter().map(|r| r.order).collect();
    assert!(
        orders.iter().copied().eq(1..=orders.len() as u32),
        "{label}: {orders:?}"
    );
}

/// Every leaf of the tree carries the static value of its position from the root side, bit for
/// bit.
fn leaves_are_static_values(label: &str, board: &Board, side: Color, w: Weights, r: &Recorder) {
    let tree = r.tree.as_ref().unwrap();
    for (id, node) in tree.nodes.iter().enumerate() {
        let Some(value) = node.static_value else {
            continue;
        };
        let mut path = Vec::new();
        let mut at = Some(id);
        while let Some(i) = at {
            path.extend(tree.nodes[i].mv.clone());
            at = tree.nodes[i].parent;
        }
        let mut b = *board;
        for mv in path.iter().rev() {
            b = b.apply(mv);
        }
        let to_move = if node.ply % 2 == 0 {
            side
        } else {
            side.opponent()
        };
        let expected = static_value(&b, to_move, side, w);
        assert_eq!(value.to_bits(), expected.to_bits(), "{label}: node {id}");
    }
}

fn recorder() -> Recorder {
    Recorder::new(
        clock,
        Some(TraceRequest {
            iteration: -1,
            max_nodes: usize::MAX,
        }),
    )
}

#[test]
fn a_traced_search_is_the_untraced_search() {
    let texel1 = Weights::named("texel1").unwrap();
    let small = TableOptions {
        mb: 1,
        strict: false,
    };
    let fixed = common::positions()
        .into_iter()
        .map(|(label, b, s, w)| (label, b, s, w, Vec::new()));
    let games = arena_positions()
        .into_iter()
        .enumerate()
        .step_by(3)
        .map(|(i, (b, s, h))| (format!("game position {i}"), b, s, texel1, h));
    for (k, (label, board, side, weights, history)) in fixed.chain(games).enumerate() {
        if board.legal_moves(side).is_empty() {
            continue;
        }
        let history = Some(history.as_slice());
        let check_leaves = k % 7 == 0;

        let mut r = recorder();
        let t = decide_traced(&board, side, weights, None, &mut r);
        same(&label, &decide(&board, side, weights), &t);
        recorded(&label, &t, &r);

        let mut r = recorder();
        let t = minimax_traced(&board, side, weights, 2, false, None, &mut r);
        same(&label, &minimax(&board, side, weights, 2), &t);
        recorded(&label, &t, &r);
        if check_leaves {
            leaves_are_static_values(&label, &board, side, weights, &r);
        }

        for (quiescence, order, pvs) in [
            (false, false, false),
            (true, true, false),
            (false, false, true),
            (true, true, true),
        ] {
            let options = SearchOptions {
                quiescence,
                order,
                pvs,
                ..SearchOptions::default()
            };
            let mut r = recorder();
            let (mut t1, mut t2) = (Table::new(small), Table::new(small));
            let plain = alpha_beta_with(&board, side, weights, 3, options, history, Some(&mut t1));
            let traced = alpha_beta_traced(
                &board,
                side,
                weights,
                3,
                options,
                history,
                Some(&mut t2),
                None,
                &mut r,
            );
            same(&label, &plain, &traced);
            recorded(&label, &traced, &r);
            if check_leaves {
                leaves_are_static_values(&label, &board, side, weights, &r);
            }
        }

        let limits = Limits {
            nodes: Some(2000),
            ..Limits::default()
        };
        // The draw rules near the 50-ply limit, with and without progress pressure.
        let near = |progress| {
            Some(DrawRules {
                quiet: 45,
                progress,
            })
        };
        // Windows from depth 2 on, so that even a short search has some.
        let window = |half_width| {
            Some(Aspiration {
                half_width,
                from_depth: 2,
            })
        };
        for (w, order, table, draws, pvs, aspiration) in [
            (weights, false, false, None, false, None),
            (texel1, true, true, None, false, None),
            (texel1, true, true, near(false), false, None),
            (texel1, true, true, near(true), false, None),
            (weights, false, false, None, true, None),
            (texel1, true, true, near(true), true, None),
            (weights, false, false, None, false, window(1e-6)),
            (texel1, true, true, None, false, window(0.1)),
            (texel1, true, true, near(true), true, window(1e-6)),
            (texel1, true, true, None, true, window(0.5)),
        ] {
            let options = SearchOptions {
                quiescence: true,
                order,
                draws,
                pvs,
                aspiration,
            };
            let mut tables = (
                table.then(|| Table::new(small)),
                table.then(|| Table::new(small)),
            );
            let plain = iterative_deepening_with(
                &board,
                side,
                w,
                limits,
                options,
                history,
                tables.0.as_mut(),
            );
            let mut r = recorder();
            let traced = iterative_deepening_traced(
                &board,
                side,
                w,
                limits,
                options,
                history,
                tables.1.as_mut(),
                None,
                &mut r,
                native_clock,
            );
            same(&label, &plain.decision, &traced.decision);
            assert_eq!(plain.depth, traced.depth, "{label}");
            assert_eq!(r.iterations.last().unwrap().depth, traced.depth, "{label}");
            recorded(&label, &traced.decision, &r);
            // Progress pressure scales U at the leaves.
            if check_leaves && draws.is_none_or(|rules| !rules.progress) {
                leaves_are_static_values(&label, &board, side, w, &r);
            }
        }
    }
}

/// The move a research replaces its scout in the recorder: the principal variation of the best
/// move ends in the position whose value it has, though the scout before the research found the
/// same value through other, cut-off lines.
#[test]
fn a_principal_variation_after_a_research_leads_to_its_value() {
    for (label, board, side, weights) in common::positions().into_iter().step_by(3) {
        if board.legal_moves(side).is_empty() {
            continue;
        }
        for pvs in [false, true] {
            let options = SearchOptions {
                quiescence: true,
                pvs,
                ..SearchOptions::default()
            };
            let mut r = recorder();
            let d = alpha_beta_traced(&board, side, weights, 3, options, None, None, None, &mut r);
            let top = &d.candidates[d.top[0]];
            let last = r.iterations.last().unwrap();
            let root = last.root.iter().find(|m| m.index == top.index).unwrap();
            assert_eq!(root.value.to_bits(), top.u.to_bits(), "{label}");
            let (mut b, mut to_move) = (board, side);
            for mv in &root.pv {
                b = b.apply(mv);
                to_move = to_move.opponent();
            }
            let moves = b.legal_moves(to_move);
            // A line that ends in a win has no value of U to compare.
            if !moves.is_empty() {
                let leaf = static_value(&b, to_move, side, weights);
                assert_eq!(
                    leaf.to_bits(),
                    top.u.to_bits(),
                    "{label}, p {pvs}: {:?}",
                    root.pv
                );
            }
        }
    }
}

#[test]
fn eval_totals_are_the_searchs_static_values() {
    let mut session = Session::new(clock);
    let name = |c: Color| if c == Color::White { "white" } else { "black" };
    for (label, board, side, _) in common::positions().into_iter().step_by(5) {
        field(&session.reset(&board.to_string(), name(side)), "board");
        for weights in [
            "tuned",
            "texel1",
            "idqrot:n20000:w=texel1",
            "tuned:cen=0.05",
        ] {
            let w = weights.parse::<AgentSpec>().unwrap().weights().unwrap();
            // By default the side that has just moved; either side when asked.
            for (options, view) in [
                ("{}".to_string(), side.opponent()),
                (format!(r#"{{"side": "{}"}}"#, name(side)), side),
                (
                    format!(r#"{{"side": "{}"}}"#, name(side.opponent())),
                    side.opponent(),
                ),
            ] {
                let eval = session.eval(weights, &options);
                assert_eq!(text(&eval, "side"), name(view), "{label}");
                let total = field(&eval, "total").as_f64().unwrap();
                assert_eq!(
                    total.to_bits(),
                    static_value(&board, side, view, w).to_bits(),
                    "{label} {options}"
                );
            }
            // One ply deep, the default view is the mover's, which is how `decide` values a move.
            if board.legal_moves(side).is_empty() {
                continue;
            }
            let decision = decide(&board, side, w);
            for c in &decision.candidates {
                let line = format!(r#"{{"line": ["{}"]}}"#, c.mv);
                let total = field(&session.eval(weights, &line), "total")
                    .as_f64()
                    .unwrap();
                assert_eq!(total.to_bits(), c.u.to_bits(), "{label} {}", c.mv);
            }
        }
    }
}

#[test]
fn eval_features_are_named_and_only_those_with_a_weight() {
    let mut session = Session::new(clock);
    field(&session.play("c3-c4"), "board");
    let eval = session.eval("texel1:mob=0", "{}");
    let features = field(&eval, "features").as_array().unwrap();
    let keys: Vec<&str> = features.iter().map(|f| text(f, "key")).collect();
    let names: Vec<&str> = features.iter().map(|f| text(f, "name")).collect();
    assert_eq!(keys, ["man", "king", "adv", "back", "cen", "edge", "qmob"]);
    assert_eq!(
        names,
        [
            "men",
            "kings",
            "advancement",
            "back row",
            "centre",
            "edge",
            "quiet mobility"
        ]
    );
    let eval = session.eval("tuned", "{}");
    let keys: Vec<&str> = (field(&eval, "features").as_array().unwrap().iter())
        .map(|f| text(f, "key"))
        .collect();
    assert_eq!(keys, ["man", "king", "mob"]);
    assert!(is_error(&session.eval("tuned", r#"{"side": "red"}"#)));
}

#[test]
fn legal_is_our_move_generator() {
    let entries = Json::parse(&common::read_testdata("movegen.json")).unwrap();
    let mut session = Session::new(clock);
    for entry in entries.as_array().unwrap() {
        let (board, side) = (text(entry, "board"), text(entry, "side"));
        field(&session.reset(board, side), "board");
        let legal = session.legal();
        let mut got: Vec<&str> = legal
            .as_array()
            .unwrap()
            .iter()
            .map(|m| text(m, "move"))
            .collect();
        let mut want: Vec<&str> = field(entry, "moves")
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m.as_str().unwrap())
            .collect();
        got.sort_unstable();
        want.sort_unstable();
        assert_eq!(got, want, "{board} {side}");
        for m in legal.as_array().unwrap() {
            let path = field(m, "path").as_array().unwrap();
            assert_eq!(path[0].as_str(), Some(text(m, "from")));
            assert_eq!(path.last().unwrap().as_str(), Some(text(m, "to")));
            let captures = field(m, "captured").as_array().unwrap().len();
            assert_eq!(text(m, "move").contains('x'), captures > 0, "{m}");
        }
        // The answers are JSON that any reader accepts.
        serde_json::from_str::<serde_json::Value>(&legal.to_string()).unwrap();
    }
}

/// A session four plies into a game with an agent on each side.
fn session_in_a_game() -> Session {
    let mut s = Session::new(clock);
    field(&s.set_agent("white", "idqrot:n2000:w=texel1", 5), "ok");
    field(&s.set_agent("black", "ab3o", 6), "ok");
    for mv in ["c3-c4", "f6-f5", "e3-e4", "b6-b5"] {
        field(&s.play(mv), "board");
    }
    s
}

#[test]
fn a_failed_call_is_an_error_and_changes_nothing() {
    let mut s = session_in_a_game();
    let (state, legal) = (s.state().to_string(), s.legal().to_string());
    let start = Board::start().to_string();
    let wrong: Vec<(&str, Json)> = vec![
        ("not a move", s.play("zz")),
        ("Black's move", s.play("f5-f4")),
        ("bad board", s.reset("xyz", "white")),
        ("bad turn", s.reset(&start, "red")),
        ("bad spec", s.set_agent("white", "nonsense", 1)),
        ("not its turn", s.think("black", "{}")),
        ("options not an object", s.think("white", "[]")),
        ("bad options", s.think("white", "{")),
        ("bad trace", s.think("white", r#"{"trace": 5}"#)),
        (
            "trace iteration 0",
            s.think("white", r#"{"trace": {"iteration": 0}}"#),
        ),
        ("back too far", s.analyze("ab3", r#"{"back": 99}"#)),
        ("back negative", s.analyze("ab3", r#"{"back": -1}"#)),
        ("only illegal", s.analyze("ab3", r#"{"only": ["zz"]}"#)),
        ("only empty", s.analyze("ab3", r#"{"only": []}"#)),
        (
            "only for random",
            s.analyze("random", r#"{"only": ["d3-d4"]}"#),
        ),
        ("bad seed", s.analyze("ab3", r#"{"seed": 1.5}"#)),
        ("no weights", s.eval("random", "{}")),
        ("bad line", s.position(r#"{"line": ["zz"]}"#)),
        ("line not a list", s.position(r#"{"line": "d3-d4"}"#)),
        ("unknown call", s.call("nope", &[], "v")),
        ("too many arguments", s.call("state", &[Json::Null], "v")),
        (
            "argument not a string",
            s.call("play", &[Json::Num(1.0)], "v"),
        ),
        (
            "seed not a number",
            s.call(
                "set_agent",
                &[Json::str("white"), Json::str("ab3"), Json::str("x")],
                "v",
            ),
        ),
    ];
    for (what, answer) in wrong {
        assert!(is_error(&answer), "{what}: {answer}");
        assert_eq!(s.state().to_string(), state, "{what}");
        assert_eq!(s.legal().to_string(), legal, "{what}");
    }
    // The agents are untouched too: White thinks as in a session that saw no failed call.
    let mut twin = session_in_a_game();
    assert_eq!(
        s.think("white", "{}").to_string(),
        twin.think("white", "{}").to_string()
    );
    assert!(is_error(&Session::new(clock).undo()));
}

#[test]
fn undo_restores_the_state() {
    let mut s = session_in_a_game();
    let mut states = vec![(s.state().to_string(), s.legal().to_string())];
    for _ in 0..6 {
        let side = text(&s.state(), "turn").to_string();
        let best = text(&s.think(&side, "{}"), "best").to_string();
        field(&s.play(&best), "board");
        states.push((s.state().to_string(), s.legal().to_string()));
    }
    states.pop();
    while let Some((state, legal)) = states.pop() {
        assert_eq!(s.undo().to_string(), state);
        assert_eq!(s.legal().to_string(), legal);
    }
    // After an undo the agents play on.
    let side = text(&s.state(), "turn").to_string();
    assert!(!is_error(&s.think(&side, "{}")));
}

#[test]
fn undo_keeps_the_table_and_gives_back_the_history() {
    // Without a table or heuristics, thinking again after an undo is thinking as before.
    let mut s = Session::new(clock);
    field(&s.set_agent("black", "idqr:n2000", 3), "ok");
    field(&s.play("c3-c4"), "board");
    let before = s.think("black", "{}");
    field(&s.play(text(&before, "best")), "board");
    s.undo();
    let after = s.think("black", "{}");
    for key in ["value", "nodes", "ties", "root", "iterations"] {
        assert_eq!(field(&after, key), field(&before, key), "{key}");
    }

    // The table stays: the same position again is found in it.
    let mut s = session_in_a_game();
    let first = s.think("white", "{}");
    field(&s.play(text(&first, "best")), "board");
    s.undo();
    let again = s.think("white", "{}");
    let hits = |r: &Json| field(field(r, "stats"), "tt_hits").as_f64().unwrap();
    assert!(hits(&again) > hits(&first), "{first}\n{again}");

    // Kings shuffle, so the history holds repetitions: after taking back two plies, the agents
    // know the game as agents that never saw them.
    let board = ".......B/......../......bb/......../......../......../....w.../W.......";
    let shuffle = ["a1-a2", "h8-h7", "a2-a1", "h7-h8", "a1-a2", "h8-h7"];
    let session = |plies: usize| {
        let mut s = Session::new(clock);
        field(&s.reset(board, "white"), "board");
        field(&s.set_agent("white", "idqr:n2000", 1), "ok");
        field(&s.set_agent("black", "idqr:n2000", 2), "ok");
        for mv in &shuffle[..plies] {
            field(&s.play(mv), "board");
        }
        s
    };
    let mut taken_back = session(6);
    taken_back.undo();
    taken_back.undo();
    let mut twin = session(4);
    assert_eq!(taken_back.state().to_string(), twin.state().to_string());
    assert_eq!(field(&twin.state(), "repetition_count").as_f64(), Some(2.0));
    assert_eq!(
        taken_back.think("white", "{}").to_string(),
        twin.think("white", "{}").to_string()
    );
}

#[test]
fn analyze_searches_a_past_position_and_only_the_moves_asked_for() {
    let mut s = session_in_a_game();
    let before = s.state().to_string();
    let report = s.analyze(
        "idqrot:n2000:w=texel1",
        r#"{"back": 2, "only": ["e3-e4", "d3-d4"]}"#,
    );
    let mut board = Board::start();
    for (mv, side) in [("c3-c4", Color::White), ("f6-f5", Color::Black)] {
        let legal = board.legal_moves(side);
        board = board.apply(legal.iter().find(|m| m.to_string() == mv).unwrap());
    }
    assert_eq!(text(&report, "board"), board.to_string());
    assert!(["e3-e4", "d3-d4"].contains(&text(&report, "best")));
    let root = field(&report, "root").as_array().unwrap();
    let searched = root
        .iter()
        .filter(|r| !matches!(r.get("value"), Some(Json::Null)))
        .count();
    assert_eq!(searched, 2, "{report}");
    assert_eq!(s.state().to_string(), before);
}

#[test]
fn reports_and_traces_carry_the_fields_of_the_contract() {
    let mut s = session_in_a_game();
    let report = s.think("white", r#"{"trace": {"iteration": -1, "max_nodes": 500}}"#);
    for key in [
        "spec",
        "side",
        "board",
        "turn",
        "ply",
        "legal",
        "best",
        "value",
        "bound",
        "depth",
        "nodes",
        "qnodes",
        "ms",
        "pv",
        "ties",
        "iterations",
        "root",
        "stats",
        "trace",
    ] {
        field(&report, key);
    }
    let pv = field(&report, "pv").as_array().unwrap();
    assert_eq!(pv[0].as_str(), Some(text(&report, "best")));
    let trace = field(&report, "trace");
    assert_eq!(field(trace, "truncated"), &Json::Bool(true));
    let nodes = field(trace, "nodes").as_array().unwrap();
    assert_eq!(nodes.len(), 500);
    for key in [
        "id", "parent", "move", "ply", "depth", "kind", "alpha", "beta", "value", "bound",
        "static", "children", "legal", "order", "cutoff", "tt", "draw", "sub", "in", "out",
    ] {
        field(&nodes[1], key);
    }
    serde_json::from_str::<serde_json::Value>(&report.to_string()).unwrap();
    let position = s.position(r#"{"back": 1, "line": ["b6-b5"]}"#);
    assert_eq!(text(&position, "turn"), "white");
}

#[test]
fn a_time_budget_thinks_for_about_its_time() {
    let mut s = Session::new(native_clock);
    field(&s.set_agent("white", "idqrot:t100:w=texel1", 1), "ok");
    let report = s.think("white", "{}");
    let ms = field(&report, "ms").as_f64().unwrap();
    assert!(
        (100.0..400.0).contains(&ms),
        "thought {ms:.1} ms for a 100 ms budget"
    );
    // The site's probe for time budgets.
    field(&s.analyze("idqrot:t5:w=texel1", "{}"), "ms");
}
