//! Times perft from the start position, for depths 1 up to the given depth (default 8).
//!
//!     cargo build --release --example perft
//!     taskset -c 2 target/release/examples/perft 8
//!
//! The method is the same as in bench/perft.js, which times engine.js:
//! - one warm-up run of depth 5;
//! - a sample is k back-to-back runs; k starts at 1 and doubles until a sample lasts at least
//!   50 ms, and that sample counts as the first one;
//! - 5 samples for depths 1-6 and 3 for depths 7-8; the time per run is the median sample / k.
//!
//! Output: one tab-separated line per depth: depth, leaves, nanoseconds per run, k, samples.

use std::hint::black_box;
use std::time::{Duration, Instant};

use engine::{Board, Color, perft};

/// Runs perft `k` times; returns the time taken and the leaf count.
fn sample(board: &Board, depth: u32, k: u32) -> (Duration, u64) {
    let start = Instant::now();
    let mut leaves = 0;
    for _ in 0..k {
        leaves = black_box(perft(black_box(board), Color::White, depth));
    }
    (start.elapsed(), leaves)
}

fn main() {
    let max_depth: u32 = std::env::args()
        .nth(1)
        .map_or(8, |s| s.parse().expect("the argument is the maximum depth"));
    let board = Board::start();
    sample(&board, 5, 1); // warm-up
    println!("depth\tleaves\tns_per_run\tk\tsamples");
    for depth in 1..=max_depth {
        let mut k = 1;
        let (mut first, mut leaves) = sample(&board, depth, k);
        while first < Duration::from_millis(50) {
            k *= 2;
            (first, leaves) = sample(&board, depth, k);
        }
        let count = if depth <= 6 { 5 } else { 3 };
        let mut times = vec![first];
        while times.len() < count {
            times.push(sample(&board, depth, k).0);
        }
        times.sort();
        let ns_per_run = times[count / 2].as_nanos() / u128::from(k);
        println!("{depth}\t{leaves}\t{ns_per_run}\t{k}\t{count}");
    }
}
