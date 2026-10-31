//! Monte Carlo tree search (engine/src/mcts.rs): the budget, the seed, the draw rules of its random
//! games, quiescence at its leaves, and the agents' names.

use engine::{
    AgentSpec, Board, Clock, Color, Limits, MctsKind, Mulberry32, Weights, mcts, quiescence_value,
    static_value,
};

const KINDS: [MctsKind; 4] = [
    MctsKind::Flat,
    MctsKind::Uct,
    MctsKind::UctEval,
    MctsKind::UctQuiescence,
];

fn nodes(n: u64) -> Limits {
    Limits {
        nodes: Some(n),
        ..Limits::default()
    }
}

/// Positions of random games from the start, every `every` plies, with their clocks.
fn positions(seed: u32, every: u32, count: usize) -> Vec<(Board, Color, Clock)> {
    let mut rng = Mulberry32::new(seed);
    let mut found = Vec::new();
    while found.len() < count {
        let (mut board, mut side, mut clock) = (Board::start(), Color::White, Clock::default());
        loop {
            let moves = board.legal_moves(side);
            if moves.is_empty() || clock.ply >= 200 {
                break;
            }
            if clock.ply % every == every - 1 && found.len() < count {
                found.push((board, side, clock));
            }
            let mv = &moves[rng.next_index(moves.len())];
            clock = Clock {
                ply: clock.ply + 1,
                quiet: if mv.is_capture() { 0 } else { clock.quiet + 1 },
            };
            board = board.apply(mv);
            side = side.opponent();
        }
    }
    found
}

#[test]
fn the_budget_holds_and_the_visits_add_up() {
    for (board, side, clock) in positions(1, 9, 20) {
        let legal = board.legal_moves(side);
        for kind in KINDS {
            let mut rng = Mulberry32::new(3);
            let r = mcts(
                kind,
                &board,
                side,
                &legal,
                clock,
                nodes(3000),
                Weights::TEXEL1,
                &mut rng,
            );
            assert!(r.nodes <= 3000, "{kind:?}: {} nodes", r.nodes);
            assert!(r.nodes >= 1500, "{kind:?}: only {} nodes", r.nodes);
            assert!(r.best < legal.len());
            assert_eq!(r.visits.len(), legal.len());
            let visits: u64 = r.visits.iter().map(|&v| u64::from(v)).sum();
            assert_eq!(visits, r.iterations, "{kind:?}");
            assert!(r.means.iter().all(|m| (0.0..=1.0).contains(m)));
            let most = *r.visits.iter().max().unwrap();
            if kind != MctsKind::Flat {
                assert_eq!(r.visits[r.best], most, "{kind:?}: the move visited most");
            }
        }
    }
}

#[test]
fn the_same_seed_gives_the_same_search() {
    for (board, side, clock) in positions(2, 13, 6) {
        let legal = board.legal_moves(side);
        for kind in KINDS {
            let run = |seed| {
                let mut rng = Mulberry32::new(seed);
                mcts(
                    kind,
                    &board,
                    side,
                    &legal,
                    clock,
                    nodes(2000),
                    Weights::TEXEL1,
                    &mut rng,
                )
            };
            assert_eq!(run(9), run(9), "{kind:?}");
        }
    }
}

/// At the ply limit every random game is over after the root move, so flat Monte Carlo spends
/// one position per game; 49 quiet plies do the same after a quiet root move.
#[test]
fn random_games_keep_the_draw_rules_of_the_game() {
    for (board, side, clock) in positions(4, 11, 10) {
        let legal = board.legal_moves(side);
        let mut clocks = vec![Clock { ply: 399, quiet: 0 }];
        if !legal[0].is_capture() {
            clocks.push(Clock {
                ply: clock.ply,
                quiet: 49,
            });
        }
        for clock in clocks {
            let mut rng = Mulberry32::new(1);
            let r = mcts(
                MctsKind::Flat,
                &board,
                side,
                &legal,
                clock,
                nodes(500),
                Weights::TUNED,
                &mut rng,
            );
            assert_eq!(r.iterations, r.nodes, "{clock:?}");
            // A draw, unless the move leaves the opponent without a move.
            for (mv, mean) in legal.iter().zip(&r.means) {
                let wins = board.apply(mv).legal_moves(side.opponent()).is_empty();
                assert_eq!(*mean, if wins { 1.0 } else { 0.5 });
            }
        }
    }
}

