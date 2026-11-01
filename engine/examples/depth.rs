//! How deep iterative deepening gets within a budget per move, on the 541 fixed positions of
//! testdata/decisions.json (with their weights) and testdata/movegen.json (with the tuned weights).
//!
//!     cargo build --release --example depth
//!     taskset -c 2 target/release/examples/depth 10 50 200
//!     taskset -c 2 target/release/examples/depth idqr:n20000:w=texel1 idqro:n20000:w=texel1
//!
//! Each argument is a time limit in milliseconds per move (`id:tMS` with each position's weights),
//! or the name of an iterative-deepening agent (`id:...`, `idqro:...`, as in `dama arena`), which
//! searches with its own limits, options, table and weights, on an empty game (a table is emptied
//! before each position). One pass over all positions
//! per argument, after one warm-up pass of alpha-beta at depth 3. With a time limit the results
//! depend on the clock and the machine, so two runs can differ by an iteration here and there; with
//! a node or depth limit they are the same every time.
//!
//! Output: one tab-separated line per argument: the argument, positions, the mean, lowest and highest
//! completed depth, the mean nodes per move, the mean and longest time per move in milliseconds,
//! how many positions completed each depth (`depth:count`), and how many stopped by proof, whose
//! last iteration reached no leaf at the depth limit, with the mean depth of the others. A proof
//! ends a search early, so an agent that finds more of them shows a lower mean depth for it.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::{Duration, Instant};

use engine::{
    AgentSpec, Aspiration, Board, Color, Limits, SearchOptions, Table, Weights, alpha_beta,
    iterative_deepening_with,
};
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
                ..Weights::ZERO
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
        "search\tpositions\tmean_depth\tmin_depth\tmax_depth\tnodes_per_move\tmean_ms\tmax_ms\tdepths\tproved\tmean_depth_unproved"
    );
    for arg in std::env::args().skip(1) {
        // A number is a time limit with each position's weights; a name brings its own weights.
        let (limits, options, history, own_weights, mut table) = match arg.parse::<u64>() {
            Ok(ms) => {
                let limits = Limits {
                    time: Some(Duration::from_millis(ms)),
                    ..Limits::default()
                };
                (limits, SearchOptions::default(), None, None, None)
            }
            Err(_) => match arg.parse::<AgentSpec>() {
                Ok(AgentSpec::Deepening {
                    limits,
                    quiescence,
                    repetition,
                    order,
                    table,
                    draws,
                    pvs,
                    aspiration,
                    weights,
                }) => {
                    let options = SearchOptions {
                        quiescence,
                        order,
                        draws,
                        pvs,
                        aspiration: aspiration.map(|half_width| Aspiration {
                            half_width,
                            from_depth: Aspiration::FROM_DEPTH,
                        }),
                    };
                    let history = repetition.then_some(&[][..]);
                    (
                        limits,
                        options,
                        history,
                        Some(weights),
                        table.map(Table::new),
                    )
                }
                _ => panic!("{arg}: expected milliseconds or an iterative-deepening agent"),
            },
        };
        let mut depths = BTreeMap::new();
        let (mut nodes, mut total, mut longest) = (0, Duration::ZERO, Duration::ZERO);
        let (mut proved, mut unproved_depths) = (0, 0);
        for (board, side, weights) in &positions {
            // A table starts empty for every position, as at the start of a game.
            if let Some(table) = &mut table {
                table.clear();
            }
            let start = Instant::now();
            let weights = own_weights.unwrap_or(*weights);
            let id = iterative_deepening_with(
                board,
                *side,
                weights,
                limits,
                options,
                history,
                table.as_mut(),
            );
            let took = start.elapsed();
            *depths.entry(id.depth).or_insert(0) += 1;
            if id.proved {
                proved += 1;
            } else {
                unproved_depths += id.depth;
            }
            nodes += id.decision.nodes;
            total += took;
            longest = longest.max(took);
        }
        let n = positions.len() as f64;
        let sum: u32 = depths.iter().map(|(d, count)| d * count).sum();
        let histogram: Vec<String> = depths.iter().map(|(d, c)| format!("{d}:{c}")).collect();
        println!(
            "{arg}\t{}\t{:.2}\t{}\t{}\t{:.0}\t{:.2}\t{:.2}\t{}\t{proved}\t{:.2}",
            positions.len(),
            f64::from(sum) / n,
            depths.keys().next().unwrap(),
            depths.keys().next_back().unwrap(),
            nodes as f64 / n,
            total.as_secs_f64() * 1e3 / n,
            longest.as_secs_f64() * 1e3,
            histogram.join(" "),
            f64::from(unproved_depths) / (n - f64::from(proved))
        );
    }
}
