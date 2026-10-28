//! The positional features of U (engine/src/eval.rs): their values on hand-made positions, `evaluate`
//! with one weight at a time, and colour symmetry: a board turned upside down with the colours
//! swapped is the same position for the other side, so every feature and value must be the same.

mod common;

use engine::{Board, Color, Features, Piece, Positional, Square, Weights, alpha_beta_q, evaluate};

/// Every positional weight set to a value that is not 0, the tuned weights for the rest.
const ALL_ON: Weights = Weights {
    adv: 0.1,
    back: -0.3,
    cen: 0.07,
    edge: -0.11,
    qmob: 0.013,
    ..Weights::TUNED
};

/// White: men d6, a2, b2 and king c4. Black: men e4, h7 and king h1.
///
/// From White's view:
/// - adv: White d6 4 rows up from rank 2, a2 and b2 0; Black e4 3 rows down from rank 7, h7 0:
///   4 − 3 = 1.
/// - back: White a2 and b2, Black h7: 2 − 1 = 1.
/// - cen: White d6 and c4, Black e4: 2 − 1 = 1.
/// - edge: White a2, Black h7 and h1: 1 − 2 = −1.
/// - qmob: White d6 3 (d7, e6, c6), a2 1 (a3), b2 2 (b3, c2), king c4 10 (c5–c8, c3–c1, d4,
///   b4, a4): 16. Black e4 3 (e3, f4, d4), h7 2 (h6, g7), king h1 12 (h2–h6, g1–a1): 17.
///   16 − 17 = −1, although White must capture: its only legal move is c4xh4xh8 (e4, then h7), so
///   U's `mob` counts 1 move for White.
const MIXED: &str = "......../.......b/...w..../......../..W.b.../......../ww....../.......B";

fn board(s: &str) -> Board {
    s.parse().unwrap()
}

#[test]
fn features_of_a_hand_made_position() {
    let mixed = board(MIXED);
    let white = Positional {
        adv: 1,
        back: 1,
        cen: 1,
        edge: -1,
        qmob: -1,
    };
    assert_eq!(Positional::of(&mixed, Color::White), white);
    let black = Positional {
        adv: -1,
        back: -1,
        cen: -1,
        edge: 1,
        qmob: 1,
    };
    assert_eq!(Positional::of(&mixed, Color::Black), black);
    // The capture is mandatory: U's mob sees only the longest capture.
    let legal = mixed.legal_moves(Color::White);
    let legal: Vec<String> = legal.iter().map(|mv| mv.to_string()).collect();
    assert_eq!(legal, ["c4xh4xh8"]);
}

#[test]
fn advancement_counts_rows_from_the_home_row() {
    // White man on rank 7 (5 rows up), Black man on rank 2 (5 rows down), White man on rank 4 (2).
    let far = board("......../...w..../......../......../...w..../......../b......./........");
    assert_eq!(Positional::of(&far, Color::White).adv, 5 + 2 - 5);
    // Kings do not advance.
    let kings = board("......../...W..../......../......../......../......../B......./........");
    assert_eq!(Positional::of(&kings, Color::White).adv, 0);
}

#[test]
fn the_start_position_is_even() {
    // Ranks 2 and 3 against ranks 7 and 6: every feature is the same for both sides.
    for side in [Color::White, Color::Black] {
        assert_eq!(Positional::of(&Board::start(), side), Positional::default());
    }
}

#[test]
fn each_weight_adds_its_feature_to_u() {
    let mixed = board(MIXED);
    let features = Features::of(&mixed, Color::White);
    let base = features.u(Weights::TUNED);
    for (key, value) in [
        ("adv", 1.0),
        ("back", 1.0),
        ("cen", 1.0),
        ("edge", -1.0),
        ("qmob", -1.0),
    ] {
        let weights = Weights::TUNED
            .with_overrides(&format!("{key}=0.5"))
            .unwrap();
        let u = evaluate(&mixed, Color::White, &features, weights);
        assert_eq!(u, base + 0.5 * value, "{key}");
    }
    // With every positional weight 0, evaluate is U, bit for bit.
    assert_eq!(
        evaluate(&mixed, Color::White, &features, Weights::TUNED),
        base
    );
}

