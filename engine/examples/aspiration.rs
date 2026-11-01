//! How much the best value moves from one iteration to the next, on the 541 fixed positions of
//! testdata/decisions.json and testdata/movegen.json, and how often a window around the previous
//! value would have held: what `:asp=W` and the depth it starts from are chosen from.
//!
//!     cargo build --release --example aspiration
//!     target/release/examples/aspiration idqrotf:n20000:w=texel1:fs=1 0.05 0.1 0.25 0.5 1
//!
//! The first argument is an iterative-deepening agent without `:asp`; the others are half-widths W.
//! A depth d's row covers the positions that completed d, with both values outside the win zone
//! (where the window stays open). A window holds when L <= best - 2 * U_EPSILON and best < H.
//!
//! Output: one tab-separated line per depth: depth, positions, how many had a win value on either
//! side, the 50th, 90th and 99th percentile of |change|, the share of the nodes of all completed
//! iterations spent at this depth, and for each W the share of positions whose window held; then the
//! same for all depths from `Aspiration::FROM_DEPTH` on.
//!
//! Then the same agent really searching with each `:asp=W`, one line per W: the completed iterations
//! that had a window, the share whose first window held, the mean failed tries per such iteration, the
//! largest number of failed tries, and the share of the agent's nodes in failed tries of completed
//! iterations. An interrupted iteration is not counted: the recorder keeps only completed ones.

use std::path::Path;

use engine::{
    AgentSpec, Aspiration, Board, Color, DrawRules, Limits, Recorder, SearchOptions, Table,
    U_EPSILON, WIN, Weights, iterative_deepening_traced, native_clock, percentile,
};
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

struct Spec {
    limits: Limits,
    options: SearchOptions,
    repetition: bool,
    table: Option<engine::TableOptions>,
    weights: Weights,
}

