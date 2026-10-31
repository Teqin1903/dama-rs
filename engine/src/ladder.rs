//! The Elo ladder of our tagged versions (docs/ladder.md): which agent each tag stands for, which
//! matches they play, one CSV row per game, and the ratings and summaries computed from the rows.
//!
//! Every version plays the one before it (`previous`) and the one two steps back (`two_back`) in the
//! chain, every version above the anchor plays the anchor (`anchor`), every version above the
//! mid-ladder opponent plays it (`mid`), the anchor plays itself once (`self`, the noise of a match
//! between equals) and a side rung plays the rung beside it (`side`). A pair that comes up in
//! several lists is played once, with the most games any list asks for, and keeps all its roles.

use std::fmt;
use std::str::FromStr;

use crate::arena::{EndReason, MatchRecord, Thinking, paired_opening_seed};
use crate::board::Color;
use crate::elo::{self, Bootstrap, Fit, PairedMatch, Score};
use crate::json::Json;

/// One version of the agent on the ladder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rung {
    /// `V4`, `V8b`: the number of the git tag it comes from.
    pub id: &'static str,
    pub tag: &'static str,
    /// The agent as `dama arena` names it.
    pub spec: &'static str,
    pub label: &'static str,
    /// What this version adds to the one before it.
    pub adds: &'static str,
    /// For a side rung, the id of the rung it stands beside; side rungs are not in the chain.
    pub beside: Option<&'static str>,
    /// For a rung of the ablation, the part of the full agent it lacks.
    pub removed: Option<&'static str>,
}

const fn rung(
    id: &'static str,
    tag: &'static str,
    spec: &'static str,
    label: &'static str,
    adds: &'static str,
) -> Rung {
    Rung {
        id,
        tag,
        spec,
        label,
        adds,
        beside: None,
        removed: None,
    }
}

/// The ladder, bottom to top. The tags v5-arena and v7-sprt-eval-params changed the measurements,
/// not the agent, so they have no rung (`FOOTNOTES`).
pub const LADDER: &[Rung] = &[
    rung(
        "V0",
        "baseline",
        "tuned",
        "V0_Baseline",
        "one ply, U = 1/2.5/-0.05",
    ),
    rung(
        "V1",
        "v1-minimax",
        "mm3",
        "V1_Minimax",
        "minimax 3 plies deep",
    ),
    rung(
        "V2",
        "v2-alpha-beta",
        "ab5",
        "V2_AlphaBeta",
        "alpha-beta, so 5 plies deep",
    ),
    rung(
        "V3",
        "v3-iterative-deepening",
        "id:n20000",
        "V3_IterativeDeepening",
        "iterative deepening within 20000 nodes per move",
    ),
    rung(
        "V4",
        "v4-quiescence",
        "idq:n20000",
        "V4_Quiescence",
        "captures searched past the depth limit",
    ),
    rung(
        "V6",
        "v6-repetition",
        "idqr:n20000",
        "V6_Repetition",
        "a repeated position is a draw in the search",
    ),
    rung(
        "V8",
        "v8-texel1",
        "idqr:n20000:w=texel1",
        "V8_Texel1",
        "Texel-tuned weights with positional features",
    ),
    Rung {
        beside: Some("V8"),
        ..rung(
            "V8b",
            "exp-texel2",
            "idqr:n20000:w=texel2",
            "V8b_Texel2",
            "second Texel tuning: lower error, weaker play",
        )
    },
    rung(
        "V9",
        "v9-move-ordering",
        "idqro:n20000:w=texel1",
        "V9_MoveOrdering",
        "move ordering: previous best move, promotions, killers, history",
    ),
    rung(
        "V10",
        "v10-transposition-table",
        "idqrot:n20000:w=texel1",
        "V10_TranspositionTable",
        "transposition table (16 MB, Zobrist keys)",
    ),
];

/// The tags without a rung, and why.
pub const FOOTNOTES: &[(&str, &str, &str)] = &[
    (
        "V5",
        "v5-arena",
        "measurement infrastructure (paired openings, parallel games); the agent did not change",
    ),
    (
        "V7",
        "v7-sprt-eval-params",
        "measurement infrastructure (SPRT, parametrised U with every new weight 0); the agent did \
         not change",
    ),
];

