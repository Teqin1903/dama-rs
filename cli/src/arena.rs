//! `dama arena <A> <B>`: plays a match between two agents, prints a report and appends one row to
//! a CSV file, results/arena.csv unless `--csv` says otherwise. The games themselves are played by
//! `engine::play_match`; this file reads the command line and writes the results.

use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use engine::{AgentSpec, Color, MatchPlan, MatchRecord, Schedule, Sprt, Thinking, play_match};

const HELP: &str = "\
usage: dama arena <A> <B> [--games N] [--seed S] [--schedule paired|legacy] [--threads T]
                          [--sprt [--elo0 E0] [--elo1 E1] [--alpha A] [--beta B]
                                  [--min-games M]]
                          [--csv PATH] [--log PATH]

Plays N games between agents A and B from the start position and ends them by the rules of
docs/reference.md. With the paired schedule (the default) every game has its own seeds, the first
4 plies are random, and each opening is played twice with the colours swapped (A is White in even
games). The legacy schedule is that of every match before it: one generator for the match and 2
random plies; it repeats the old rows of results/arena.csv and the games of bench/playgame_log.js.
With --sprt the match stops as soon as a sequential probability ratio test decides whether A is
better than B (H1: A is E1 Elo stronger) or not (H0: A is E0 Elo stronger), counting the pairs of
games of each opening; N is then the most games. Prints the result from A's point of view and
appends one row to the CSV file.

agents:
  random            a random legal move
  material          the baseline agent (one ply, U) with weights man/king/mob = 1/3/0
  first             the baseline agent with weights 1/3/0.1
  tuned             the baseline agent with weights 1/2.5/-0.05
  u:MAN/KING/MOB    the baseline agent with any weights, for example u:1/2/0.2
  mmN               minimax N plies deep (1 to 9) with the tuned weights, for example mm3
  abN               alpha-beta N plies deep (1 to 9): the same moves as mmN, faster
  id:LIMITS         iterative deepening: alpha-beta 1, 2, 3, ... plies deep, the move of the
                    deepest completed iteration; LIMITS are one or more of dN (at most N plies),
                    nK (at most K nodes per move), tMS (at most MS milliseconds per move),
                    separated by ':', for example id:n20000 or id:d5:t50. Matches with a time
                    limit cannot be repeated exactly
  abNq, idq:LIMITS  abN and id:LIMITS with quiescence: past the depth limit, a position whose
                    side to move must capture is searched on, captures only, until it is quiet
  abNr, abNqr,      abN, abNq, id:LIMITS and idq:LIMITS that recognise repetitions: a position
  idr:LIMITS,       of the game or of the search path that occurs again is a draw (0), so the
  idqr:LIMITS       agent avoids repeating when ahead and seeks it when behind

options:
  --games N         the number of games, even for the paired schedule (default 200); with --sprt
                    the most games (default 2000)
  --seed S          the seed of the match, 0 to 4294967295 (default 1)
  --schedule NAME   paired or legacy (default paired)
  --threads T       play T games at the same time (default 1). The games are the same with any
                    T, the time per move is not: it is a measurement only with --threads 1, and
                    the CSV row leaves it empty otherwise
  --sprt            stop when the SPRT decides (paired schedule only); the CSV row gets the
                    verdict (H0, H1, or none when N games were played first) and its LLR
  --elo0 E0         Elo of A over B under H0 (default 0)
  --elo1 E1         Elo of A over B under H1, more than E0 (default 20)
  --alpha A         the chance of accepting H1 when H0 holds, 0 < A < 0.5 (default 0.05)
  --beta B          the chance of accepting H0 when H1 holds, 0 < B < 0.5 (default 0.05)
  --min-games M     no verdict before M games, at most N (default 100)
  --csv PATH        the CSV file to append the row to (default results/arena.csv)
  --log PATH        also write every game with its moves to PATH, one TSV line per game
";

