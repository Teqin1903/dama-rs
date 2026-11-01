//! How far the current engine (V10, `idqrot:w=texel1`) gets in one minute: one search of 60
//! seconds on each of ten fixed positions of testdata/decisions.json (the start, openings,
//! middlegames and endings), and again with a 256 MB table on three of them.
//!
//!     cargo build --release --example minute
//!     taskset -c 2 target/release/examples/minute
//!
//! `--seconds S` sets the time per search (default 60), `--out PATH` the file (default
//! results/minute/minute.json). Each search starts on an empty game with an empty table, as the
//! first move of a game would. The result depends on the machine and the clock: two runs differ in
//! nodes, and here and there by an iteration.
//!
//! Output: the JSON file, with one row per search (name, phase, pieces, kings, nodes, depth, ms,
//! nps, depth_limit_hit: the deepest iteration, `MAX_ID_DEPTH`, completed), the rows with the large
//! table in `hash256`, the medians of the default table's rows in `summary`, the CPU, the version
//! (`git describe`) and the command.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use engine::{
    AgentSpec, Board, Color, Kind, MAX_ID_DEPTH, SearchOptions, Table, TableOptions,
    iterative_deepening_with,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// The positions, by their name in decisions.json, with their phase.
const POSITIONS: [(&str, &str); 10] = [
    ("agent_g12_ply0", "start"),
    ("agent_g0_ply4", "opening"),
    ("agent_g3_ply9", "opening"),
    ("agent_g16_ply18", "middlegame"),
    ("agent_g12_ply40", "middlegame"),
    ("agent_g36_ply28", "middlegame"),
    ("agent_g8_ply52", "endgame"),
    ("agent_g17_ply89", "endgame"),
    ("agent_g41_ply101", "endgame"),
    ("agent_g57_ply49", "endgame"),
];

/// The positions searched again with the large table.
const HASH256: [&str; 3] = ["agent_g12_ply0", "agent_g12_ply40", "agent_g41_ply101"];

/// The engine (V10) with a time budget of `ms` and, if not the default, a table of `mb`.
fn engine(ms: u128, mb: Option<u32>) -> String {
    let hash = mb.map_or(String::new(), |mb| format!(":hash={mb}"));
    format!("idqrot:t{ms}:w=texel1{hash}")
}

#[derive(Deserialize)]
struct Position {
    name: String,
    board: String,
    side: String,
}

/// One search of `spec` on `position` of `phase`: its row.
fn search(spec: &str, position: &Position, phase: &str) -> Value {
    let Ok(AgentSpec::Deepening {
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
    }) = spec.parse::<AgentSpec>()
    else {
        panic!("{spec}: expected an iterative-deepening agent");
    };
    let board: Board = position.board.parse().expect("bad board");
    let side = if position.side == "white" {
        Color::White
    } else {
        Color::Black
    };
    let mut table = table.map(Table::new);
    let options = SearchOptions {
        quiescence,
        order,
        draws,
        pvs,
        extend,
        ..SearchOptions::default()
    };
    let history = repetition.then_some(&[][..]);
    let start = Instant::now();
    let id = iterative_deepening_with(
        &board,
        side,
        weights,
        limits,
        options,
        history,
        table.as_mut(),
    );
    let ms = start.elapsed().as_secs_f64() * 1e3;
    let nodes = id.decision.nodes;
    let count = |kind| board.count(Color::White, kind) + board.count(Color::Black, kind);
    let row = json!({
        "name": position.name,
        "phase": phase,
        "pieces": count(Kind::Man) + count(Kind::King),
        "kings": count(Kind::King),
        "nodes": nodes,
        "depth": id.depth,
        "ms": (ms * 10.0).round() / 10.0,
        "nps": (nodes as f64 / ms * 1e3).round() as u64,
        "depth_limit_hit": id.depth >= MAX_ID_DEPTH,
    });
    eprintln!("{spec}\t{}", row);
    row
}

/// The median of `values` (the mean of the two middle ones for an even count).
fn median(mut values: Vec<f64>) -> f64 {
    values.sort_by(f64::total_cmp);
    let n = values.len();
    if n % 2 == 1 {
        values[n / 2]
    } else {
        (values[n / 2 - 1] + values[n / 2]) / 2.0
    }
}

/// The text of `program args`, trimmed, or `None` if it cannot be run.
fn output(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The CPU model, from /proc/cpuinfo.
fn machine() -> String {
    let info = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    (info.lines())
        .find_map(|l| {
            l.strip_prefix("model name")
                .map(|r| r.trim_start_matches([' ', '\t', ':']))
        })
        .unwrap_or("unknown")
        .to_string()
}

/// The command that ran this, with the `taskset` it was pinned with if it runs on one CPU.
fn command() -> String {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let cpus = (status.lines()).find_map(|l| l.strip_prefix("Cpus_allowed_list:").map(str::trim));
    let args: Vec<String> = std::env::args().collect();
    match cpus {
        Some(cpu) if cpu.parse::<u32>().is_ok() => format!("taskset -c {cpu} {}", args.join(" ")),
        _ => args.join(" "),
    }
}

fn main() {
    let (mut seconds, mut out) = (60u64, "results/minute/minute.json".to_string());
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seconds" => {
                seconds = args
                    .next()
                    .and_then(|s| s.parse().ok())
                    .expect("--seconds S")
            }
            "--out" => out = args.next().expect("--out PATH"),
            _ => panic!("unknown argument {arg}: expected --seconds S or --out PATH"),
        }
    }
    let ms = Duration::from_secs(seconds).as_millis();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../testdata");
    let text = std::fs::read_to_string(dir.join("decisions.json")).expect("cannot read testdata");
    let all: Vec<Position> = serde_json::from_str(&text).expect("bad testdata");
    let find = |name: &str| (all.iter().find(|p| p.name == name)).expect("no such position");
    let phase = |name: &str| POSITIONS.iter().find(|(n, _)| *n == name).unwrap().1;

    let version = output("git", &["describe", "--always", "--dirty"]).unwrap_or_default();
    if version.ends_with("-dirty") {
        eprintln!("warning: the working tree is dirty ({version})");
    }
    let spec = engine(ms, None);
    let rows: Vec<Value> = (POSITIONS.iter())
        .map(|(name, phase)| search(&spec, find(name), phase))
        .collect();
    let large = engine(ms, Some(256));
    let hash256: Vec<Value> = (HASH256.iter())
        .map(|name| search(&large, find(name), phase(name)))
        .collect();

    let column = |key: &str| rows.iter().map(|r| r[key].as_f64().unwrap()).collect();
    let report = json!({
        "engine": spec,
        "engine_hash256": large,
        "positions": rows,
        "hash256": hash256,
        "summary": {
            "median_nodes": median(column("nodes")).round() as u64,
            "median_depth": median(column("depth")),
            "median_nps": median(column("nps")).round() as u64,
            "hash_mb": TableOptions::DEFAULT_MB,
        },
        "machine": machine(),
        "version": version,
        "command": command(),
    });
    let path = Path::new(&out);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("cannot make the directory");
    }
    let text = serde_json::to_string_pretty(&report).unwrap() + "\n";
    std::fs::write(path, text).expect("cannot write the result");
    println!(
        "{}",
        serde_json::to_string_pretty(&report["summary"]).unwrap()
    );
}