/// The ablation: the top rung against itself with one part turned off, and against the A3 agent.
/// V9 is V10 without its table, so V10–V9 is the ladder's pairing, not played again.
pub const ABLATION: &[Rung] = &[
    rung(
        "V10",
        "v10-transposition-table",
        "idqrot:n20000:w=texel1",
        "V10_TranspositionTable",
        "the full agent",
    ),
    ablated(
        "V10-q",
        "idrot:n20000:w=texel1",
        "V10_NoQuiescence",
        "quiescence",
    ),
    ablated(
        "V10-r",
        "idqot:n20000:w=texel1",
        "V10_NoRepetition",
        "repetitions in the search",
    ),
    ablated(
        "V10-o",
        "idqrt:n20000:w=texel1",
        "V10_NoOrdering",
        "move ordering (the table's move is still searched first)",
    ),
    Rung {
        removed: Some("transposition table"),
        ..rung(
            "V9",
            "v9-move-ordering",
            "idqro:n20000:w=texel1",
            "V9_MoveOrdering",
            "move ordering: previous best move, promotions, killers, history",
        )
    },
    ablated(
        "V10-w",
        "idqrot:n20000:w=tuned",
        "V10_TunedWeights",
        "Texel weights (the tuned U instead)",
    ),
    Rung {
        removed: Some("everything since A3: the one-ply baseline agent"),
        ..rung(
            "V0",
            "baseline",
            "tuned",
            "V0_Baseline",
            "one ply, U = 1/2.5/-0.05",
        )
    },
];

/// A rung of the ablation that is V10 with `removed` turned off.
const fn ablated(
    id: &'static str,
    spec: &'static str,
    label: &'static str,
    removed: &'static str,
) -> Rung {
    Rung {
        removed: Some(removed),
        ..rung(id, "v10-transposition-table", spec, label, "")
    }
}

/// Notes on the ablation's rungs.
pub const ABLATION_FOOTNOTES: &[(&str, &str, &str)] = &[
    (
        "V10-o",
        "v10-transposition-table",
        "without `o` the search still tries the table's move first at every node; only the \
         previous iteration's best move at the root, promotions, killers and history are off",
    ),
    (
        "V9",
        "v9-move-ordering",
        "V10 without its table; the games of V10_vs_V9 are the ladder's (results/ladder/games)",
    ),
];

/// Monte Carlo tree search, in four forms that add one thing each (engine/src/mcts.rs), and
/// three rungs of the ladder they play against (`MCTS_OPPONENTS`).
pub const MCTS: &[Rung] = &[
    LADDER[0],
    LADDER[2],
    LADDER[9],
    rung(
        "MC",
        "mcts-1",
        "mc:n20000",
        "MC_Flat",
        "flat Monte Carlo: random games after each root move",
    ),
    rung(
        "UCT",
        "mcts-1",
        "uct:n20000",
        "UCT_RandomGames",
        "UCT, a random game from each new position",
    ),
    rung(
        "UCTe",
        "mcts-1",
        "ucte:n20000:w=texel1",
        "UCT_Eval",
        "UCT, each new position scored with sigmoid(K U), texel1",
    ),
    rung(
        "UCTq",
        "mcts-1",
        "uctq:n20000:w=texel1",
        "UCT_Quiescence",
        "UCT, each new position scored with sigmoid(K quiescence), texel1",
    ),
];

/// The rungs of the ladder every form of `MCTS` plays.
pub const MCTS_OPPONENTS: &[&str] = &["V0", "V2", "V10"];

/// The reference opponent, whose Elo is 0.
pub const ANCHOR: &str = "V1";
/// The mid-ladder opponent of the upper rungs.
pub const MID: &str = "V6";
/// Games of a previous or side pairing, and of the others.
pub const LONG_MATCH: u32 = 400;
pub const SHORT_MATCH: u32 = 200;
/// The seed of every ladder match.
pub const SEED: u32 = 1;

/// Why a pairing is played.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Previous,
    Anchor,
    TwoBack,
    Mid,
    SelfPlay,
    Side,
    /// The full agent against a version with one part turned off.
    Ablation,
    /// Another kind of agent against a rung of the ladder.
    Against,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Role::Previous => "previous",
            Role::Anchor => "anchor",
            Role::TwoBack => "two_back",
            Role::Mid => "mid",
            Role::SelfPlay => "self",
            Role::Side => "side",
            Role::Ablation => "ablation",
            Role::Against => "against",
        })
    }
}

