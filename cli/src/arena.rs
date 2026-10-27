//! `dama arena <A> <B>`: plays a match between two agents, prints a report and appends one row to
//! a CSV file, results/arena.csv unless `--csv` says otherwise. The games themselves are played by
//! `engine::play_match`; this file reads the command line and writes the results.

use std::error::Error;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use engine::{AgentSpec, Color, MatchRecord, MatchSettings, Thinking, play_match};

const HELP: &str = "\
usage: dama arena <A> <B> [--games N] [--seed S] [--csv PATH] [--log PATH]

Plays N games between agents A and B from the start position. Colours alternate (A is White in
even games), the first 2 plies of every game are random, and games end by the rules of
docs/reference.md. Prints the result from A's point of view and appends one row to the CSV file.

agents:
  random            a random legal move
  material          the baseline agent (one ply, U) with weights man/king/mob = 1/3/0
  first             the baseline agent with weights 1/3/0.1
  tuned             the baseline agent with weights 1/2.5/-0.05
  u:MAN/KING/MOB    the baseline agent with any weights, for example u:1/2/0.2
  mmN               minimax N plies deep (1 to 9) with the tuned weights, for example mm3
  abN               alpha-beta N plies deep (1 to 9): the same moves as mmN, faster

options:
  --games N         the number of games (default 200)
  --seed S          the seed of the match, 0 to 4294967295 (default 1)
  --csv PATH        the CSV file to append the row to (default results/arena.csv)
  --log PATH        also write every game with its moves to PATH, one TSV line per game
";

/// The columns of the CSV file, one row per run.
const CSV_HEADER: &str = "date,version,build,a,b,games,seed,wins,draws,losses,score_pct,se_pct,\
mean_plies,a_us_per_move,b_us_per_move,repetition,no_move,one_piece_each,fifty_quiet,cap,\
a_nodes_per_move,b_nodes_per_move";

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
    csv: PathBuf,
    log: Option<PathBuf>,
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
    let (mut games, mut seed) = (200, 1);
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
        let value = args.next().ok_or(format!("{arg} needs a value"))?;
        match arg.as_str() {
            "--games" => {
                games = value
                    .parse()
                    .ok()
                    .filter(|&n| n > 0)
                    .ok_or(format!("--games {value}: expected a number of games"))?;
            }
            "--seed" => {
                seed = value
                    .parse()
                    .map_err(|_| format!("--seed {value}: expected 0 to 4294967295"))?;
            }
            "--csv" => csv = PathBuf::from(value),
            "--log" => log = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown option {arg}")),
        }
    }
    let [a, b]: [AgentSpec; 2] = agents
        .try_into()
        .map_err(|agents: Vec<_>| format!("expected two agents, found {}", agents.len()))?;
    Ok(Some(Args {
        a,
        b,
        games,
        seed,
        csv,
        log,
    }))
}

/// Plays the match, prints the report, writes the log if asked and appends the CSV row.
/// `Box<dyn Error>` holds any error: an agent's, or one from the file system.
fn run(args: &Args) -> Result<(), Box<dyn Error>> {
    let (mut a, mut b) = (args.a.build(), args.b.build());
    let started = Instant::now();
    let record = play_match(
        a.as_mut(),
        b.as_mut(),
        args.games,
        args.seed,
        &MatchSettings::REFERENCE,
    )?;
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
    let row = csv_row(&utc(unix_now()), &version, BUILD, args, &record);
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
        (AgentSpec::AlphaBeta(depth), _) => {
            format!(
                "{spec} (alpha-beta, {depth} plies, {})",
                engine::Weights::TUNED
            )
        }
        _ => spec.to_string(),
    }
}

/// The report printed after the match, from A's point of view.
fn report(args: &Args, record: &MatchRecord, version: &str, elapsed: Duration) -> String {
    let (wins, draws, losses) = record.wins_draws_losses();
    let percent = |x: Option<f64>| x.map_or("-".to_string(), |x| format!("{:.2}", 100.0 * x));
    let per_move = |t: &Thinking| match t.per_move() {
        Some(d) => format!("{} ({} moves)", duration(d), t.moves),
        None => "- (no moves)".to_string(),
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
    let settings = MatchSettings::REFERENCE;
    format!(
        "A {} vs B {}: {} games, seed {}, {} random opening plies, colours alternate\n\
         \x20 A: +{wins} ={draws} -{losses}, score {}% (SE {})\n\
         \x20 plies per game: {}\n\
         \x20 time per move: A {}, B {}\n\
         \x20 nodes per move: A {}, B {}\n\
         \x20 endings: {}\n\
         \x20 version {version} ({BUILD} build), {} in all\n",
        describe(&args.a),
        describe(&args.b),
        args.games,
        args.seed,
        settings.opening_plies,
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
fn csv_row(date: &str, version: &str, build: &str, args: &Args, record: &MatchRecord) -> String {
    let (wins, draws, losses) = record.wins_draws_losses();
    let number = |x: Option<f64>, scale: f64, decimals: usize| {
        x.map_or(String::new(), |x| format!("{:.decimals$}", x * scale))
    };
    let micros = |t: &Thinking| number(t.per_move().map(|d| d.as_secs_f64()), 1e6, 2);
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
        args.games.to_string(),
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
    ];
    fields.join(",")
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
                csv: PathBuf::from("results/arena.csv"),
                log: None,
            }))
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
        };
        let args = parse(&["tuned", "u:1/2/0.2", "--games", "2"])
            .unwrap()
            .unwrap();
        let row = csv_row(
            "2026-10-25T00:00:00Z",
            "baseline-1-gabc",
            "release",
            &args,
            &record,
        );
        assert_eq!(
            row,
            "2026-10-25T00:00:00Z,baseline-1-gabc,release,tuned,u:1/2/0.2,2,1,1,1,0,75.00,25.00,\
             40.0,2.50,,1,1,0,0,0,22.5,"
        );
        assert_eq!(row.split(',').count(), CSV_HEADER.split(',').count());
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
