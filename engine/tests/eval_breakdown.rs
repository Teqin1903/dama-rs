//! The evaluation broken down (`breakdown` in engine/src/eval.rs): its total is the value the
//! search scores a leaf with, and its features and pieces add up to it.

mod common;

use engine::{Board, Color, Kind, Mulberry32, WIN, Weights, breakdown, static_value};

/// The 541 fixed positions, and the positions of a few random games, with both sides to move.
fn boards() -> Vec<(String, Board, Color)> {
    let mut all: Vec<(String, Board, Color)> = common::positions()
        .into_iter()
        .map(|(label, board, side, _)| (label, board, side))
        .collect();
    let mut rng = Mulberry32::new(7);
    for game in 0..20 {
        let (mut board, mut side) = (Board::start(), Color::White);
        for ply in 0..120 {
            let legal = board.legal_moves(side);
            if legal.is_empty() {
                break;
            }
            all.push((format!("game {game} ply {ply}"), board, side));
            board = board.apply(&legal[rng.next_index(legal.len())]);
            side = side.opponent();
        }
    }
    all
}

#[test]
fn the_total_is_the_searchs_static_value_and_the_parts_add_up_to_it() {
    let weights = ["tuned", "texel1", "texel2", "material"].map(|n| Weights::named(n).unwrap());
    for (label, board, to_move) in boards() {
        for w in weights {
            for side in [to_move, to_move.opponent()] {
                let b = breakdown(&board, to_move, side, w);
                assert_eq!(b.side, side);
                // Bit for bit the value the search uses, a win or loss one ply deep included.
                assert_eq!(
                    b.total.to_bits(),
                    static_value(&board, to_move, side, w).to_bits(),
                    "{label}"
                );
                if board.legal_moves(to_move).is_empty() {
                    let total = if side == to_move { -WIN } else { WIN };
                    assert_eq!(
                        (b.total, b.win),
                        (total, Some(to_move.opponent())),
                        "{label}"
                    );
                    continue;
                }
                assert_eq!(b.win, None, "{label}");
                // The other view is the negation.
                let other = breakdown(&board, to_move, side.opponent(), w);
                assert_eq!(other.total, -b.total, "{label}");
                assert!(b.features.iter().all(|f| f.weight != 0.0), "{label}");
                let features: f64 = b.features.iter().map(|f| f.contribution).sum();
                assert!(
                    (features - b.total).abs() < 1e-9,
                    "{label}: {features} vs {}",
                    b.total
                );
                // The pieces carry every feature but mobility.
                let pieces: f64 = b.squares.iter().map(|s| s.contribution).sum();
                let mobility: f64 = (b.features.iter())
                    .filter(|f| f.key == "mob")
                    .map(|f| f.contribution)
                    .sum();
                assert!(
                    (pieces + mobility - b.total).abs() < 1e-9,
                    "{label}: pieces {pieces} + mobility {mobility} vs {}",
                    b.total
                );
                let on_board: u32 = [Color::White, Color::Black]
                    .into_iter()
                    .flat_map(|c| [Kind::Man, Kind::King].map(|k| board.count(c, k)))
                    .sum();
                assert_eq!(b.squares.len(), on_board as usize, "{label}");
            }
        }
    }
}

#[test]
fn features_come_in_the_engines_order_with_each_colours_count() {
    let mut w = Weights::named("texel1").unwrap();
    w.mob = 0.1;
    let b = breakdown(&Board::start(), Color::White, Color::White, w);
    let keys: Vec<&str> = b.features.iter().map(|f| f.key).collect();
    assert_eq!(
        keys,
        ["man", "king", "mob", "adv", "back", "cen", "edge", "qmob"]
    );
    let men = b.features[0];
    assert_eq!(
        (men.name, men.white, men.black, men.weight, men.contribution),
        ("men", 16, 16, 1.0, 0.0)
    );
    // The start is symmetric: every contribution is 0, so the total is too.
    assert!(b.features.iter().all(|f| f.contribution == 0.0));
    assert_eq!(b.total, 0.0);
}
