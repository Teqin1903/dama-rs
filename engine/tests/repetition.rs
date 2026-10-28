//! Repetitions in the search (`alpha_beta_rep`, `iterative_deepening_rep` and the `r` agents): a
//! position of the game or of the search path that occurs again is a draw, worth `DRAW`. A side
//! ahead avoids the move that repeats, a side behind takes it, and where nothing can repeat the
//! search is the one without repetitions.

use engine::{
    AgentSpec, Board, Color, DRAW, Decision, Limits, Move, Weights, alpha_beta, alpha_beta_q,
    alpha_beta_rep, iterative_deepening, iterative_deepening_rep,
};

/// White: king a2, men a1 and c1. Black: king h8, man h7. White is a man ahead, and no capture is
/// possible in a few plies.
const AHEAD: &str = ".......B/.......b/......../......../......../......../W......./w.w.....";

/// White: king a2, man a1. Black: king h8, men h7 and h6. White is a man behind.
const BEHIND: &str = ".......B/.......b/.......b/......../......../......../W......./w.......";

/// The kings go a2-b2, h8-g8, b2-a2, g8-h8: the game is back where it started.
const SHUFFLE: [&str; 4] = ["a2-b2", "h8-g8", "b2-a2", "g8-h8"];

fn find<'m>(legal: &'m [Move], notation: &str) -> &'m Move {
    legal
        .iter()
        .find(|mv| mv.to_string() == notation)
        .unwrap_or_else(|| panic!("{notation} is not legal here"))
}

/// The positions of a game from `start` (White to move) through `moves`, the last one included.
fn game(start: &str, moves: &[&str]) -> Vec<(Board, Color)> {
    let (mut board, mut side): (Board, Color) = (start.parse().unwrap(), Color::White);
    let mut positions = vec![(board, side)];
    for notation in moves {
        board = board.apply(find(&board.legal_moves(side), notation));
        side = side.opponent();
        positions.push((board, side));
    }
    positions
}

/// The value of the move written `notation` in `decision`, and whether it is among the best.
fn value_of(decision: &Decision, notation: &str) -> (f64, bool) {
    let i = decision
        .candidates
        .iter()
        .position(|c| c.mv.to_string() == notation)
        .unwrap();
    (decision.candidates[i].u, decision.top.contains(&i))
}

#[test]
fn ahead_the_search_avoids_the_move_that_repeats() {
    // After the shuffle, a2-b2 brings back the position after the first a2-b2.
    let history = game(AHEAD, &SHUFFLE);
    let (board, side) = *history.last().unwrap();
    for depth in 1..=3 {
        let plain = alpha_beta(&board, side, Weights::TUNED, depth);
        // Without repetitions a2-b2 keeps the extra man; whether it is the very best depends on
        // mobility.
        let (u, _) = value_of(&plain, "a2-b2");
        assert!(
            u > 0.25,
            "depth {depth}: a2-b2 keeps the extra man without repetitions"
        );
        let rep = alpha_beta_rep(&board, side, Weights::TUNED, depth, false, &history);
        assert_eq!(value_of(&rep, "a2-b2"), (DRAW, false), "depth {depth}");
        assert!(rep.candidates[rep.top[0]].u > 0.25);
    }
}

#[test]
fn behind_the_search_takes_the_move_that_repeats() {
    let history = game(BEHIND, &SHUFFLE);
    let (board, side) = *history.last().unwrap();
    for depth in 1..=3 {
        let plain = alpha_beta(&board, side, Weights::TUNED, depth);
        assert!(value_of(&plain, "a2-b2").0 < -0.25, "depth {depth}");
        let rep = alpha_beta_rep(&board, side, Weights::TUNED, depth, false, &history);
        let top: Vec<String> = rep
            .top
            .iter()
            .map(|&i| rep.candidates[i].mv.to_string())
            .collect();
        assert_eq!(top, ["a2-b2"], "depth {depth}");
        assert_eq!(value_of(&rep, "a2-b2"), (DRAW, true));
    }
}

#[test]
fn a_repetition_on_the_search_path_counts_too() {
    // With no game behind it, the line a2-b2 h8-g8 b2-a2 g8-h8 comes back to the root at ply 4:
    // the root is on the path, so it is a draw there. Listing the root in the history as well
    // changes nothing.
    let board: Board = AHEAD.parse().unwrap();
    let with_root = [(board, Color::White)];
    for depth in 1..=5 {
        let empty = alpha_beta_rep(&board, Color::White, Weights::TUNED, depth, false, &[]);
        let root = alpha_beta_rep(
            &board,
            Color::White,
            Weights::TUNED,
            depth,
            false,
            &with_root,
        );
        // The root is on the path anyway: listing it in the history changes nothing.
        let values = |d: &Decision| d.candidates.iter().map(|c| c.u).collect::<Vec<_>>();
        assert_eq!(values(&empty), values(&root));
        assert_eq!((empty.top, empty.nodes), (root.top, root.nodes));
    }
}

