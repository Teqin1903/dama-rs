//! How often the single-reply extension (the letter `x`) applies, and where it changes the decision.
//!
//!     target/release/dama arena idqrotfx:n20000:w=texel1:fs=1 idqrotf:n20000:w=texel1:fs=1 \
//!         --games 200 --seed 1 --threads 6 --log games.tsv
//!     target/release/examples/forced games.tsv
//!
//! First, on the 541 fixed positions of testdata and on the positions of the games in the log after
//! their 4 random opening plies: how many positions have a move, how many a single legal move, and of
//! what kind: a capture (the part `x` extends), or a quiet move, which it leaves out: a king's move,
//! a man's step sideways, a man's step forward.
//!
//! Then the positions of those games with more than one move where `ab4qox:w=texel1` and
//! `ab4qo:w=texel1` have no best move in common, on an empty game: their number and those whose
//! principal variation with `x` holds most forced captures in the fewest plies, each with the values
//! of both moves in both searches (searched alone, so exact), and both lines. Extended moves in a line
//! are marked with `!`.
//!
//! Output: tab-separated tables; the game is the log's first column, the ply counts from 0.

use std::path::Path;

use engine::{
    Board, Color, Decision, Kind, Move, Recorder, SearchOptions, Weights, alpha_beta_traced,
    alpha_beta_with, native_clock,
};
use serde::Deserialize;

const DEPTH: u32 = 4;
const OPENING_PLIES: usize = 4;

#[derive(Deserialize)]
struct Position {
    board: String,
    side: String,
}