/// A match of the ladder: rung `a` (the newer one) against rung `b`, indices into the rungs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pairing {
    pub a: usize,
    pub b: usize,
    pub roles: Vec<Role>,
    pub games: u32,
    /// Its games are those of the ladder's pairing of the same name: it is not played again.
    pub reused: bool,
}

impl Pairing {
    /// `V2_vs_V1`: the name of its file of games.
    pub fn name(&self, rungs: &[Rung]) -> String {
        format!("{}_vs_{}", rungs[self.a].id, rungs[self.b].id)
    }
}

/// The pairings of `rungs`, in the order they are played: previous and side, then anchor and
/// self, then two_back, then mid.
///
/// # Panics
/// If `anchor`, `mid` or the rung a side rung stands beside is not one of `rungs`.
pub fn plan(rungs: &[Rung], anchor: &str, mid: &str) -> Vec<Pairing> {
    let index = |id: &str| {
        rungs
            .iter()
            .position(|r| r.id == id)
            .unwrap_or_else(|| panic!("no rung {id}"))
    };
    let (anchor, mid) = (index(anchor), index(mid));
    let chain: Vec<usize> = (0..rungs.len())
        .filter(|&i| rungs[i].beside.is_none())
        .collect();
    let mut wanted: Vec<(usize, usize, Role, u32)> = Vec::new();
    for k in 1..chain.len() {
        wanted.push((chain[k], chain[k - 1], Role::Previous, LONG_MATCH));
    }
    for (i, r) in rungs.iter().enumerate() {
        if let Some(beside) = r.beside {
            wanted.push((i, index(beside), Role::Side, LONG_MATCH));
        }
    }
    for i in anchor + 1..rungs.len() {
        wanted.push((i, anchor, Role::Anchor, SHORT_MATCH));
    }
    wanted.push((anchor, anchor, Role::SelfPlay, SHORT_MATCH));
    for k in 2..chain.len() {
        wanted.push((chain[k], chain[k - 2], Role::TwoBack, SHORT_MATCH));
    }
    for i in mid + 1..rungs.len() {
        wanted.push((i, mid, Role::Mid, SHORT_MATCH));
    }
    let mut pairings: Vec<Pairing> = Vec::new();
    for (a, b, role, games) in wanted {
        match pairings.iter_mut().find(|p| p.a == a && p.b == b) {
            Some(p) => {
                p.roles.push(role);
                p.games = p.games.max(games);
            }
            None => pairings.push(Pairing {
                a,
                b,
                roles: vec![role],
                games,
                reused: false,
            }),
        }
    }
    pairings
}

/// The pairings of the ablation: every rung against `zero` (the full agent, as `a`), 400 games
/// each, in the order of `rungs`. A pairing whose agents and games the ladder has already played
/// is `reused`.
///
/// # Panics
/// If `zero` is not one of `rungs`.
pub fn ablation_plan(rungs: &[Rung], zero: &str) -> Vec<Pairing> {
    let a = rungs
        .iter()
        .position(|r| r.id == zero)
        .unwrap_or_else(|| panic!("no rung {zero}"));
    let ladder = plan(LADDER, ANCHOR, MID);
    (0..rungs.len())
        .filter(|&b| b != a)
        .map(|b| {
            let reused = ladder.iter().any(|p| {
                (LADDER[p.a].spec, LADDER[p.b].spec) == (rungs[a].spec, rungs[b].spec)
                    && p.games == LONG_MATCH
                    && p.name(LADDER) == format!("{}_vs_{}", rungs[a].id, rungs[b].id)
            });
            Pairing {
                a,
                b,
                roles: vec![Role::Ablation],
                games: LONG_MATCH,
                reused,
            }
        })
        .collect()
}

/// The pairings of every rung of `rungs` that is not one of `opponents` (as `a`) against each of
/// them (as `b`), `SHORT_MATCH` games each, in the order of `rungs` and then of `opponents`.
///
/// # Panics
/// If an opponent is not one of `rungs`.
pub fn against_plan(rungs: &[Rung], opponents: &[&str]) -> Vec<Pairing> {
    let index = |id: &str| {
        rungs
            .iter()
            .position(|r| r.id == id)
            .unwrap_or_else(|| panic!("no rung {id}"))
    };
    let opponents: Vec<usize> = opponents.iter().map(|id| index(id)).collect();
    (0..rungs.len())
        .filter(|a| !opponents.contains(a))
        .flat_map(|a| {
            opponents.iter().map(move |&b| Pairing {
                a,
                b,
                roles: vec![Role::Against],
                games: SHORT_MATCH,
                reused: false,
            })
        })
        .collect()
}

