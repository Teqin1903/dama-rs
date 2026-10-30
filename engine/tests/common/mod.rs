//! Helpers shared by the integration tests. Every file in tests/ is compiled as its own crate,
//! so shared code lives in this subdirectory module and each test file declares `mod common;`.

use std::path::Path;

use engine::{Board, Color, Weights};
use serde::Deserialize;

/// The contents of `testdata/<file>`.
pub fn read_testdata(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../testdata")
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The side to move as the testdata files write it: `"white"` or `"black"`.
pub fn side(name: &str) -> Color {
    match name {
        "white" => Color::White,
        "black" => Color::Black,
        other => panic!("unknown side {other:?}"),
    }
}

/// A position of decisions.json or movegen.json. Only decisions.json has `name` and `weights`.
#[derive(Deserialize)]
struct Position {
    name: Option<String>,
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

/// Every position of both files, with a label for messages, the side to move and its weights.
#[allow(dead_code)]
pub fn positions() -> Vec<(String, Board, Color, Weights)> {
    let mut all = Vec::new();
    for file in ["decisions.json", "movegen.json"] {
        let entries: Vec<Position> = serde_json::from_str(&read_testdata(file)).unwrap();
        for (i, p) in entries.into_iter().enumerate() {
            let weights = p.weights.map_or(Weights::TUNED, |w| Weights {
                man: w.man,
                king: w.king,
                mob: w.mob,
                ..Weights::ZERO
            });
            let label = format!("{file} #{i} {}", p.name.unwrap_or_default());
            all.push((label, p.board.parse().unwrap(), side(&p.side), weights));
        }
    }
    assert_eq!(all.len(), 541);
    all
}