/// The columns of the CSV file, one row per run. `games` is the number of games played (with an
/// SPRT, up to the pair that decided). `a_us_per_move` and `b_us_per_move` are empty when
/// `threads` is more than 1; `wall_s` is the time of the whole match. The SPRT columns, `sprt`
/// (H0, H1 or none) to `min_games`, are empty without an SPRT.
const CSV_HEADER: &str = "date,version,build,a,b,games,seed,wins,draws,losses,score_pct,se_pct,\
mean_plies,a_us_per_move,b_us_per_move,repetition,no_move,one_piece_each,fifty_quiet,cap,\
a_nodes_per_move,b_nodes_per_move,schedule,opening_plies,threads,wall_s,\
sprt,llr,elo0,elo1,alpha,beta,max_games,min_games";

/// Debug builds are many times slower: the times per move of a debug build are not measurements.
const BUILD: &str = if cfg!(debug_assertions) {
    "debug"
} else {
    "release"
};

/// What the command line asks for.
#[derive(Debug, PartialEq)]
struct Args {
    a: AgentSpec,
    b: AgentSpec,
    games: u32,
    seed: u32,
    schedule: Schedule,
    threads: usize,
    /// Stop when this test decides; `games` is then the most games.
    sprt: Option<Sprt>,
    csv: PathBuf,
    log: Option<PathBuf>,
}

impl Args {
    fn plan(&self) -> MatchPlan {
        MatchPlan {
            games: self.games,
            seed: self.seed,
            schedule: self.schedule,
            threads: self.threads,
            sprt: self.sprt,
        }
    }
}