/// A set of rungs rated together: the ladder, the ablation or Monte Carlo tree search.
#[derive(Clone, Copy, Debug)]
pub struct Set {
    /// `ladder`, `ablation`: the name `dama ladder --set` takes.
    pub name: &'static str,
    pub rungs: &'static [Rung],
    /// The rung at Elo 0.
    pub zero: &'static str,
    pub mid: Option<&'static str>,
    /// When not empty, every other rung plays each of these (`against_plan`).
    pub opponents: &'static [&'static str],
    pub footnotes: &'static [(&'static str, &'static str, &'static str)],
    /// Where its games and its JSON file go, and the file's name.
    pub dir: &'static str,
    pub json: &'static str,
}

pub const LADDER_SET: Set = Set {
    name: "ladder",
    rungs: LADDER,
    zero: ANCHOR,
    mid: Some(MID),
    opponents: &[],
    footnotes: FOOTNOTES,
    dir: "results/ladder",
    json: "ladder.json",
};

pub const ABLATION_SET: Set = Set {
    name: "ablation",
    rungs: ABLATION,
    zero: "V10",
    mid: None,
    opponents: &[],
    footnotes: ABLATION_FOOTNOTES,
    dir: "results/ablation",
    json: "ablation.json",
};

pub const MCTS_SET: Set = Set {
    name: "mcts",
    rungs: MCTS,
    zero: "V10",
    mid: None,
    opponents: MCTS_OPPONENTS,
    footnotes: &[],
    dir: "results/mcts",
    json: "mcts.json",
};

impl Set {
    /// The sets `dama ladder --set` knows.
    pub const ALL: [Set; 3] = [LADDER_SET, ABLATION_SET, MCTS_SET];

    /// Its pairings, in the order they are played.
    pub fn plan(&self) -> Vec<Pairing> {
        match self.mid {
            _ if !self.opponents.is_empty() => against_plan(self.rungs, self.opponents),
            Some(mid) => plan(self.rungs, self.zero, mid),
            None => ablation_plan(self.rungs, self.zero),
        }
    }

    /// The index of the rung at Elo 0.
    pub fn zero_index(&self) -> usize {
        (self.rungs.iter().position(|r| r.id == self.zero)).expect("the zero is a rung")
    }
}

/// A side's thinking in one game, per move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SideStats {
    pub moves: u64,
    pub nodes_per_move: f64,
    pub depth_per_move: f64,
    pub ms_per_move: f64,
}

impl SideStats {
    fn of(t: &Thinking) -> SideStats {
        let per_move = |x: f64| {
            if t.moves == 0 {
                0.0
            } else {
                x / t.moves as f64
            }
        };
        SideStats {
            moves: t.moves,
            nodes_per_move: per_move(t.nodes as f64),
            depth_per_move: per_move(t.depths as f64),
            ms_per_move: per_move(t.time.as_secs_f64() * 1e3),
        }
    }
}

/// One game of a ladder match, as a row of its CSV file.
#[derive(Clone, Debug, PartialEq)]
pub struct GameRow {
    /// The index of the game in the match; A is White in even games.
    pub game: u32,
    /// The index of its opening: games 2p and 2p + 1 share opening p, with the colours swapped.
    pub pair: u32,
    pub opening_seed: u32,
    pub white: String,
    pub black: String,
    pub winner: Option<Color>,
    pub plies: u32,
    pub reason: EndReason,
    pub white_stats: SideStats,
    pub black_stats: SideStats,
    /// The version of the code that played it.
    pub version: String,
}

/// The header of a games file.
pub const GAMES_HEADER: &str = "game,pair,opening_seed,white,black,result,plies,reason,\
white_moves,white_nodes_per_move,white_depth_per_move,white_ms_per_move,\
black_moves,black_nodes_per_move,black_depth_per_move,black_ms_per_move,version";

/// How a reason is written in a games file and in ladder.json.
fn reason_key(reason: EndReason) -> &'static str {
    match reason {
        EndReason::Repetition => "repetition",
        EndReason::NoMove => "no_move",
        EndReason::OnePieceEach => "one_each",
        EndReason::QuietPlies => "fifty",
        EndReason::Cap => "ply_cap",
    }
}

