//! Plays a match again (paired, seed 1, as `dama ladder run`) and tells how its draws ended: per
//! reason, how often A had more pieces, as many or fewer on the last board, counting a king as a
//! man and as `Weights::TEXEL1.king` men. Checks every game against the stored games file first.
//!
//!     cargo build --release --example endings
//!     target/release/examples/endings results/mcts/games/UCTe_vs_V0.csv 6
//!
//! The arguments are a games file and the threads.

use std::collections::BTreeMap;

use engine::{
    AgentSpec, Board, Color, EndReason, GameRow, Kind, MatchPlan, SEED, Schedule, Weights,
    play_match,
};

/// A's material less B's: men and kings, and with a king worth `king` men.
fn material(board: &Board, a: Color, king: f64) -> (i32, i32, f64) {
    let side = |c: Color| (board.count(c, Kind::Man), board.count(c, Kind::King));
    let ((am, ak), (bm, bk)) = (side(a), side(a.opponent()));
    let pieces = (am + ak) as i32 - (bm + bk) as i32;
    let kings = ak as i32 - bk as i32;
    let weighted = f64::from(am) - f64::from(bm) + king * (f64::from(ak) - f64::from(bk));
    (pieces, kings, weighted)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = &args[0];
    let threads: usize = args.get(1).map_or(6, |t| t.parse().expect("threads"));
    let text = std::fs::read_to_string(path).expect("cannot read the games file");
    let stored: Vec<GameRow> = (text.lines().skip(1))
        .map(|line| line.parse().expect("bad row"))
        .collect();
    let (a, b) = (stored[0].white.clone(), stored[0].black.clone());
    let (sa, sb): (AgentSpec, AgentSpec) = (a.parse().unwrap(), b.parse().unwrap());
    let plan = MatchPlan {
        games: stored.len() as u32,
        seed: SEED,
        schedule: Schedule::Paired,
        threads,
        sprt: None,
    };
    let record = play_match(&|| sa.build(), &|| sb.build(), &plan).unwrap();
    // reason -> [A ahead, even, A behind], the sum of the weighted differences, and A ahead by
    // at least two men's worth.
    let mut by: BTreeMap<String, ([u32; 3], f64, u32)> = BTreeMap::new();
    for (game, row) in record.games.iter().zip(&stored) {
        let r = &game.record;
        assert_eq!(
            (r.winner, r.plies() as u32, r.reason),
            (row.winner, row.plies, row.reason),
            "game {} differs from {path}",
            row.game
        );
        if r.winner.is_some() {
            continue;
        }
        let mut board = Board::start();
        for mv in &r.moves {
            board = board.apply(mv);
        }
        let (pieces, _, weighted) = material(&board, game.a_color(), Weights::TEXEL1.king);
        let reason = match r.reason {
            EndReason::Repetition => "repetition",
            EndReason::QuietPlies => "fifty",
            EndReason::OnePieceEach => "one_each",
            EndReason::Cap => "ply_cap",
            EndReason::NoMove => unreachable!("a draw"),
        };
        let entry = by.entry(reason.to_string()).or_default();
        entry.0[match pieces {
            p if p > 0 => 0,
            0 => 1,
            _ => 2,
        }] += 1;
        entry.1 += weighted;
        entry.2 += u32::from(weighted >= 2.0);
    }
    println!("{a} against {b}: {} games, every one as stored", stored.len());
    println!("draw by\tdraws\tA more pieces\teven\tA fewer\tA ahead by >= 2 men (king = 2.51)\tmean difference (men)");
    for (reason, (counts, sum, two)) in by {
        let n = counts.iter().sum::<u32>();
        println!(
            "{reason}\t{n}\t{}\t{}\t{}\t{two}\t{:+.2}",
            counts[0],
            counts[1],
            counts[2],
            sum / f64::from(n)
        );
    }
}
