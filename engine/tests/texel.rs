//! Texel tuning (engine/src/texel.rs): the features of a position give `evaluate` as a dot product,
//! the samples of a game are its quiet positions with the right label, and the fit finds known
//! weights and K again from labels made with them.

// Only `read_testdata` is used here.
#[allow(dead_code)]
mod common;

use engine::{
    Board, Color, EndReason, Features, GameRecord, Sample, Weights, evaluate, features_of, fit_k,
    mse, samples_of, to_array, tune, value,
};

/// The 541 fixed positions of decisions.json and movegen.json in which both sides have a legal
/// move.
fn positions() -> Vec<Board> {
    let mut all = Vec::new();
    for file in ["decisions.json", "movegen.json"] {
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(&common::read_testdata(file)).unwrap();
        for entry in entries {
            let board: Board = entry["board"].as_str().unwrap().parse().unwrap();
            if [Color::White, Color::Black]
                .iter()
                .all(|&side| !board.legal_moves(side).is_empty())
            {
                all.push(board);
            }
        }
    }
    assert!(all.len() > 500, "{}", all.len());
    all
}

/// Weights with every feature on, all on the 0.0001 grid of `tune`.
const SOME: Weights = Weights {
    man: 1.0,
    king: 2.2,
    mob: -0.03,
    adv: 0.05,
    back: 0.1,
    cen: 0.02,
    edge: -0.04,
    qmob: 0.01,
};

#[test]
fn the_value_is_evaluate_from_whites_view() {
    for board in positions() {
        let features = features_of(&board);
        for w in [Weights::TUNED, SOME] {
            let expected = evaluate(&board, Color::White, &Features::of(&board, Color::White), w);
            let got = value(&features, &to_array(&w));
            assert!(
                (got - expected).abs() < 1e-12,
                "{board:?}: {got} vs {expected}"
            );
        }
    }
}

/// A game of the first legal move every ply from the start, at most `plies` long, that Black won.
fn first_move_game(plies: usize) -> GameRecord {
    let (mut board, mut side) = (Board::start(), Color::White);
    let mut moves = Vec::new();
    for _ in 0..plies {
        let Some(mv) = board.legal_moves(side).first().cloned() else {
            break;
        };
        board = board.apply(&mv);
        side = side.opponent();
        moves.push(mv);
    }
    GameRecord {
        winner: Some(Color::Black),
        reason: EndReason::Cap,
        moves,
    }
}

#[test]
fn the_samples_of_a_game_are_its_quiet_positions() {
    let game = first_move_game(60);
    // The positions the samples must come from, found by replaying the game here.
    let (mut board, mut side) = (Board::start(), Color::White);
    let (mut quiet, mut captures) = (Vec::new(), 0);
    for ply in 0..=game.moves.len() {
        let legal = board.legal_moves(side);
        if legal.iter().any(|mv| mv.is_capture()) {
            captures += 1;
        } else if ply >= 8 && !legal.is_empty() {
            quiet.push(features_of(&board));
        }
        if ply < game.moves.len() {
            board = board.apply(&game.moves[ply]);
            side = side.opponent();
        }
    }
    assert!(captures > 0, "the test game should have captures");
    let samples = samples_of(&game, 8);
    let features: Vec<_> = samples.iter().map(|s| s.features).collect();
    // The first-move game repeats no position, so every quiet position is a sample.
    assert_eq!(features, quiet);
    assert!(samples.iter().all(|s| s.result == 0.0));
    // From ply 0 the start position comes first: every feature 0.
    let from_start = samples_of(&game, 0);
    assert_eq!(from_start[0].features, [0.0; 8]);
    assert!(from_start.len() > samples.len());
    // The label follows the winner.
    let white_won = GameRecord {
        winner: Some(Color::White),
        ..game.clone()
    };
    assert!(samples_of(&white_won, 8).iter().all(|s| s.result == 1.0));
    let drawn = GameRecord {
        winner: None,
        ..game
    };
    assert!(samples_of(&drawn, 8).iter().all(|s| s.result == 0.5));
}

#[test]
fn the_fit_finds_known_weights_and_k_again() {
    // Labels sigmoid(K · value) with known K and weights: the error is 0 there and only there.
    let k = 0.8;
    let truth = to_array(&SOME);
    let samples: Vec<Sample> = positions()
        .iter()
        .map(|board| {
            let features = features_of(board);
            let result = 1.0 / (1.0 + (-k * value(&features, &truth)).exp());
            Sample { features, result }
        })
        .collect();
    assert!((fit_k(&samples, &SOME) - k).abs() < 1e-4);
    assert!(mse(&samples, &SOME, k) < 1e-30);
    let tuning = tune(&samples, &Weights::TUNED, k, &["man"]);
    let found = to_array(&tuning.weights);
    for (i, key) in Weights::KEYS.iter().enumerate() {
        assert!(
            (found[i] - truth[i]).abs() < 0.002,
            "{key}: {} instead of {}",
            found[i],
            truth[i]
        );
    }
    assert!(tuning.error < mse(&samples, &Weights::TUNED, k) / 100.0);
    // `man` stays where it started.
    assert_eq!(tuning.weights.man, 1.0);
    let half_man = Weights {
        man: 0.5,
        ..Weights::TUNED
    };
    assert_eq!(tune(&samples, &half_man, k, &["man"]).weights.man, 0.5);
}