impl GameRow {
    /// A's score in this game: A is White in even games.
    pub fn a_score(&self) -> f64 {
        let a = if self.game.is_multiple_of(2) {
            Color::White
        } else {
            Color::Black
        };
        match self.winner {
            None => 0.5,
            Some(w) if w == a => 1.0,
            Some(_) => 0.0,
        }
    }

    /// The thinking of A and of B in this game.
    fn a_b_stats(&self) -> [SideStats; 2] {
        if self.game.is_multiple_of(2) {
            [self.white_stats, self.black_stats]
        } else {
            [self.black_stats, self.white_stats]
        }
    }
}

impl fmt::Display for GameRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let result = match self.winner {
            Some(Color::White) => "1-0",
            Some(Color::Black) => "0-1",
            None => "1/2-1/2",
        };
        let side = |s: &SideStats| {
            format!(
                "{},{:.1},{:.3},{:.3}",
                s.moves, s.nodes_per_move, s.depth_per_move, s.ms_per_move
            )
        };
        write!(
            f,
            "{},{},{},{},{},{result},{},{},{},{},{}",
            self.game,
            self.pair,
            self.opening_seed,
            self.white,
            self.black,
            self.plies,
            reason_key(self.reason),
            side(&self.white_stats),
            side(&self.black_stats),
            self.version
        )
    }
}

impl FromStr for GameRow {
    type Err = String;

    fn from_str(line: &str) -> Result<GameRow, String> {
        let fields: Vec<&str> = line.split(',').collect();
        let columns = GAMES_HEADER.split(',').count();
        if fields.len() != columns {
            return Err(format!("{} fields instead of {columns}", fields.len()));
        }
        fn number<T: FromStr>(s: &str) -> Result<T, String> {
            s.parse().map_err(|_| format!("bad number {s:?}"))
        }
        let side = |k: usize| -> Result<SideStats, String> {
            Ok(SideStats {
                moves: number(fields[k])?,
                nodes_per_move: number(fields[k + 1])?,
                depth_per_move: number(fields[k + 2])?,
                ms_per_move: number(fields[k + 3])?,
            })
        };
        let winner = match fields[5] {
            "1-0" => Some(Color::White),
            "0-1" => Some(Color::Black),
            "1/2-1/2" => None,
            other => return Err(format!("bad result {other:?}")),
        };
        let reason = EndReason::ALL
            .into_iter()
            .find(|&r| reason_key(r) == fields[7])
            .ok_or(format!("bad reason {:?}", fields[7]))?;
        Ok(GameRow {
            game: number(fields[0])?,
            pair: number(fields[1])?,
            opening_seed: number(fields[2])?,
            white: fields[3].to_string(),
            black: fields[4].to_string(),
            winner,
            plies: number(fields[6])?,
            reason,
            white_stats: side(8)?,
            black_stats: side(12)?,
            version: fields[16].to_string(),
        })
    }
}

/// The rows of a paired match between agents named `a` and `b`, played with `seed` by `version`.
pub fn rows_of(record: &MatchRecord, a: &str, b: &str, seed: u32, version: &str) -> Vec<GameRow> {
    record
        .games
        .iter()
        .zip(&record.per_game)
        .enumerate()
        .map(|(g, (game, [ta, tb]))| {
            let g = g as u32;
            let (white, black, ws, bs) = if game.a_white {
                (a, b, ta, tb)
            } else {
                (b, a, tb, ta)
            };
            GameRow {
                game: g,
                pair: g / 2,
                opening_seed: paired_opening_seed(seed, g),
                white: white.to_string(),
                black: black.to_string(),
                winner: game.record.winner,
                plies: game.record.plies() as u32,
                reason: game.record.reason,
                white_stats: SideStats::of(ws),
                black_stats: SideStats::of(bs),
                version: version.to_string(),
            }
        })
        .collect()
}

/// A pairing's games, summed up.
#[derive(Clone, Debug, PartialEq)]
pub struct PairingSummary {
    pub pairing: Pairing,
    pub games: u32,
    /// From A's (the newer rung's) point of view.
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub score: f64,
    /// The Elo of the score alone, `None` at 0% or 100%.
    pub elo_diff: Option<f64>,
    pub ci: [f64; 2],
    /// Draws by repetition, 50 quiet plies, one piece each, the ply cap.
    pub draws_by: [u32; 4],
    pub no_move: u32,
    pub mean_plies: f64,
}