/// Where a move leaves the opponent without a legal move and another does not, every form gives
/// that move the mean 1 and plays a move with the mean 1. With random games a move that wins
/// later scores 1 as well (flat Monte Carlo and UCT do not prefer the quicker win), and so does
/// one that wins after forced captures with quiescence (sigmoid(K · WIN) is 1 in f64); with U
/// only a move that wins at once scores 1, so `ucte` plays one.
#[test]
fn a_move_that_wins_at_once_is_played() {
    let mut rng = Mulberry32::new(5);
    let mut cases = 0;
    while cases < 8 {
        let (mut board, mut side) = (Board::start(), Color::White);
        for _ in 0..400 {
            let moves = board.legal_moves(side);
            if moves.is_empty() {
                break;
            }
            let wins: Vec<bool> = (moves.iter())
                .map(|mv| board.apply(mv).legal_moves(side.opponent()).is_empty())
                .collect();
            if wins.contains(&true) && wins.contains(&false) {
                cases += 1;
                for kind in KINDS {
                    let mut seed = Mulberry32::new(1);
                    let clock = Clock::default();
                    let r = mcts(
                        kind,
                        &board,
                        side,
                        &moves,
                        clock,
                        nodes(20000),
                        Weights::TEXEL1,
                        &mut seed,
                    );
                    for (i, &w) in wins.iter().enumerate() {
                        assert!(!w || r.means[i] == 1.0, "{kind:?} on {board:?}");
                    }
                    assert_eq!(r.means[r.best], 1.0, "{kind:?} on {board:?}");
                    if kind == MctsKind::UctEval {
                        assert!(wins[r.best], "{kind:?} on {board:?}");
                    }
                }
            }
            board = board.apply(&moves[rng.next_index(moves.len())]);
            side = side.opponent();
        }
    }
}

#[test]
fn one_legal_move_is_played_without_a_search() {
    let (board, side, _) = (positions(6, 1, 400).into_iter())
        .find(|(b, s, _)| b.legal_moves(*s).len() == 1)
        .expect("a position with one legal move");
    let legal = board.legal_moves(side);
    for kind in KINDS {
        let mut agent = AgentSpec::Mcts {
            kind,
            limits: nodes(20000),
            weights: Weights::TUNED,
        }
        .build();
        assert_eq!(agent.choose(&board, side, &legal).unwrap(), &legal[0]);
        assert_eq!(agent.nodes(), 0);
    }
}

/// Quiescence at a quiet position is U; with a capture it searches, within its budget.
#[test]
fn quiescence_at_a_leaf() {
    let mut quiet = 0;
    let mut captures = 0;
    for (board, side, _) in positions(7, 3, 200) {
        let moves = board.legal_moves(side);
        let w = Weights::TEXEL1;
        let (v, n) = quiescence_value(&board, side, side.opponent(), w, 10_000).unwrap();
        if moves[0].is_capture() {
            captures += 1;
            assert!(n >= moves.len() as u64);
            assert_eq!(
                quiescence_value(&board, side, side.opponent(), w, n),
                Some((v, n))
            );
            assert_eq!(
                quiescence_value(&board, side, side.opponent(), w, n - 1),
                None
            );
        } else {
            quiet += 1;
            assert_eq!(n, 0);
            assert_eq!(v, static_value(&board, side, side.opponent(), w));
        }
    }
    assert!(
        quiet > 20 && captures > 20,
        "{quiet} quiet, {captures} with a capture"
    );
}

#[test]
fn names() {
    let round_trip = [
        "mc:n20000",
        "uct:n20000",
        "ucte:n20000:w=texel1",
        "uctq:n20000:w=texel1",
        "uct:t50",
        "uctq:n100:t50:adv=0.1",
    ];
    for name in round_trip {
        assert_eq!(name.parse::<AgentSpec>().unwrap().to_string(), name);
    }
    assert_eq!(
        "ucte:n20000".parse::<AgentSpec>().unwrap(),
        AgentSpec::Mcts {
            kind: MctsKind::UctEval,
            limits: nodes(20000),
            weights: Weights::TUNED,
        }
    );
    assert_eq!("mc:n5".parse::<AgentSpec>().unwrap().weights(), None);
    for bad in [
        "uct",
        "uct:d5",
        "uct:n0",
        "mc:n20000:w=texel1",
        "uctx:n5",
        "mcq:n5",
    ] {
        assert!(bad.parse::<AgentSpec>().is_err(), "{bad}");
    }
}