/// Runs `dama arena` with the arguments that follow `arena`.
pub fn main(args: impl Iterator<Item = String>) -> ExitCode {
    let args = match parse_args(args) {
        Ok(Some(args)) => args,
        Ok(None) => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("dama arena: {message}\n\n{HELP}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dama arena: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The arguments, or `None` if they ask for help.
fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<Args>, String> {
    let mut args = args.into_iter();
    let mut agents = Vec::new();
    let (mut games, mut seed): (Option<u32>, u32) = (None, 1);
    let (mut schedule, mut threads) = (Schedule::Paired, 1);
    let mut sprt_flag = false;
    let mut sprt = Sprt::default();
    let mut sprt_options = Vec::new();
    let mut csv = PathBuf::from("results/arena.csv");
    let mut log = None;
    while let Some(arg) = args.next() {
        if arg == "-h" || arg == "--help" {
            return Ok(None);
        }
        if !arg.starts_with("--") {
            agents.push(arg.parse::<AgentSpec>().map_err(|e| e.to_string())?);
            continue;
        }
        if arg == "--sprt" {
            sprt_flag = true;
            continue;
        }
        let value = args.next().ok_or(format!("{arg} needs a value"))?;
        let number = || {
            value
                .parse::<f64>()
                .ok()
                .filter(|x| x.is_finite())
                .ok_or(format!("{arg} {value}: expected a number"))
        };
        match arg.as_str() {
            "--games" => {
                games = Some(
                    value
                        .parse()
                        .ok()
                        .filter(|&n| n > 0)
                        .ok_or(format!("--games {value}: expected a number of games"))?,
                );
            }
            "--elo0" => sprt.elo0 = number()?,
            "--elo1" => sprt.elo1 = number()?,
            "--alpha" => sprt.alpha = number()?,
            "--beta" => sprt.beta = number()?,
            "--min-games" => {
                sprt.min_games = value
                    .parse()
                    .map_err(|_| format!("--min-games {value}: expected a number of games"))?;
            }
            "--seed" => {
                seed = value
                    .parse()
                    .map_err(|_| format!("--seed {value}: expected 0 to 4294967295"))?;
            }
            "--schedule" => schedule = value.parse().map_err(|e| format!("--schedule: {e}"))?,
            "--threads" => {
                threads = value
                    .parse()
                    .ok()
                    .filter(|&n| n > 0)
                    .ok_or(format!("--threads {value}: expected a number of threads"))?;
            }
            "--csv" => csv = PathBuf::from(value),
            "--log" => log = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown option {arg}")),
        }
        if ["--elo0", "--elo1", "--alpha", "--beta", "--min-games"].contains(&arg.as_str()) {
            sprt_options.push(arg);
        }
    }
    let [a, b]: [AgentSpec; 2] = agents
        .try_into()
        .map_err(|agents: Vec<_>| format!("expected two agents, found {}", agents.len()))?;
    let sprt = if sprt_flag {
        if schedule != Schedule::Paired {
            return Err("--sprt counts pairs of games: it needs the paired schedule".to_string());
        }
        if sprt.elo0 >= sprt.elo1 {
            return Err(format!(
                "--elo0 {} --elo1 {}: E0 must be less than E1",
                sprt.elo0, sprt.elo1
            ));
        }
        for (name, p) in [("--alpha", sprt.alpha), ("--beta", sprt.beta)] {
            if !(p > 0.0 && p < 0.5) {
                return Err(format!("{name} {p}: expected a number between 0 and 0.5"));
            }
        }
        Some(sprt)
    } else if let Some(option) = sprt_options.first() {
        return Err(format!("{option} sets the SPRT: it needs --sprt"));
    } else {
        None
    };
    let games = games.unwrap_or(if sprt.is_some() { 2000 } else { 200 });
    if let Some(sprt) = sprt
        && sprt.min_games > games
    {
        return Err(format!(
            "--min-games {}: more than the most games, {games}",
            sprt.min_games
        ));
    }
    if schedule == Schedule::Paired && !games.is_multiple_of(2) {
        return Err(format!(
            "--games {games}: the paired schedule plays every opening twice, so it needs an even \
             number of games"
        ));
    }
    Ok(Some(Args {
        a,
        b,
        games,
        seed,
        schedule,
        threads,
        sprt,
        csv,
        log,
    }))
}

/// Plays the match, prints the report, writes the log if asked and appends the CSV row.
/// `Box<dyn Error>` holds any error: an agent's, or one from the file system.
fn run(args: &Args) -> Result<(), Box<dyn Error>> {
    // Each thread of the match builds its own agents with these closures.
    let (make_a, make_b) = (|| args.a.build(), || args.b.build());
    let started = Instant::now();
    let record = play_match(&make_a, &make_b, &args.plan())?;
    let elapsed = started.elapsed();
    let version = code_version().unwrap_or_else(|| {
        eprintln!("dama arena: warning: no version from git; writing \"unknown\"");
        "unknown".to_string()
    });
    print!("{}", report(args, &record, &version, elapsed));
    if let Some(path) = &args.log {
        write_log(path, args, &record)?;
        println!("  games written to {}", path.display());
    }
    let row = csv_row(&utc(unix_now()), &version, BUILD, args, &record, elapsed);
    append_csv(&args.csv, &row)?;
    println!("  appended to {}", args.csv.display());
    Ok(())
}

/// `tuned (1/2.5/-0.05)`: the agent's name, and its weights if the name does not show them.
fn describe(spec: &AgentSpec) -> String {
    match (spec, spec.preset()) {
        (AgentSpec::U(weights), Some(name)) => format!("{name} ({weights})"),
        (AgentSpec::Minimax(depth), _) => {
            format!(
                "{spec} (minimax, {depth} plies, {})",
                engine::Weights::TUNED
            )
        }
        (
            AgentSpec::AlphaBeta {
                depth,
                quiescence,
                repetition,
            },
            _,
        ) => {
            format!(
                "{spec} (alpha-beta, {depth} plies{}, {})",
                extras(*quiescence, *repetition),
                engine::Weights::TUNED
            )
        }
        (
            AgentSpec::Deepening {
                quiescence,
                repetition,
                ..
            },
            _,
        ) => format!(
            "{spec} (iterative deepening alpha-beta{}, {})",
            extras(*quiescence, *repetition),
            engine::Weights::TUNED
        ),
        _ => spec.to_string(),
    }
}

/// `, quiescence` and `, repetitions` for the searches that have them.
fn extras(quiescence: bool, repetition: bool) -> String {
    let mut extras = String::new();
    if quiescence {
        extras.push_str(", quiescence");
    }
    if repetition {
        extras.push_str(", repetitions");
    }
    extras
}

/// The report printed after the match, from A's point of view.
fn report(args: &Args, record: &MatchRecord, version: &str, elapsed: Duration) -> String {
    let (wins, draws, losses) = record.wins_draws_losses();
    let percent = |x: Option<f64>| x.map_or("-".to_string(), |x| format!("{:.2}", 100.0 * x));
    let per_move = |t: &Thinking| match t.per_move() {
        Some(d) => format!("{} ({} moves)", duration(d), t.moves),
        None => "- (no moves)".to_string(),
    };
    let timing = if args.threads > 1 {
        format!(" ({} threads: not a measurement)", args.threads)
    } else {
        String::new()
    };
    let settings = args.schedule.settings();
    let layout = match args.schedule {
        Schedule::Legacy => format!(
            "{} random opening plies, colours alternate",
            settings.opening_plies
        ),
        Schedule::Paired => format!(
            "openings of {} random plies, each played twice with the colours swapped",
            settings.opening_plies
        ),
    };
    let nodes = |t: &Thinking| {
        t.nodes_per_move()
            .map_or("-".to_string(), |n| format!("{n:.1}"))
    };
    let endings: Vec<String> = record
        .endings()
        .iter()
        .map(|(reason, n)| format!("{reason} {n}"))
        .collect();
    let games = match &record.sprt {
        Some(_) => format!("{} of at most {} games", record.games.len(), args.games),
        None => format!("{} games", record.games.len()),
    };
    let sprt = record.sprt.map_or(String::new(), |result| {
        let (lower, upper) = result.sprt.bounds();
        let elo = match result.pairs.elo() {
            Some((elo, error)) => format!("{elo:.1} ± {error:.1}"),
            None => "-".to_string(),
        };
        let [p0, p1, p2, p3, p4] = result.pairs.0;
        format!(
            "  SPRT (Elo0 {}, Elo1 {}, alpha {}, beta {}, at least {} games): {} after {} pairs, \
             LLR {:.3} \
             (bounds {lower:.3}, {upper:.3})\n\
             \x20 pairs scoring 0/0.5/1/1.5/2: {p0}/{p1}/{p2}/{p3}/{p4}; Elo of A over B {elo} (95%)\n",
            result.sprt.elo0,
            result.sprt.elo1,
            result.sprt.alpha,
            result.sprt.beta,
            result.sprt.min_games,
            result.verdict,
            result.pairs.pairs(),
            result.llr,
        )
    });
    format!(
        "A {} vs B {}: {games}, seed {}, {} schedule: {layout}\n\
         {sprt}\
         \x20 A: +{wins} ={draws} -{losses}, score {}% (SE {})\n\
         \x20 plies per game: {}\n\
         \x20 time per move{timing}: A {}, B {}\n\
         \x20 nodes per move: A {}, B {}\n\
         \x20 endings: {}\n\
         \x20 version {version} ({BUILD} build), {} in all with {} thread{}\n",
        describe(&args.a),
        describe(&args.b),
        args.seed,
        args.schedule,
        percent(record.score()),
        percent(record.standard_error()),
        record
            .mean_plies()
            .map_or("-".to_string(), |p| format!("{p:.1}")),
        per_move(&record.a),
        per_move(&record.b),
        nodes(&record.a),
        nodes(&record.b),
        endings.join(", "),
        duration(elapsed),
        args.threads,
        if args.threads == 1 { "" } else { "s" },
    )
}

/// A duration in the unit that suits it: 40 ns, 41.3 µs, 2.51 ms, 1.20 s.
fn duration(d: Duration) -> String {
    let secs = d.as_secs_f64();
    if secs < 1e-6 {
        format!("{:.0} ns", secs * 1e9)
    } else if secs < 1e-3 {
        format!("{:.1} µs", secs * 1e6)
    } else if secs < 1.0 {
        format!("{:.2} ms", secs * 1e3)
    } else {
        format!("{secs:.2} s")
    }
}

/// Writes every game as one TSV line: game, white, black, result, reason, plies, moves.
fn write_log(path: &Path, args: &Args, record: &MatchRecord) -> io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(out, "game\twhite\tblack\tresult\treason\tplies\tmoves")?;
    for (g, game) in record.games.iter().enumerate() {
        let (white, black) = if game.a_white {
            (args.a, args.b)
        } else {
            (args.b, args.a)
        };
        let result = match game.record.winner {
            Some(Color::White) => "1-0",
            Some(Color::Black) => "0-1",
            None => "1/2-1/2",
        };
        let moves: Vec<String> = game.record.moves.iter().map(|m| m.to_string()).collect();
        writeln!(
            out,
            "{g}\t{white}\t{black}\t{result}\t{}\t{}\t{}",
            game.record.reason,
            game.record.plies(),
            moves.join(" ")
        )?;
    }
    out.flush()
}

/// One row of the CSV file; see `CSV_HEADER` for the columns.
fn csv_row(
    date: &str,
    version: &str,
    build: &str,
    args: &Args,
    record: &MatchRecord,
    elapsed: Duration,
) -> String {
    let (wins, draws, losses) = record.wins_draws_losses();
    let number = |x: Option<f64>, scale: f64, decimals: usize| {
        x.map_or(String::new(), |x| format!("{:.decimals$}", x * scale))
    };
    // Threads that play at the same time slow each other down: no time per move then.
    let micros = |t: &Thinking| {
        let per_move = t.per_move().filter(|_| args.threads == 1);
        number(per_move.map(|d| d.as_secs_f64()), 1e6, 2)
    };
    let nodes = |t: &Thinking| number(t.nodes_per_move(), 1.0, 1);
    let endings: Vec<String> = record
        .endings()
        .iter()
        .map(|(_, n)| n.to_string())
        .collect();
    let fields = [
        csv_field(date),
        csv_field(version),
        csv_field(build),
        csv_field(&args.a.to_string()),
        csv_field(&args.b.to_string()),
        record.games.len().to_string(),
        args.seed.to_string(),
        wins.to_string(),
        draws.to_string(),
        losses.to_string(),
        number(record.score(), 100.0, 2),
        number(record.standard_error(), 100.0, 2),
        number(record.mean_plies(), 1.0, 1),
        micros(&record.a),
        micros(&record.b),
        endings.join(","),
        nodes(&record.a),
        nodes(&record.b),
        args.schedule.to_string(),
        args.schedule.settings().opening_plies.to_string(),
        args.threads.to_string(),
        format!("{:.1}", elapsed.as_secs_f64()),
    ];
    let sprt: [String; 8] = match &record.sprt {
        Some(result) => [
            result.verdict.to_string(),
            format!("{:.3}", result.llr),
            result.sprt.elo0.to_string(),
            result.sprt.elo1.to_string(),
            result.sprt.alpha.to_string(),
            result.sprt.beta.to_string(),
            args.games.to_string(),
            result.sprt.min_games.to_string(),
        ],
        None => Default::default(),
    };
    fields.join(",") + "," + &sprt.join(",")
}

/// A text field of a CSV row, quoted if it holds a comma or a quote.
fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Appends `row` to the CSV file at `path`, writing the header first if the file is new or empty.
/// A file with other columns is left alone: the new row would not match them.
fn append_csv(path: &Path, row: &str) -> io::Result<()> {
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        fs::create_dir_all(dir)?;
    }
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(path)?;
    let mut header = String::new();
    BufReader::new(&file).read_line(&mut header)?;
    if header.is_empty() {
        writeln!(file, "{CSV_HEADER}")?;
    } else if header.trim_end() != CSV_HEADER {
        return Err(io::Error::other(format!(
            "{} has other columns; move it away or choose another file with --csv",
            path.display()
        )));
    }
    writeln!(file, "{row}")
}

/// The version of the code: `git describe --tags --always`, and `-dirty` if a tracked file
/// outside results/ differs from HEAD. The rows the arena appends to results/ do not count, so
/// two runs of the same code report the same version. `None` if git cannot tell.
fn code_version() -> Option<String> {
    let describe = git(&["describe", "--tags", "--always"])?;
    let changed = git(&[
        "status",
        "--porcelain",
        "--untracked-files=no",
        "--",
        ":(top,exclude)results",
    ])?;
    Some(if changed.is_empty() {
        describe
    } else {
        format!("{describe}-dirty")
    })
}

/// The output of git with these arguments, or `None` if it fails.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    let stdout = String::from_utf8(output.stdout).ok()?;
    output.status.success().then(|| stdout.trim().to_string())
}