fn spec(name: &str) -> Spec {
    match name.parse::<AgentSpec>() {
        Ok(AgentSpec::Deepening {
            limits,
            quiescence,
            repetition,
            order,
            table,
            draws,
            pvs,
            extend,
            weights,
            ..
        }) => Spec {
            limits,
            options: SearchOptions {
                quiescence,
                order,
                draws: draws.map(|rules| DrawRules { quiet: 0, ..rules }),
                pvs,
                extend,
                ..SearchOptions::default()
            },
            repetition,
            table,
            weights,
        },
        _ => panic!("{name}: expected an iterative-deepening agent"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let spec = spec(&args[0]);
    let widths: Vec<f64> = args[1..].iter().map(|w| w.parse().expect("W")).collect();
    let zone = WIN / 2.0;
    let mut table = spec.table.map(Table::new);
    // Per depth: the (previous, best) pairs, the pairs with a win value, the nodes.
    let mut pairs: Vec<Vec<(f64, f64)>> = vec![Vec::new(); 65];
    let mut wins = [0u32; 65];
    let mut nodes = [0u64; 65];
    let boards = positions();
    for (board, side) in &boards {
        let (board, side) = (*board, *side);
        if let Some(table) = &mut table {
            table.clear();
        }
        let mut recorder = Recorder::new(native_clock, None);
        let history = spec.repetition.then_some(&[][..]);
        iterative_deepening_traced(
            &board,
            side,
            spec.weights,
            spec.limits,
            spec.options,
            history,
            table.as_mut(),
            None,
            &mut recorder,
            native_clock,
        );
        let mut previous: Option<(f64, u64)> = None;
        for iteration in &recorder.iterations {
            let best = (iteration.root.iter())
                .map(|m| m.value)
                .fold(f64::NEG_INFINITY, f64::max);
            let d = iteration.depth as usize;
            nodes[d] += iteration.nodes - previous.map_or(0, |(_, n)| n);
            if let Some((before, _)) = previous {
                if before.abs() > zone || best.abs() > zone {
                    wins[d] += 1;
                } else {
                    pairs[d].push((before, best));
                }
            }
            previous = Some((best, iteration.nodes));
        }
    }
    let total: u64 = nodes.iter().sum();
    let heads: Vec<String> = widths.iter().map(|w| format!("holds_{w}")).collect();
    println!(
        "depth\tpositions\twins\tp50\tp90\tp99\tnode_share\t{}",
        heads.join("\t")
    );
    let from = Aspiration::FROM_DEPTH as usize;
    for d in 2..65 {
        if !pairs[d].is_empty() {
            row(&d.to_string(), &pairs[d], wins[d], nodes[d], total, &widths);
        }
    }
    let late = pairs[from..].concat();
    let late_wins = wins[from..].iter().sum();
    let late_nodes = nodes[from..].iter().sum();
    row(
        &format!("{from}+"),
        &late,
        late_wins,
        late_nodes,
        total,
        &widths,
    );
    println!("W\titerations\tfirst_held\tfailed_per_iteration\tmost_failed\tfailed_node_share");
    for w in &widths {
        searched(&spec, *w, &boards);
    }
}

/// One line of the first table.
fn row(name: &str, pairs: &[(f64, f64)], wins: u32, nodes: u64, total: u64, widths: &[f64]) {
    let mut changes: Vec<f64> = pairs.iter().map(|(a, b)| (b - a).abs()).collect();
    changes.sort_by(f64::total_cmp);
    let holds: Vec<String> = widths
        .iter()
        .map(|w| format!("{:.3}", held(pairs, *w)))
        .collect();
    println!(
        "{name}\t{}\t{wins}\t{:.4}\t{:.4}\t{:.4}\t{:.3}\t{}",
        pairs.len(),
        percentile(&changes, 0.5),
        percentile(&changes, 0.9),
        percentile(&changes, 0.99),
        nodes as f64 / total as f64,
        holds.join("\t")
    );
}

/// One line of the second table: `spec` with `:asp=W` on `boards`.
fn searched(spec: &Spec, half_width: f64, boards: &[(Board, Color)]) {
    let options = SearchOptions {
        aspiration: Some(Aspiration {
            half_width,
            from_depth: Aspiration::FROM_DEPTH,
        }),
        ..spec.options
    };
    let mut table = spec.table.map(Table::new);
    let (mut iterations, mut first_held, mut failed, mut most) = (0u64, 0u64, 0u64, 0);
    let (mut failed_nodes, mut all_nodes) = (0u64, 0u64);
    for (board, side) in boards {
        if let Some(table) = &mut table {
            table.clear();
        }
        let mut recorder = Recorder::new(native_clock, None);
        let id = iterative_deepening_traced(
            board,
            *side,
            spec.weights,
            spec.limits,
            options,
            spec.repetition.then_some(&[][..]),
            table.as_mut(),
            None,
            &mut recorder,
            native_clock,
        );
        all_nodes += id.decision.nodes;
        let mut previous: Option<(f64, u64)> = None;
        for iteration in &recorder.iterations {
            let best = (iteration.root.iter())
                .map(|m| m.value)
                .fold(f64::NEG_INFINITY, f64::max);
            let start = previous.map_or(0, |(_, n)| n);
            let had_window = iteration.depth >= Aspiration::FROM_DEPTH
                && previous.is_some_and(|(v, _)| v.abs() <= WIN / 2.0);
            if had_window {
                iterations += 1;
                first_held += u64::from(iteration.attempts.is_empty());
                failed += iteration.attempts.len() as u64;
                most = most.max(iteration.attempts.len());
                failed_nodes += iteration.attempts.last().map_or(0, |a| a.nodes - start);
            }
            previous = Some((best, iteration.nodes));
        }
    }
    println!(
        "{half_width}\t{iterations}\t{:.3}\t{:.3}\t{most}\t{:.3}",
        first_held as f64 / iterations as f64,
        failed as f64 / iterations as f64,
        failed_nodes as f64 / all_nodes as f64
    );
}

/// The share of `pairs` whose window of half-width `w` around the previous value holds.
fn held(pairs: &[(f64, f64)], w: f64) -> f64 {
    let hits = pairs
        .iter()
        .filter(|(before, best)| before - w <= best - 2.0 * U_EPSILON && *best < before + w)
        .count();
    hits as f64 / pairs.len() as f64
}