#[test]
fn a_win_is_worth_win_whatever_the_positional_weights() {
    // White has just taken Black's last piece.
    let won = board("......../......../...w..../......../......../......../......../........");
    let features = Features::of(&won, Color::White);
    assert!(features.win);
    assert_eq!(evaluate(&won, Color::White, &features, ALL_ON), engine::WIN);
}

/// The board turned upside down (rank r to rank 9 − r) with the colours swapped.
fn flip(board: &Board) -> Board {
    let mut flipped = Board::empty();
    for file in 0..8 {
        for rank in 0..8 {
            let piece = board.piece_at(Square::new(file, rank)).map(|p| Piece {
                color: p.color.opponent(),
                kind: p.kind,
            });
            flipped.set(Square::new(file, 7 - rank), piece);
        }
    }
    flipped
}

/// The board in a mirror (file a to file h), the colours kept.
fn mirror(board: &Board) -> Board {
    let mut mirrored = Board::empty();
    for file in 0..8 {
        for rank in 0..8 {
            mirrored.set(
                Square::new(7 - file, rank),
                board.piece_at(Square::new(file, rank)),
            );
        }
    }
    mirrored
}

/// The 541 fixed positions: the 141 of decisions.json, then the 400 of movegen.json.
fn positions() -> Vec<(Board, Color)> {
    let mut all = Vec::new();
    for file in ["decisions.json", "movegen.json"] {
        let entries: Vec<serde_json::Value> =
            serde_json::from_str(&common::read_testdata(file)).unwrap();
        for entry in entries {
            let board: Board = entry["board"].as_str().unwrap().parse().unwrap();
            all.push((board, common::side(entry["side"].as_str().unwrap())));
        }
    }
    assert_eq!(all.len(), 541);
    all
}

#[test]
fn the_features_do_not_depend_on_colour_or_side_of_the_board() {
    for (board, _) in positions() {
        let (flipped, mirrored) = (flip(&board), mirror(&board));
        assert_eq!(flip(&flipped), board);
        for side in [Color::White, Color::Black] {
            let ours = Positional::of(&board, side);
            assert_eq!(Positional::of(&flipped, side.opponent()), ours, "{board:?}");
            assert_eq!(Positional::of(&mirrored, side), ours, "{board:?}");
            // The opponent's view is ours negated.
            let theirs = Positional::of(&board, side.opponent());
            assert_eq!(
                (
                    theirs.adv,
                    theirs.back,
                    theirs.cen,
                    theirs.edge,
                    theirs.qmob
                ),
                (-ours.adv, -ours.back, -ours.cen, -ours.edge, -ours.qmob)
            );
            let features = Features::of(&board, side);
            let flipped_features = Features::of(&flipped, side.opponent());
            assert_eq!(flipped_features, features);
            assert_eq!(
                evaluate(&flipped, side.opponent(), &flipped_features, ALL_ON),
                evaluate(&board, side, &features, ALL_ON),
                "{board:?}"
            );
        }
    }
}

#[test]
fn a_search_with_every_weight_on_is_colour_symmetric() {
    // The best value is exact, so it is the same double; the moves come in another order.
    for (board, side) in positions().into_iter().take(141) {
        let ours = alpha_beta_q(&board, side, ALL_ON, 2);
        let theirs = alpha_beta_q(&flip(&board), side.opponent(), ALL_ON, 2);
        let best = |d: &engine::Decision| d.candidates[d.top[0]].u;
        assert_eq!(best(&theirs), best(&ours), "{board:?}");
        assert_eq!(theirs.top.len(), ours.top.len(), "{board:?}");
    }
}