fn fixed_positions() -> Vec<(Board, Color)> {
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

/// A position of a game in the log.
struct GameAt {
    game: usize,
    ply: usize,
    board: Board,
    side: Color,
}

fn game_positions(log: &str) -> Vec<GameAt> {
    let text = std::fs::read_to_string(log).expect("cannot read the log");
    let mut all = Vec::new();
    for row in text.lines().skip(1) {
        let columns: Vec<&str> = row.split('\t').collect();
        let game = columns[0].parse().expect("bad game number");
        let (mut board, mut side) = (Board::start(), Color::White);
        for (ply, notation) in columns[6].split(' ').enumerate() {
            if ply >= OPENING_PLIES {
                all.push(GameAt {
                    game,
                    ply,
                    board,
                    side,
                });
            }
            let legal = board.legal_moves(side);
            let mv = (legal.iter())
                .find(|m| m.to_string() == notation)
                .unwrap_or_else(|| panic!("game {game}, ply {ply}: {notation} is not legal"));
            board = board.apply(mv);
            side = side.opponent();
        }
    }
    all
}

/// Positions with a move, with one move, captures, kings' moves, steps sideways, steps forward.
fn census(positions: impl Iterator<Item = (Board, Color)>) -> [u64; 6] {
    let mut counts = [0; 6];
    for (board, side) in positions {
        let moves = board.legal_moves(side);
        if moves.is_empty() {
            continue;
        }
        counts[0] += 1;
        let [mv] = &moves[..] else {
            continue;
        };
        counts[1] += 1;
        let piece = board.piece_at(mv.from()).expect("a piece moves");
        counts[match () {
            _ if mv.is_capture() => 2,
            _ if piece.kind == Kind::King => 3,
            _ if mv.from().rank() == mv.to().rank() => 4,
            _ => 5,
        }] += 1;
    }
    counts
}

fn print_census(name: &str, c: [u64; 6]) {
    let share = |n: u64, of: u64| 100.0 * n as f64 / of as f64;
    println!(
        "{name}\t{}\t{}\t{:.2}\t{}\t{:.1}\t{}\t{}\t{}\t{:.2}",
        c[0],
        c[1],
        share(c[1], c[0]),
        c[2],
        share(c[2], c[1]),
        c[3],
        c[4],
        c[5],
        share(c[2], c[0])
    );
}

/// The value of move `index` alone and its line, `!` after an extended move, and how many there are.
fn line(
    board: &Board,
    side: Color,
    index: usize,
    options: SearchOptions,
) -> (f64, String, usize, usize) {
    let mut recorder = Recorder::new(native_clock, None);
    alpha_beta_traced(
        board,
        side,
        Weights::TEXEL1,
        DEPTH,
        options,
        None,
        None,
        Some(&[index]),
        &mut recorder,
    );
    let last = recorder.iterations.last().expect("an iteration");
    let root = &last.root[0];
    let (mut at, mut to_move, mut left) = (*board, side, DEPTH as i32);
    let (mut text, mut forced) = (Vec::new(), 0);
    for (ply, mv) in root.pv.iter().enumerate() {
        let single = at.legal_moves(to_move).len() == 1 && mv.is_capture();
        let extended = options.extend && ply > 0 && left >= 1 && single;
        forced += usize::from(extended);
        text.push(format!("{mv}{}", if extended { "!" } else { "" }));
        left -= i32::from(!extended);
        at = at.apply(mv);
        to_move = to_move.opponent();
    }
    (root.value, text.join(" "), forced, root.pv.len())
}

fn first_best(decision: &Decision) -> &Move {
    &decision.candidates[decision.top[0]].mv
}

fn main() {
    let log = std::env::args().nth(1).expect("usage: forced GAMES.TSV");
    let games = game_positions(&log);
    println!(
        "positions\twith_a_move\tsingle\tsingle_pct\tcapture\tcapture_pct_of_single\tking_move\tsideways\tforward\tcapture_pct_of_all"
    );
    print_census("541 fixed", census(fixed_positions().into_iter()));
    print_census("games", census(games.iter().map(|g| (g.board, g.side))));

    let plain = SearchOptions {
        quiescence: true,
        order: true,
        ..SearchOptions::default()
    };
    let extend = SearchOptions {
        extend: true,
        ..plain
    };
    let (mut searched, mut different) = (0, 0);
    let mut scenes = Vec::new();
    for g in &games {
        if g.board.legal_moves(g.side).len() < 2 {
            continue;
        }
        searched += 1;
        let run = |options| {
            alpha_beta_with(
                &g.board,
                g.side,
                Weights::TEXEL1,
                DEPTH,
                options,
                None,
                None,
            )
        };
        let (a, b) = (run(extend), run(plain));
        if a.top.iter().any(|t| b.top.contains(t)) {
            continue;
        }
        different += 1;
        let (ia, ib) = (a.top[0], b.top[0]);
        let (va, text_a, forced, plies) = line(&g.board, g.side, ia, extend);
        let (vb, text_b, _, _) = line(&g.board, g.side, ib, plain);
        let (va_plain, _, _, _) = line(&g.board, g.side, ia, plain);
        let (vb_extend, _, _, _) = line(&g.board, g.side, ib, extend);
        scenes.push((
            forced,
            plies,
            format!(
                "{}\t{}\t{:?}\t{}\t{}\t{va:.4}\t{va_plain:.4}\t{}\t{vb_extend:.4}\t{vb:.4}\t{text_a}\t{text_b}",
                g.game,
                g.ply,
                g.side,
                g.board,
                first_best(&a),
                first_best(&b),
            ),
        ));
    }
    println!("searched\tdifferent\tdifferent_pct");
    println!(
        "{searched}\t{different}\t{:.2}",
        100.0 * different as f64 / searched as f64
    );
    scenes.sort_by_key(|s| (std::cmp::Reverse(s.0), s.1));
    println!(
        "forced\tplies\tgame\tply\tside\tboard\tmove_x\tits_value_x\tits_value\tmove\tits_value_x\tits_value\tline_x\tline"
    );
    for (forced, plies, row) in scenes.iter().take(25) {
        println!("{forced}\t{plies}\t{row}");
    }
}