#[test]
fn where_nothing_can_repeat_the_search_is_the_plain_one() {
    // From the start, three plies cannot bring back a position.
    let board = Board::start();
    let history = [(board, Color::White)];
    let values = |d: &Decision| d.candidates.iter().map(|c| c.u).collect::<Vec<_>>();
    for depth in 1..=3 {
        let plain = alpha_beta(&board, Color::White, Weights::TUNED, depth);
        let rep = alpha_beta_rep(&board, Color::White, Weights::TUNED, depth, false, &history);
        assert_eq!(values(&rep), values(&plain));
        assert_eq!((rep.top, rep.nodes), (plain.top, plain.nodes));
        let plain_q = alpha_beta_q(&board, Color::White, Weights::TUNED, depth);
        let rep_q = alpha_beta_rep(&board, Color::White, Weights::TUNED, depth, true, &history);
        assert_eq!(values(&rep_q), values(&plain_q));
        assert_eq!(
            (rep_q.nodes, rep_q.q_nodes),
            (plain_q.nodes, plain_q.q_nodes)
        );
    }
    let limits = Limits {
        nodes: Some(5000),
        ..Limits::default()
    };
    let plain = iterative_deepening(&board, Color::White, Weights::TUNED, limits, true);
    let rep = iterative_deepening_rep(&board, Color::White, Weights::TUNED, limits, true, &history);
    assert_eq!(rep.depth, plain.depth);
    assert_eq!(values(&rep.decision), values(&plain.decision));
}

#[test]
fn iterative_deepening_with_a_depth_limit_is_alpha_beta_with_repetitions() {
    let values = |d: &Decision| d.candidates.iter().map(|c| c.u).collect::<Vec<_>>();
    for start in [AHEAD, BEHIND] {
        let history = game(start, &SHUFFLE);
        let (board, side) = *history.last().unwrap();
        for depth in 1..=4 {
            let limits = Limits {
                depth: Some(depth),
                ..Limits::default()
            };
            let id = iterative_deepening_rep(&board, side, Weights::TUNED, limits, true, &history);
            let ab = alpha_beta_rep(&board, side, Weights::TUNED, depth, true, &history);
            assert_eq!(id.depth, depth);
            assert_eq!(id.decision.top, ab.top);
            assert_eq!(values(&id.decision), values(&ab));
        }
    }
}

/// The agent's choice at the end of `moves` from `start`, after observing the whole game.
fn agent_choice(name: &str, start: &str, moves: &[&str]) -> String {
    let spec: AgentSpec = name.parse().unwrap();
    let mut agent = spec.build();
    agent.new_game(1).unwrap();
    let history = game(start, moves);
    for (board, side) in &history {
        agent.observe(board, *side);
    }
    let (board, side) = *history.last().unwrap();
    let legal = board.legal_moves(side);
    agent.choose(&board, side, &legal).unwrap().to_string()
}

#[test]
fn the_agents_remember_the_game() {
    for name in ["ab2r", "ab3qr", "idr:d3", "idqr:n2000"] {
        assert_ne!(agent_choice(name, AHEAD, &SHUFFLE), "a2-b2", "{name}");
        assert_eq!(agent_choice(name, BEHIND, &SHUFFLE), "a2-b2", "{name}");
    }
    // A new game forgets the old one: without the shuffle, a2-b2 repeats nothing.
    let spec: AgentSpec = "ab1r".parse().unwrap();
    let mut agent = spec.build();
    for (board, side) in game(BEHIND, &SHUFFLE) {
        agent.observe(&board, side);
    }
    agent.new_game(1).unwrap();
    let board: Board = BEHIND.parse().unwrap();
    agent.observe(&board, Color::White);
    let legal = board.legal_moves(Color::White);
    let plain = alpha_beta(&board, Color::White, Weights::TUNED, 1);
    let chosen = agent.choose(&board, Color::White, &legal).unwrap();
    assert!(plain.top.iter().any(|&i| plain.candidates[i].mv == *chosen));
}

#[test]
fn agents_without_repetitions_ignore_the_game() {
    for name in ["ab2", "ab3q", "idq:d3"] {
        // Their choice after the shuffle is the plain search's choice in that position.
        let history = game(BEHIND, &SHUFFLE);
        let (board, side) = *history.last().unwrap();
        let spec: AgentSpec = name.parse().unwrap();
        let (mut observed, mut fresh) = (spec.build(), spec.build());
        observed.new_game(7).unwrap();
        fresh.new_game(7).unwrap();
        for (b, s) in &history {
            observed.observe(b, *s);
        }
        let legal = board.legal_moves(side);
        assert_eq!(
            observed.choose(&board, side, &legal).unwrap(),
            fresh.choose(&board, side, &legal).unwrap(),
            "{name}"
        );
    }
}
