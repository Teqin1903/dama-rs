//! Plays a scene of the site (site/data/scenes.js) on to the end of the game, in a `Session`, as
//! the site does: the scene's moves are played, then White and Black get their agents (each having
//! observed the game so far) and play until the arena's rules end the game.
//!
//! cargo run --release --example scene -- fifty WHITE BLACK [WHITE_SEED BLACK_SEED | gameN]
//!     [--from PLY]
//!
//! The seeds default to the site's, 1000 and 1001; `gameN` takes those of game N of a paired match
//! with seed 1 (`paired_agent_seeds`; agent A is White in the even games). With `--from PLY` the agents are seated at that
//! ply instead (0 to the scene's length) and think at every ply up to the scene's end, where the
//! scene's move is still played; it reports how many of those moves they would have played. With
//! the arena's seeds of a game and `--from 4` (after the random opening plies) the agents replay
//! that game: the check that a scene continues as the stored game did.

use engine::{Json, Session, native_clock, paired_agent_seeds};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (from, args): (Option<usize>, Vec<&String>) = match args.iter().position(|a| a == "--from")
    {
        Some(i) => (
            Some(args[i + 1].parse().expect("--from takes a ply")),
            args.iter()
                .enumerate()
                .filter(|&(j, _)| j != i && j != i + 1)
                .map(|(_, a)| a)
                .collect(),
        ),
        None => (None, args.iter().collect()),
    };
    let [id, white, black, rest @ ..] = &args[..] else {
        panic!("usage: scene ID WHITE BLACK [WHITE_SEED BLACK_SEED | gameN] [--from PLY]");
    };
    let seeds: (u32, u32) = match rest {
        [] => (1000, 1001),
        [game] => {
            let game: u32 = (game.strip_prefix("game").and_then(|n| n.parse().ok()))
                .expect("gameN with N a number");
            let (a, b) = paired_agent_seeds(1, game);
            if game.is_multiple_of(2) {
                (a, b)
            } else {
                (b, a)
            }
        }
        [w, b] => (w.parse().unwrap(), b.parse().unwrap()),
        _ => panic!("two seeds, gameN or nothing"),
    };
    let moves = scene_moves(id);
    let from = from.unwrap_or(moves.len());
    assert!(
        from <= moves.len(),
        "--from past the scene's {} plies",
        moves.len()
    );

    let mut session = Session::new(native_clock);
    let seat = |session: &mut Session| {
        expect_ok(session.set_agent("white", white, seeds.0));
        expect_ok(session.set_agent("black", black, seeds.1));
    };
    let mut agreed = 0;
    for (ply, mv) in moves.iter().enumerate() {
        if ply == from {
            seat(&mut session);
        }
        if ply >= from {
            let side = if ply % 2 == 0 { "white" } else { "black" };
            let report = expect_ok(session.think(side, "{}"));
            agreed += usize::from(report.get("best").and_then(Json::as_str) == Some(mv));
        }
        expect_ok(session.play(mv));
    }
    if from == moves.len() {
        seat(&mut session);
    } else {
        println!(
            "plies {from} to {}: the agents chose the scene's move {agreed} times of {}",
            moves.len(),
            moves.len() - from
        );
    }
    let start = moves.len();
    let mut line = Vec::new();
    let state = loop {
        let state = session.state();
        if state.get("status").and_then(Json::as_str) != Some("playing") {
            break state;
        }
        let side = state
            .get("turn")
            .and_then(Json::as_str)
            .unwrap()
            .to_string();
        let report = expect_ok(session.think(&side, "{}"));
        let best = report
            .get("best")
            .and_then(Json::as_str)
            .unwrap()
            .to_string();
        let value = report
            .get("value")
            .and_then(Json::as_f64)
            .unwrap_or(f64::NAN);
        line.push(format!("{best} ({value:+.2})"));
        expect_ok(session.play(&best));
    };
    let get = |key: &str| state.get(key).map_or("-".to_string(), |v| v.to_string());
    println!(
        "{id}: White {white}, Black {black}, seeds {} {}: from ply {start}, {} at ply {}, reason {}, pieces {}",
        seeds.0,
        seeds.1,
        get("status"),
        get("ply"),
        get("reason"),
        get("pieces"),
    );
    for (i, chunk) in line.chunks(8).enumerate() {
        println!("  {:>3}: {}", start + 8 * i, chunk.join("  "));
    }
}

/// The moves of the scene `id` in site/data/scenes.js.
fn scene_moves(id: &str) -> Vec<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../site/data/scenes.js");
    let text = std::fs::read_to_string(path).expect("site/data/scenes.js");
    let json = &text[text.find('[').unwrap()..=text.rfind(']').unwrap()];
    let scenes: serde_json::Value = serde_json::from_str(json).expect("scenes are JSON");
    let scene = (scenes.as_array().unwrap().iter())
        .find(|s| s["id"] == id)
        .unwrap_or_else(|| panic!("no scene {id}"));
    (scene["moves"].as_array().unwrap().iter())
        .map(|m| m.as_str().unwrap().to_string())
        .collect()
}

/// The answer of a call, or a panic with its error.
fn expect_ok(answer: Json) -> Json {
    if let Some(error) = answer.get("error") {
        panic!("{error}");
    }
    answer
}
