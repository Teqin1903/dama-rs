//! Referee 1 of CLAUDE.md: perft from the start position equals kish 1.1.2 for depths 1-8
//! (testdata/perft.json). Depths 7 and 8 are slow: `cargo test --release -- --ignored`.

mod common;

use std::collections::BTreeMap;

use engine::{Board, perft};
use serde::Deserialize;

/// testdata/perft.json. Its depth 9 comes from our JS engine only, so it is not a referee.
#[derive(Deserialize)]
struct PerftData {
    board: String,
    side: String,
    counts: BTreeMap<u32, u64>,
}

fn check(depth: u32) {
    let data: PerftData = serde_json::from_str(&common::read_testdata("perft.json")).unwrap();
    let board: Board = data.board.parse().unwrap();
    let expected = data.counts[&depth];
    assert_eq!(
        perft(&board, common::side(&data.side), depth),
        expected,
        "perft {depth}"
    );
}

#[test]
fn perft_1_to_6() {
    for depth in 1..=6 {
        check(depth);
    }
}

#[test]
#[ignore = "slow: cargo test --release -- --ignored"]
fn perft_7() {
    check(7);
}

#[test]
#[ignore = "slow: cargo test --release -- --ignored"]
fn perft_8() {
    check(8);
}
