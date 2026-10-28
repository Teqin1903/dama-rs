//! Times the agents' search on fixed positions: the 141 of testdata/decisions.json (with their
//! weights) and the 400 of testdata/movegen.json (with the tuned weights).
//!
//!     cargo build --release --example search
//!     taskset -c 2 target/release/examples/search mm3 ab3 ab4 ab5 ab6 id5
//!
//! Each argument is a search: `mmN` (minimax), `abN` (alpha-beta) or `idN` (iterative deepening
//! up to N plies, no node or time limit), N plies deep. One pass over
//! all positions is a sample; 3 samples per search, after one warm-up pass, and the time per move
//! is the median sample / the number of positions.
//!
//! Output: one tab-separated line per search: search, positions, nodes per move, microseconds per
//! move, and the slowest position's time in microseconds (from the median sample).

use std::hint::black_box;
use std::path::Path;
use std::time::{Duration, Instant};

use engine::{Board, Color, Decision, Limits, Weights, alpha_beta, iterative_deepening, minimax};
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

type Search = fn(&Board, Color, Weights, u32) -> Decision;

/// Iterative deepening up to `depth` plies, with no other limit.
fn deepening(board: &Board, side: Color, weights: Weights, depth: u32) -> Decision {
    let limits = Limits {
        depth: Some(depth),
        ..Limits::default()
    };
    iterative_deepening(board, side, weights, limits).decision
}

/// One pass over `positions`: the total time, the slowest position's time and the total nodes.
fn sample(
    positions: &[(Board, Color, Weights)],
    search: Search,
    depth: u32,
) -> (Duration, Duration, u64) {
    let (mut total, mut slowest, mut nodes) = (Duration::ZERO, Duration::ZERO, 0);
    for (board, side, weights) in positions {
        let start = Instant::now();
        let decision = black_box(search(black_box(board), *side, *weights, depth));
        let took = start.elapsed();
        total += took;
        slowest = slowest.max(took);
        nodes += decision.nodes;
    }
    (total, slowest, nodes)
}

fn main() {
    let positions = positions();
    println!("search\tpositions\tnodes_per_move\tus_per_move\tslowest_us");
    for name in std::env::args().skip(1) {
        let (search, depth): (Search, &str) = if let Some(d) = name.strip_prefix("mm") {
            (minimax, d)
        } else if let Some(d) = name.strip_prefix("ab") {
            (alpha_beta, d)
        } else if let Some(d) = name.strip_prefix("id") {
            (deepening, d)
        } else {
            panic!("{name}: expected mmN, abN or idN");
        };
        let depth: u32 = depth.parse().expect("expected a depth after mm, ab or id");
        sample(&positions, search, depth); // warm-up
        let mut samples: Vec<_> = (0..3).map(|_| sample(&positions, search, depth)).collect();
        samples.sort();
        let (total, slowest, nodes) = samples[1];
        let n = positions.len();
        println!(
            "{name}\t{n}\t{:.1}\t{:.1}\t{:.0}",
            nodes as f64 / n as f64,
            total.as_secs_f64() * 1e6 / n as f64,
            slowest.as_secs_f64() * 1e6
        );
    }
}