/// Seconds since 1970-01-01T00:00:00Z.
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Seconds since 1970-01-01T00:00:00Z as ISO 8601 in UTC, for example 2026-10-25T16:20:05Z.
/// The date is Howard Hinnant's `civil_from_days`: it counts in eras of 400 years, which start
/// on March 1st so that the leap day comes last.
fn utc(secs: u64) -> String {
    let (days, time) = (secs / 86_400, secs % 86_400);
    let z = days + 719_468; // days since 0000-03-01
    let (era, day_of_era) = (z / 146_097, z % 146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 {
        month_from_march + 3
    } else {
        month_from_march - 9
    };
    let year = era * 400 + year_of_era + u64::from(month <= 2);
    let (hour, minute, second) = (time / 3600, time / 60 % 60, time % 60);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Board, EndReason, GameRecord, MatchGame};

    fn parse(args: &[&str]) -> Result<Option<Args>, String> {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_and_options() {
        assert_eq!(
            parse(&["tuned", "random"]),
            Ok(Some(Args {
                a: AgentSpec::U(engine::Weights::TUNED),
                b: AgentSpec::Random,
                games: 200,
                seed: 1,
                schedule: Schedule::Paired,
                threads: 1,
                sprt: None,
                csv: PathBuf::from("results/arena.csv"),
                log: None,
            }))
        );
        let args = parse(&[
            "tuned",
            "random",
            "--schedule",
            "legacy",
            "--threads",
            "6",
            "--games",
            "7",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(
            (args.schedule, args.threads, args.games),
            (Schedule::Legacy, 6, 7)
        );
        let args = parse(&[
            "--games",
            "10",
            "u:1/2/0.2",
            "--seed",
            "4294967295",
            "first",
            "--csv",
            "x.csv",
            "--log",
            "g.tsv",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(
            (args.a.to_string(), args.b.to_string()),
            ("u:1/2/0.2".to_string(), "first".to_string())
        );
        assert_eq!((args.games, args.seed), (10, u32::MAX));
        assert_eq!(args.csv, PathBuf::from("x.csv"));
        assert_eq!(args.log, Some(PathBuf::from("g.tsv")));
        assert_eq!(parse(&["tuned", "--help"]), Ok(None));
        let args = parse(&["tuned", "random", "--sprt"]).unwrap().unwrap();
        assert_eq!((args.sprt, args.games), (Some(Sprt::default()), 2000));
        let args = parse(&[
            "tuned",
            "random",
            "--elo0",
            "-5",
            "--sprt",
            "--elo1",
            "5",
            "--alpha",
            "0.1",
            "--beta",
            "0.2",
            "--games",
            "500",
            "--min-games",
            "40",
        ])
        .unwrap()
        .unwrap();
        let sprt = Sprt {
            elo0: -5.0,
            elo1: 5.0,
            alpha: 0.1,
            beta: 0.2,
            min_games: 40,
        };
        assert_eq!((args.sprt, args.games), (Some(sprt), 500));
    }

    #[test]
    fn bad_arguments() {
        for args in [
            &["tuned"][..],
            &["tuned", "random", "first"],
            &["tuned", "rando"],
            &["tuned", "random", "--games"],
            &["tuned", "random", "--games", "0"],
            &["tuned", "random", "--games", "-3"],
            &["tuned", "random", "--seed", "4294967296"],
            &["tuned", "random", "--sed", "1"],
            &["tuned", "random", "--games", "7"],
            &["tuned", "random", "--schedule", "pairs"],
            &["tuned", "random", "--threads", "0"],
            &["tuned", "random", "--elo1", "10"],
            &["tuned", "random", "--sprt", "--schedule", "legacy"],
            &["tuned", "random", "--sprt", "--elo0", "20"],
            &["tuned", "random", "--sprt", "--elo1", "x"],
            &["tuned", "random", "--sprt", "--alpha", "0.5"],
            &["tuned", "random", "--sprt", "--beta", "0"],
            &["tuned", "random", "--sprt", "--games", "7"],
            &["tuned", "random", "--min-games", "50"],
            &["tuned", "random", "--sprt", "--min-games", "-2"],
            &[
                "tuned",
                "random",
                "--sprt",
                "--games",
                "100",
                "--min-games",
                "102",
            ],
        ] {
            assert!(parse(args).is_err(), "{args:?} should be an error");
        }
    }

    #[test]
    fn a_csv_row_matches_the_header() {
        let some_move = Board::start().legal_moves(Color::White).swap_remove(0);
        let game = |a_white, winner, reason| MatchGame {
            a_white,
            record: GameRecord {
                winner,
                reason,
                moves: vec![some_move.clone(); 40],
            },
        };
        let record = MatchRecord {
            games: vec![
                game(true, Some(Color::White), EndReason::NoMove),
                game(false, None, EndReason::Repetition),
            ],
            a: Thinking {
                moves: 4,
                time: Duration::from_micros(10),
                nodes: 90,
            },
            b: Thinking::default(),
            sprt: None,
        };
        let args = parse(&["tuned", "u:1/2/0.2", "--games", "2"])
            .unwrap()
            .unwrap();
        let row = |args: &Args| {
            csv_row(
                "2026-10-25T00:00:00Z",
                "baseline-1-gabc",
                "release",
                args,
                &record,
                Duration::from_millis(12_340),
            )
        };
        let one = row(&args);
        assert_eq!(
            one,
            "2026-10-25T00:00:00Z,baseline-1-gabc,release,tuned,u:1/2/0.2,2,1,1,1,0,75.00,25.00,\
             40.0,2.50,,1,1,0,0,0,22.5,,paired,4,1,12.3,,,,,,,,"
        );
        assert_eq!(one.split(',').count(), CSV_HEADER.split(',').count());
        // With several threads the time per move is left out.
        let args = parse(&[
            "tuned",
            "u:1/2/0.2",
            "--games",
            "2",
            "--schedule",
            "legacy",
            "--threads",
            "6",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(
            row(&args),
            "2026-10-25T00:00:00Z,baseline-1-gabc,release,tuned,u:1/2/0.2,2,1,1,1,0,75.00,25.00,\
             40.0,,,1,1,0,0,0,22.5,,legacy,2,6,12.3,,,,,,,,"
        );
        // With an SPRT: the games played, the verdict and the most games.
        let args = parse(&["tuned", "u:1/2/0.2", "--sprt", "--elo1", "10"])
            .unwrap()
            .unwrap();
        let sprt = Sprt {
            elo1: 10.0,
            ..Sprt::default()
        };
        let record = MatchRecord {
            sprt: Some(engine::SprtResult {
                sprt,
                verdict: engine::Verdict::H1,
                llr: 2.9512,
                pairs: engine::Pentanomial([0, 0, 0, 1, 0]),
            }),
            ..record.clone()
        };
        let with_sprt = csv_row(
            "2026-10-25T00:00:00Z",
            "baseline-1-gabc",
            "release",
            &args,
            &record,
            Duration::from_millis(12_340),
        );
        assert_eq!(
            with_sprt,
            "2026-10-25T00:00:00Z,baseline-1-gabc,release,tuned,u:1/2/0.2,2,1,1,1,0,75.00,25.00,\
             40.0,2.50,,1,1,0,0,0,22.5,,paired,4,1,12.3,H1,2.951,0,10,0.05,0.05,2000,100"
        );
        assert_eq!(with_sprt.split(',').count(), CSV_HEADER.split(',').count());
    }

    #[test]
    fn csv_fields_with_commas_are_quoted() {
        assert_eq!(csv_field("v1"), "v1");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn utc_dates() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc(1_792_886_400), "2026-10-25T00:00:00Z");
        assert_eq!(utc(1_792_946_405), "2026-10-25T16:40:05Z");
    }
}
