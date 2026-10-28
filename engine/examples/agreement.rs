//! How often a search's decision agrees with a deeper reference search, on the 541 fixed
//! positions of testdata/decisions.json (with their weights) and testdata/movegen.json (with the
//! tuned weights).
//!
//!     cargo build --release --example agreement
//!     target/release/examples/agreement ab6 ab3 ab3q ab4 ab5
//!
//! The first argument is the reference; the others are compared with it. Names as in the search
//! example: `mmN`, `abN`, `idN`, each optionally with `q` (quiescence). The agents break ties at
//! random among `top`, so a position counts with the share of its `top` moves that are also in the
//! reference's `top`: the chance that the agent plays one of the reference's best moves.
//!
//! Output: one tab-separated line per search: search, positions, that chance in percent (mean over
//! the positions), the percentage of positions where every `top` move is one of the reference's,
//! and where at least one is.

use std::path::Path;

use engine::{
    Board, Color, Decision, Limits, Weights, alpha_beta, alpha_beta_q, iterative_deepening,
    minimax, minimax_q,
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

type Search = fn(&Board, Color, Weights, u32) -> Decision;

fn deepening(board: &Board, side: Color, weights: Weights, depth: u32) -> Decision {
    let limits = Limits {
        depth: Some(depth),
        ..Limits::default()
    };
    iterative_deepening(board, side, weights, limits, false).decision
}

fn deepening_q(board: &Board, side: Color, weights: Weights, depth: u32) -> Decision {
    let limits = Limits {
        depth: Some(depth),
        ..Limits::default()
    };
    iterative_deepening(board, side, weights, limits, true).decision
}

/// The search a name stands for, and its depth.
fn search(name: &str) -> (Search, u32) {
    let searches: [(&str, Search, Search); 3] = [
        ("mm", minimax, minimax_q),
        ("ab", alpha_beta, alpha_beta_q),
        ("id", deepening, deepening_q),
    ];
    let (search, depth) = searches
        .iter()
        .find_map(|&(prefix, plain, with_q)| {
            let rest = name.strip_prefix(prefix)?;
            Some(match rest.strip_suffix('q') {
                Some(depth) => (with_q, depth),
                None => (plain, rest),
            })
        })
        .unwrap_or_else(|| panic!("{name}: expected mmN, abN or idN, optionally with q"));
    (search, depth.parse().expect("expected a depth"))
}

/// The decision of the search called `name` on every position.
fn decisions(positions: &[(Board, Color, Weights)], name: &str) -> Vec<Decision> {
    let (search, depth) = search(name);
    positions
        .iter()
        .map(|(board, side, weights)| search(board, *side, *weights, depth))
        .collect()
}

fn main() {
    let names: Vec<String> = std::env::args().skip(1).collect();
    let (reference, others) = names
        .split_first()
        .expect("usage: agreement REFERENCE SEARCH...");
    let positions = positions();
    let truth = decisions(&positions, reference);
    println!("search\tpositions\tchance_pct\tall_top_pct\tany_top_pct\treference");
    for name in others {
        let (mut chance, mut all, mut any) = (0.0, 0, 0);
        for (decision, best) in decisions(&positions, name).iter().zip(&truth) {
            let shared = decision.top.iter().filter(|i| best.top.contains(i)).count();
            chance += shared as f64 / decision.top.len() as f64;
            all += usize::from(shared == decision.top.len());
            any += usize::from(shared > 0);
        }
        let n = positions.len() as f64;
        println!(
            "{name}\t{}\t{:.2}\t{:.2}\t{:.2}\t{reference}",
            positions.len(),
            100.0 * chance / n,
            100.0 * all as f64 / n,
            100.0 * any as f64 / n
        );
    }
}