/// A rung's rating and its thinking over all its games.
#[derive(Clone, Debug, PartialEq)]
pub struct VersionSummary {
    pub rung: Rung,
    /// `None` for a rung without games.
    pub elo: Option<f64>,
    pub ci: [f64; 2],
    pub nodes_per_move: Option<f64>,
    pub depth_per_move: Option<f64>,
    pub ms_per_move: Option<f64>,
}

/// The ladder computed from the games.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub versions: Vec<VersionSummary>,
    pub pairings: Vec<PairingSummary>,
    pub fit: Fit,
    pub bootstrap: Bootstrap,
    pub zero: usize,
}

/// Rates `rungs` from the games of `pairings` (`rows[k]` are the games of `pairings[k]`, in game
/// order; a pairing without rows is left out), with the rung `zero` at 0 and `samples` bootstrap
/// samples from `seed`.
///
/// # Panics
/// If a pairing has an odd number of games: the bootstrap draws whole pairs.
pub fn rate(
    rungs: &[Rung],
    pairings: &[Pairing],
    rows: &[Vec<GameRow>],
    zero: usize,
    samples: usize,
    seed: u32,
) -> Report {
    let played: Vec<(&Pairing, &Vec<GameRow>)> = pairings
        .iter()
        .zip(rows)
        .filter(|(_, r)| !r.is_empty())
        .collect();
    let matches: Vec<PairedMatch> = played
        .iter()
        .map(|(p, r)| {
            assert!(r.len() % 2 == 0, "{}: odd number of games", p.name(rungs));
            PairedMatch {
                a: p.a,
                b: p.b,
                pairs: r
                    .chunks(2)
                    .map(|c| c[0].a_score() + c[1].a_score())
                    .collect(),
            }
        })
        .collect();
    let scores: Vec<Score> = matches
        .iter()
        .map(|m| Score {
            a: m.a,
            b: m.b,
            games: 2.0 * m.pairs.len() as f64,
            score: m.pairs.iter().sum(),
        })
        .collect();
    let fit = elo::fit(rungs.len(), &scores, zero);
    let bootstrap = elo::bootstrap(rungs.len(), &matches, zero, samples, seed);
    let summaries = played
        .iter()
        .zip(&bootstrap.matches)
        .map(|((p, r), ci)| summarise(p, r, *ci))
        .collect();
    // Thinking per rung: [moves, nodes, depths, ms] summed over every game it played.
    let mut thinking = vec![[0.0f64; 4]; rungs.len()];
    for (p, r) in &played {
        for row in r.iter() {
            for (rung, s) in [p.a, p.b].into_iter().zip(row.a_b_stats()) {
                let m = s.moves as f64;
                let t = &mut thinking[rung];
                t[0] += m;
                t[1] += s.nodes_per_move * m;
                t[2] += s.depth_per_move * m;
                t[3] += s.ms_per_move * m;
            }
        }
    }
    let versions = rungs
        .iter()
        .enumerate()
        .map(|(i, rung)| {
            let t = thinking[i];
            let per_move = |k: usize| (t[0] > 0.0).then(|| t[k] / t[0]);
            let has_games = played.iter().any(|(p, _)| p.a == i || p.b == i);
            VersionSummary {
                rung: *rung,
                elo: has_games.then_some(fit.elo[i]),
                ci: bootstrap.players[i],
                nodes_per_move: per_move(1),
                depth_per_move: per_move(2),
                ms_per_move: per_move(3),
            }
        })
        .collect();
    Report {
        versions,
        pairings: summaries,
        fit,
        bootstrap,
        zero,
    }
}

fn summarise(pairing: &Pairing, rows: &[GameRow], ci: [f64; 2]) -> PairingSummary {
    let mut s = PairingSummary {
        pairing: pairing.clone(),
        games: rows.len() as u32,
        wins: 0,
        draws: 0,
        losses: 0,
        score: 0.0,
        elo_diff: None,
        ci,
        draws_by: [0; 4],
        no_move: 0,
        mean_plies: 0.0,
    };
    for row in rows {
        match row.a_score() {
            1.0 => s.wins += 1,
            0.0 => s.losses += 1,
            _ => s.draws += 1,
        }
        match row.reason {
            EndReason::Repetition => s.draws_by[0] += 1,
            EndReason::QuietPlies => s.draws_by[1] += 1,
            EndReason::OnePieceEach => s.draws_by[2] += 1,
            EndReason::Cap => s.draws_by[3] += 1,
            EndReason::NoMove => s.no_move += 1,
        }
        s.mean_plies += f64::from(row.plies);
    }
    let n = f64::from(s.games);
    s.score = (f64::from(s.wins) + 0.5 * f64::from(s.draws)) / n;
    s.mean_plies /= n;
    let elo = elo::elo_or_infinite(s.score);
    s.elo_diff = elo.is_finite().then_some(elo);
    s
}

