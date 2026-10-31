//! Times agents on fixed positions, for Monte Carlo tree search also its iterations (random games
//! for `mc` and `uct`): the 541 positions of `examples/search` (testdata/decisions.json and
//! movegen.json) with more than one legal move (383), each searched once from a new game (seed 1, the
//! game's clock at ply 0), the agent's own weights.
//!
//!     cargo build --release --example mcts
//!     taskset -c 2 target/release/examples/mcts mc:n20000 uct:n20000 idqrot:n20000:w=texel1 ab5
//!
//! Output: one tab-separated line per agent: agent, positions, nodes per move, iterations per
//! move (empty unless Monte Carlo tree search), milliseconds per move, the deepest tree ply (or
//! depth) per move.

use std::path::Path;
use std::time::{Duration, Instant};

use engine::{AgentSpec, Board, Clock, Color, Mulberry32, derive_seed, mcts};
use serde::Deserialize;

#[derive(Deserialize)]
struct Position {
    board: String,
    side: String,
}

fn positions() -> Vec<(Board, Color)> {
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
            all.push((p.board.parse().expect("bad board"), side));
        }
    }
    all
}

fn main() {
    let positions: Vec<(Board, Color)> = (positions().into_iter())
        .filter(|(b, s)| b.legal_moves(*s).len() > 1)
        .collect();
    println!("agent\tpositions\tnodes/move\titerations/move\tms/move\tdepth/move");
    for name in std::env::args().skip(1) {
        let spec: AgentSpec = name.parse().unwrap_or_else(|e| panic!("{e}"));
        let (mut nodes, mut iterations, mut depth) = (0u64, 0u64, 0u64);
        let mut time = Duration::ZERO;
        for (k, (board, side)) in positions.iter().enumerate() {
            let legal = board.legal_moves(*side);
            let seed = derive_seed(1, 0, k as u32);
            if let AgentSpec::Mcts {
                kind,
                limits,
                weights,
            } = spec
            {
                let mut rng = Mulberry32::new(seed);
                let start = Instant::now();
                let r = mcts(
                    kind,
                    board,
                    *side,
                    &legal,
                    Clock::default(),
                    limits,
                    weights,
                    &mut rng,
                );
                time += start.elapsed();
                (nodes, iterations, depth) = (
                    nodes + r.nodes,
                    iterations + r.iterations,
                    depth + u64::from(r.depth),
                );
            } else {
                let mut agent = spec.build();
                agent.new_game(seed).unwrap();
                agent.observe(board, *side);
                let start = Instant::now();
                agent.choose(board, *side, &legal).unwrap();
                time += start.elapsed();
                (nodes, depth) = (nodes + agent.nodes(), depth + agent.depths());
            }
        }
        let n = positions.len() as f64;
        let iterations = match spec {
            AgentSpec::Mcts { .. } => format!("{:.1}", iterations as f64 / n),
            _ => String::new(),
        };
        println!(
            "{name}\t{}\t{:.1}\t{iterations}\t{:.2}\t{:.2}",
            positions.len(),
            nodes as f64 / n,
            time.as_secs_f64() * 1e3 / n,
            depth as f64 / n
        );
    }
}
