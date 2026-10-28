//! How deep iterative deepening gets in a fixed time per move, on the 541 fixed positions of
//! testdata/decisions.json (with their weights) and testdata/movegen.json (with the tuned weights).
//!
//!     cargo build --release --example depth
//!     taskset -c 2 target/release/examples/depth 10 50 200
//!
//! Each argument is a time limit in milliseconds per move. One pass over all positions per limit,
//! after one warm-up pass of alpha-beta at depth 3. The results depend on the clock and the
//! machine, so two runs can differ by an iteration here and there.
//!
//! Output: one tab-separated line per limit: the limit, positions, the mean, lowest and highest
//! completed depth, the mean nodes per move, the mean and longest time per move in milliseconds,
//! and how many positions completed each depth (`depth:count`).

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use engine::{Board, Color, Limits, Weights, alpha_beta, iterative_deepening};
use serde::Deserialize;

#[derive(Deserialize)]
struct Position {
    board: String,
    side: String,
    weights: Option<JsWeights>,
}

#[derive(Deserialize)]
struct JsWeights {
    man: f64,
    king: f64,
    mob: f64,
}

fn positions() -> Vec<(Board, Color, Weights)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testdata");
    let mut all = Vec::new();
    for file in ["decisions.json", "movegen.json"] {
        let text = std::fs::read_to_string(dir.join(file)).expect("cannot read testdata");
        let entries: Vec<Position> = serde_json::from_str(&text).expect("bad testdata");
        for p in entries {
            let side = if p.side == "white" {
                Color::White
            } else {
                Color::Black
            };
            let weights = p.weights.map_or(Weights::TUNED, |w| Weights {
                man: w.man,
                king: w.king,
                mob: w.mob,
            });
            all.push((p.board.parse().expect("bad board"), side, weights));
        }
    }
    all
}

fn main() {
    let positions = positions();
    for (board, side, weights) in &positions {
        alpha_beta(board, *side, *weights, 3); // warm-up
    }
    println!(
        "ms\tpositions\tmean_depth\tmin_depth\tmax_depth\tnodes_per_move\tmean_ms\tmax_ms\tdepths"
    );
    for arg in std::env::args().skip(1) {
        let ms: u64 = arg.parse().expect("expected a time in milliseconds");
        let limits = Limits {
            time: Some(Duration::from_millis(ms)),
            ..Limits::default()
        };
        let mut depths = BTreeMap::new();
        let (mut nodes, mut total, mut longest) = (0, Duration::ZERO, Duration::ZERO);
        for (board, side, weights) in &positions {
            let start = Instant::now();
            let id = iterative_deepening(board, *side, *weights, limits);
            let took = start.elapsed();
            *depths.entry(id.depth).or_insert(0) += 1;
            nodes += id.decision.nodes;
            total += took;
            longest = longest.max(took);
        }
        let n = positions.len() as f64;
        let sum: u32 = depths.iter().map(|(d, count)| d * count).sum();
        let histogram: Vec<String> = depths.iter().map(|(d, c)| format!("{d}:{c}")).collect();
        println!(
            "{ms}\t{}\t{:.2}\t{}\t{}\t{:.0}\t{:.2}\t{:.2}\t{}",
            positions.len(),
            f64::from(sum) / n,
            depths.keys().next().unwrap(),
            depths.keys().next_back().unwrap(),
            nodes as f64 / n,
            total.as_secs_f64() * 1e3 / n,
            longest.as_secs_f64() * 1e3,
            histogram.join(" ")
        );
    }
}
