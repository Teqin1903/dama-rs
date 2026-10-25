//! Referee 3 of CLAUDE.md: for every position in testdata/decisions.json, with its weights, our
//! features (men, kings, mob, win) match for every move, and the agent's move is one of `best`.

mod common;

use std::collections::BTreeMap;

use engine::{Board, Candidate, Mulberry32, U_EPSILON, Weights, decide};
use serde::Deserialize;

/// One decision of the baseline agent, as make_testdata.js recorded it.
#[derive(Deserialize)]
struct Entry {
    name: String,
    board: String,
    side: String,
    weights: JsWeights,
    candidates: Vec<JsCandidate>,
    /// Every move with the highest U, sorted.
    best: Vec<String>,
}

#[derive(Deserialize)]
struct JsWeights {
    man: f64,
    king: f64,
    mob: f64,
}

#[derive(Deserialize)]
struct JsCandidate {
    #[serde(rename = "move")]
    notation: String,
    men: i32,
    kings: i32,
    mob: i32,
    win: bool,
    u: f64,
}

fn entries() -> Vec<Entry> {
    let entries: Vec<Entry> =
        serde_json::from_str(&common::read_testdata("decisions.json")).unwrap();
    assert!(!entries.is_empty(), "decisions.json has no entries");
    entries
}

/// How our decision differs from the recorded one; empty if they agree.
fn differences(entry: &Entry) -> Vec<String> {
    let board: Board = entry.board.parse().unwrap();
    let w = &entry.weights;
    let weights = Weights {
        man: w.man,
        king: w.king,
        mob: w.mob,
    };
    let decision = decide(&board, common::side(&entry.side), weights);
    let mut found = Vec::new();

    // 1. The same moves, each with the same features and U.
    let ours: BTreeMap<String, &Candidate> = decision
        .candidates
        .iter()
        .map(|c| (c.mv.to_string(), c))
        .collect();
    for expected in &entry.candidates {
        let Some(c) = ours.get(&expected.notation) else {
            found.push(format!("missing move {}", expected.notation));
            continue;
        };
        let f = c.features;
        let same_features = (f.men, f.kings, f.mob, f.win)
            == (expected.men, expected.kings, expected.mob, expected.win);
        if !same_features || (c.u - expected.u).abs() > U_EPSILON {
            found.push(format!(
                "{}: ours {f:?} u {}, expected men {} kings {} mob {} win {} u {}",
                expected.notation,
                c.u,
                expected.men,
                expected.kings,
                expected.mob,
                expected.win,
                expected.u
            ));
        }
    }
    for notation in ours.keys() {
        if !entry.candidates.iter().any(|e| &e.notation == notation) {
            found.push(format!("extra move {notation}"));
        }
    }

    // 2. Our moves with the highest U are exactly `best`.
    let mut top: Vec<String> = decision
        .top
        .iter()
        .map(|&i| decision.candidates[i].mv.to_string())
        .collect();
    top.sort();
    if top != entry.best {
        found.push(format!("top {top:?}, expected best {:?}", entry.best));
    }

    // 3. The move the agent plays is in `best`, with or without random tie-breaking.
    let mut rng = Mulberry32::new(7070);
    let mut picks = vec![decision.choose(None)];
    for _ in 0..8 {
        picks.push(decision.choose(Some(&mut rng)));
    }
    for pick in picks {
        match pick {
            Some(mv) if entry.best.contains(&mv.to_string()) => {}
            other => found.push(format!(
                "plays {:?}, which is not in best",
                other.map(|mv| mv.to_string())
            )),
        }
    }
    found
}

#[test]
fn decisions_json() {
    let entries = entries();
    let failures: Vec<String> = entries
        .iter()
        .filter_map(|entry| {
            let found = differences(entry);
            (!found.is_empty()).then(|| {
                format!(
                    "{} ({} to move, board {}):\n  {}",
                    entry.name,
                    entry.side,
                    entry.board,
                    found.join("\n  ")
                )
            })
        })
        .collect();
    assert!(
        failures.is_empty(),
        "{} of {} decisions differ:\n{}",
        failures.len(),
        entries.len(),
        failures.join("\n")
    );
}

/// The position of slide 7 in our talk: the agent plays g2-h2 with U 7.95.
#[test]
fn talk_slide7() {
    let entry = entries()
        .into_iter()
        .find(|e| e.name == "talk_slide7_game0_ply36")
        .unwrap();
    let w = &entry.weights;
    assert_eq!(
        Weights {
            man: w.man,
            king: w.king,
            mob: w.mob
        },
        Weights::TUNED
    );
    let board: Board = entry.board.parse().unwrap();
    let decision = decide(&board, common::side(&entry.side), Weights::TUNED);
    assert_eq!(decision.choose(None).unwrap().to_string(), "g2-h2");
    // Exactly 7.95: the same operations in the same order as agent.js give the same double.
    assert_eq!(decision.candidates[decision.top[0]].u, 7.95);
}