/// What ladder.json says about how the games were played and rated.
#[derive(Clone, Debug, PartialEq)]
pub struct Meta {
    pub commit: String,
    pub date: String,
    pub threads: usize,
    /// The time of every match together, if known.
    pub wall_s: Option<f64>,
}

impl Report {
    /// ladder.json or ablation.json: meta, versions and pairings (docs/ladder.md describes the
    /// fields). The ablation's versions also say what each lacks (`removed`), and its pairings
    /// whether their games are the ladder's (`reused`).
    pub fn json(&self, meta: &Meta, set: &Set) -> String {
        let rungs = set.rungs;
        let ablation = rungs.iter().any(|r| r.removed.is_some());
        let ci = |c: [f64; 2]| Json::Arr(vec![Json::num(c[0], 1), Json::num(c[1], 1)]);
        let settings = Json::obj(vec![
            ("schedule", Json::str("paired")),
            ("opening_plies", Json::Num(4.0)),
            ("max_plies", Json::Num(400.0)),
            ("seed", Json::Num(f64::from(SEED))),
            ("threads", Json::Num(meta.threads as f64)),
            (
                "draws",
                Json::str("third repetition, 50 plies without a capture, one piece each, ply 400"),
            ),
            (
                "ms_per_move",
                Json::str("measured with several games at once: not a measurement of speed"),
            ),
        ]);
        let bootstrap = Json::obj(vec![
            ("samples", Json::Num(self.bootstrap.samples as f64)),
            ("seed", Json::Num(f64::from(self.bootstrap.seed))),
            ("rng", Json::str("mulberry32")),
            (
                "unit",
                Json::str("colour-swapped pairs of games, drawn again within each pairing"),
            ),
            (
                "percentiles",
                Json::Arr(vec![Json::Num(2.5), Json::Num(97.5)]),
            ),
            ("diverged", Json::Num(self.bootstrap.diverged as f64)),
        ]);
        let diverged: Vec<Json> = self
            .fit
            .diverged
            .iter()
            .map(|&i| Json::str(rungs[i].id))
            .collect();
        let meta_json = Json::obj(vec![
            ("commit", Json::str(&meta.commit)),
            ("date", Json::str(&meta.date)),
            ("settings", settings),
            (
                "elo_model",
                Json::str(
                    "Bradley-Terry, maximum likelihood over all games (draws count as half a \
                     win), E = 1 / (1 + 10^(-(Ra - Rb) / 400)); self-play left out",
                ),
            ),
            ("elo_zero", Json::str(rungs[self.zero].id)),
            ("mid_opponent", set.mid.map_or(Json::Null, Json::str)),
            ("bootstrap", bootstrap),
            ("diverged", Json::Arr(diverged)),
            ("wall_s", Json::opt(meta.wall_s, 1)),
        ]);
        let versions = self
            .versions
            .iter()
            .map(|v| {
                let mut fields = vec![
                    ("id", Json::str(v.rung.id)),
                    ("tag", Json::str(v.rung.tag)),
                    ("spec", Json::str(v.rung.spec)),
                    ("label", Json::str(v.rung.label)),
                    ("adds", Json::str(v.rung.adds)),
                    ("side", Json::Bool(v.rung.beside.is_some())),
                ];
                if ablation {
                    fields.push(("removed", v.rung.removed.map_or(Json::Null, Json::str)));
                }
                fields.extend([
                    ("elo", Json::opt(v.elo, 1)),
                    ("ci", ci(v.ci)),
                    ("avg_nodes_per_move", Json::opt(v.nodes_per_move, 1)),
                    ("avg_depth", Json::opt(v.depth_per_move, 3)),
                    ("ms_per_move", Json::opt(v.ms_per_move, 3)),
                ]);
                Json::obj(fields)
            })
            .collect();
        let pairings = self
            .pairings
            .iter()
            .map(|p| {
                let roles = p.pairing.roles.iter().map(|r| Json::Str(r.to_string()));
                let mut fields = vec![
                    ("a", Json::str(rungs[p.pairing.a].id)),
                    ("b", Json::str(rungs[p.pairing.b].id)),
                    ("roles", Json::Arr(roles.collect())),
                ];
                if ablation {
                    fields.push(("reused", Json::Bool(p.pairing.reused)));
                }
                fields.extend([
                    ("games", Json::Num(f64::from(p.games))),
                    ("w", Json::Num(f64::from(p.wins))),
                    ("d", Json::Num(f64::from(p.draws))),
                    ("l", Json::Num(f64::from(p.losses))),
                    ("score", Json::num(p.score, 4)),
                    ("elo_diff", Json::opt(p.elo_diff, 1)),
                    ("ci", ci(p.ci)),
                    (
                        "draws_by",
                        Json::obj(vec![
                            ("repetition", Json::Num(f64::from(p.draws_by[0]))),
                            ("fifty", Json::Num(f64::from(p.draws_by[1]))),
                            ("one_each", Json::Num(f64::from(p.draws_by[2]))),
                            ("ply_cap", Json::Num(f64::from(p.draws_by[3]))),
                        ]),
                    ),
                    ("no_move", Json::Num(f64::from(p.no_move))),
                    ("avg_plies", Json::num(p.mean_plies, 1)),
                ]);
                Json::obj(fields)
            })
            .collect();
        let footnotes = set
            .footnotes
            .iter()
            .map(|(id, tag, note)| {
                Json::obj(vec![
                    ("id", Json::str(id)),
                    ("tag", Json::str(tag)),
                    ("note", Json::str(note)),
                ])
            })
            .collect();
        let mut out = String::new();
        Json::obj(vec![
            ("meta", meta_json),
            ("versions", Json::Arr(versions)),
            ("pairings", Json::Arr(pairings)),
            ("footnotes", Json::Arr(footnotes)),
        ])
        .write(&mut out, 0);
        out.push('\n');
        out
    }

