//! Referee 2 of CLAUDE.md: for every position in testdata/rules.json and testdata/movegen.json,
//! our legal move list must equal the expected list exactly.

mod common;

use engine::Board;
use serde::Deserialize;

/// One position. `name` and `rule` exist only in rules.json, `kind` only in movegen.json.
#[derive(Deserialize)]
struct Case {
    name: Option<String>,
    rule: Option<String>,
    kind: Option<String>,
    board: String,
    side: String,
    moves: Vec<String>,
}

/// Runs every case of `testdata/<file>` and reports all failing cases together.
fn check(file: &str) {
    let text = common::read_testdata(file);
    let cases: Vec<Case> = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{file}: {e}"));
    assert!(!cases.is_empty(), "{file} has no cases");

    let mut failures = Vec::new();
    for (i, case) in cases.iter().enumerate() {
        let board: Board = case
            .board
            .parse()
            .unwrap_or_else(|e| panic!("{file} #{i}: {e}"));
        assert_eq!(
            board.to_string(),
            case.board,
            "{file} #{i}: board string round trip"
        );

        let mut got: Vec<String> = board
            .legal_moves(common::side(&case.side))
            .iter()
            .map(|m| m.to_string())
            .collect();
        got.sort();
        if got != case.moves {
            let missing: Vec<&String> = case.moves.iter().filter(|m| !got.contains(m)).collect();
            let extra: Vec<&String> = got.iter().filter(|m| !case.moves.contains(m)).collect();
            let label = case.name.as_deref().or(case.kind.as_deref()).unwrap_or("?");
            let rule = case
                .rule
                .as_deref()
                .map(|r| format!("\n  rule: {r}"))
                .unwrap_or_default();
            failures.push(format!(
                "{file} #{i} {label}{rule}\n  board: {} ({} to move)\n  \
                 expected {} moves, got {}\n  missing: {missing:?}\n  extra:   {extra:?}",
                case.board,
                case.side,
                case.moves.len(),
                got.len(),
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

#[test]
fn rules_json() {
    check("rules.json");
}

#[test]
fn movegen_json() {
    check("movegen.json");
}