    /// The two tables for the terminal: the versions, then the pairings.
    pub fn tables(&self, rungs: &[Rung]) -> String {
        let elo = |x: f64| {
            if x.is_finite() {
                format!("{x:+.0}")
            } else if x.is_nan() {
                "?".to_string()
            } else if x > 0.0 {
                "+inf".to_string()
            } else {
                "-inf".to_string()
            }
        };
        let opt = |x: Option<f64>, d: usize| x.map_or("-".to_string(), |x| format!("{x:.d$}"));
        let mut out = format!(
            "{:<5} {:<24} {:>7} {:>16} {:>9} {:>6} {:>8}\n",
            "id", "spec", "Elo", "95%", "nodes", "depth", "ms"
        );
        for v in &self.versions {
            out.push_str(&format!(
                "{:<5} {:<24} {:>7} {:>16} {:>9} {:>6} {:>8}\n",
                v.rung.id,
                v.rung.spec,
                v.elo.map_or("-".to_string(), elo),
                format!("[{}, {}]", elo(v.ci[0]), elo(v.ci[1])),
                opt(v.nodes_per_move, 0),
                opt(v.depth_per_move, 2),
                opt(v.ms_per_move, 2),
            ));
        }
        out.push_str(&format!(
            "\n{:<10} {:<22} {:>5} {:>14} {:>7} {:>7} {:>16} {:>15}\n",
            "pairing", "roles", "games", "+ = -", "score", "Elo", "95%", "rep/50/1-1/cap"
        ));
        for p in &self.pairings {
            let roles: Vec<String> = p.pairing.roles.iter().map(Role::to_string).collect();
            out.push_str(&format!(
                "{:<10} {:<22} {:>5} {:>14} {:>6.1}% {:>7} {:>16} {:>15}\n",
                p.pairing.name(rungs),
                roles.join(","),
                p.games,
                format!("+{} ={} -{}", p.wins, p.draws, p.losses),
                100.0 * p.score,
                p.elo_diff.map_or("-".to_string(), elo),
                format!("[{}, {}]", elo(p.ci[0]), elo(p.ci[1])),
                format!(
                    "{}/{}/{}/{}",
                    p.draws_by[0], p.draws_by[1], p.draws_by[2], p.draws_by[3]
                ),
            ));
        }
        out
    }
}
