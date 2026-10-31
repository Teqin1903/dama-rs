//! `dama ladder`: plays the matches of the Elo ladder (`engine::plan`), of the ablation
//! (`engine::ablation_plan`) or of Monte Carlo tree search (`engine::against_plan`) and rates the versions from their games (`engine::rate`). This file reads and writes the files; the ladder itself, its
//! pairings and its ratings live in engine/src/ladder.rs and engine/src/elo.rs.

use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use engine::{
    AgentSpec, GAMES_HEADER, GameRow, LADDER_SET, MatchPlan, Meta, Pairing, Rung, SEED, Schedule,
    Set, play_match, rate, rows_of,
};

use crate::arena::{self, Args};

const HELP: &str = "\
usage: dama ladder run [--set S] [--estimate] [--threads T] [--dir DIR] [--csv PATH]
       dama ladder rate [--set S] [--dir DIR] [--csv PATH] [--samples N] [--seed S]

The Elo ladder of the tagged versions (docs/ladder.md): every version against the one before it,
against v1-minimax (the zero) and against the one two steps back, the upper versions against the
mid-ladder opponent (v6-repetition), all paired matches with seed 1. The ablation plays
v10-transposition-table against itself with one part turned off and against the baseline.
The mcts set plays four forms of Monte Carlo tree search against V0, V2 and V10, 200 games each.

run         plays every pairing whose file DIR/games/<A>_vs_<B>.csv is missing, writes that file
            when the pairing is over, appends one row to the arena CSV and one line to
            DIR/progress.log. Rerun it to go on after an interruption: finished pairings are
            skipped
  --estimate  plays the first 12 games of every pairing, prints the time the whole run would
              take, and writes nothing
            A pairing the ladder has played already (the ablation's V10_vs_V9) is not played
            again: its games are read from results/ladder/games
rate        reads the games and writes DIR/ladder.json (ablation.json, mcts.json): one Bradley-Terry
            fit of all games, bootstrap intervals over the colour-swapped pairs of each pairing

options:
  --set S       ladder (default), ablation or mcts
  --threads T   games played at the same time (default 6)
  --dir DIR     the set's directory (default results/ladder, results/ablation, results/mcts)
  --csv PATH    the arena CSV (default results/arena.csv); rate takes the time and threads of
                each match from it
  --samples N   bootstrap samples (default 1000)
  --seed S      the bootstrap's mulberry32 seed (default 1)
";

/// Games of each pairing that `--estimate` plays.
const ESTIMATE_GAMES: u32 = 12;

struct Options {
    command: String,
    set: Set,
    estimate: bool,
    threads: usize,
    dir: PathBuf,
    csv: PathBuf,
    samples: usize,
    seed: u32,
}

/// Runs `dama ladder` with the arguments that follow `ladder`.
pub fn main(args: impl Iterator<Item = String>) -> ExitCode {
    let options = match parse_args(args) {
        Ok(Some(options)) => options,
        Ok(None) => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("dama ladder: {message}\n\n{HELP}");
            return ExitCode::from(2);
        }
    };
    let result = match options.command.as_str() {
        "run" => run(&options),
        _ => rate_command(&options),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dama ladder: {e}");
            ExitCode::FAILURE
        }
    }
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<Options>, String> {
    let mut args = args.into_iter();
    let command = match args.next().as_deref() {
        Some("run") => "run",
        Some("rate") => "rate",
        Some("-h" | "--help") | None => return Ok(None),
        Some(other) => return Err(format!("unknown command {other:?}: expected run or rate")),
    };
    let mut options = Options {
        command: command.to_string(),
        set: LADDER_SET,
        estimate: false,
        threads: 6,
        dir: PathBuf::new(),
        csv: PathBuf::from("results/arena.csv"),
        samples: 1000,
        seed: 1,
    };
    let mut dir = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--estimate" if command == "run" => {
                options.estimate = true;
                continue;
            }
            _ => {}
        }
        let value = args.next().ok_or(format!("{arg} needs a value"))?;
        let positive = |what: &str| {
            value
                .parse::<usize>()
                .ok()
                .filter(|&n| n > 0)
                .ok_or(format!("{arg} {value}: expected a number of {what}"))
        };
        match (command, arg.as_str()) {
            ("run", "--threads") => options.threads = positive("threads")?,
            (_, "--csv") => options.csv = PathBuf::from(value),
            ("rate", "--samples") => options.samples = positive("samples")?,
            ("rate", "--seed") => {
                options.seed = value
                    .parse()
                    .map_err(|_| format!("--seed {value}: expected 0 to 4294967295"))?;
            }
            (_, "--dir") => dir = Some(PathBuf::from(value)),
            (_, "--set") => {
                options.set = Set::ALL
                    .into_iter()
                    .find(|s| s.name == value)
                    .ok_or(format!("--set {value}: expected ladder, ablation or mcts"))?;
            }
            _ => return Err(format!("unknown option {arg} for {command}")),
        }
    }
    options.dir = dir.unwrap_or_else(|| PathBuf::from(options.set.dir));
    Ok(Some(options))
}

/// The file of a pairing's games: in the ladder's directory if the pairing is reused from it.
fn games_path(options: &Options, pairing: &Pairing) -> PathBuf {
    let dir = if pairing.reused {
        Path::new(LADDER_SET.dir)
    } else {
        &options.dir
    };
    dir.join("games")
        .join(format!("{}.csv", pairing.name(options.set.rungs)))
}

/// The games of a finished pairing, or `None` if its file is missing.
fn read_games(path: &Path) -> Result<Option<Vec<GameRow>>, Box<dyn Error>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    let mut lines = text.lines();
    if lines.next() != Some(GAMES_HEADER) {
        return Err(format!("{}: not a games file of this version", path.display()).into());
    }
    let rows = lines
        .enumerate()
        .map(|(k, line)| {
            line.parse::<GameRow>()
                .map_err(|e| format!("{} line {}: {e}", path.display(), k + 2))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(rows))
}

/// Checks that `rows` are the games `pairing` asks for, played by `version` if given.
fn check_games(
    path: &Path,
    rungs: &[Rung],
    pairing: &Pairing,
    rows: &[GameRow],
    version: Option<&str>,
) -> Result<(), String> {
    let (a, b) = (rungs[pairing.a].spec, rungs[pairing.b].spec);
    let fail = |what: String| {
        Err(format!(
            "{}: {what}; move the file away to play it again",
            path.display()
        ))
    };
    if rows.len() != pairing.games as usize {
        return fail(format!("{} games instead of {}", rows.len(), pairing.games));
    }
    for (g, row) in rows.iter().enumerate() {
        let (white, black) = if g % 2 == 0 { (a, b) } else { (b, a) };
        if row.game as usize != g || row.white != white || row.black != black {
            return fail(format!("game {g} is not {white} against {black}"));
        }
        if let Some(version) = version
            && row.version != version
        {
            return fail(format!(
                "played by version {}, this is {version}",
                row.version
            ));
        }
    }
    Ok(())
}

fn run(options: &Options) -> Result<(), Box<dyn Error>> {
    let version = arena::code_version().ok_or("no version from git")?;
    if version.ends_with("-dirty") && !options.estimate {
        eprintln!("dama ladder: warning: version {version}: the results will not name a commit");
    }
    let rungs = options.set.rungs;
    let pairings = options.set.plan();
    let total_games: u32 = pairings.iter().filter(|p| !p.reused).map(|p| p.games).sum();
    println!(
        "{}: {} pairings, {total_games} games to play, version {version}, {} threads",
        options.set.name,
        pairings.len(),
        options.threads
    );
    let started = Instant::now();
    let mut estimated = 0.0;
    for (k, pairing) in pairings.iter().enumerate() {
        let name = pairing.name(rungs);
        let path = games_path(options, pairing);
        if pairing.reused {
            let rows = read_games(&path)?.ok_or(format!("{}: missing", path.display()))?;
            check_games(&path, rungs, pairing, &rows, None)?;
            println!(
                "[{:>2}/{}] {name}: the ladder's games ({})",
                k + 1,
                pairings.len(),
                path.display()
            );
            continue;
        }
        if !options.estimate
            && let Some(rows) = read_games(&path)?
        {
            check_games(&path, rungs, pairing, &rows, Some(&version))?;
            println!("[{:>2}/{}] {name}: done already", k + 1, pairings.len());
            continue;
        }
        let games = if options.estimate {
            ESTIMATE_GAMES.min(pairing.games)
        } else {
            pairing.games
        };
        let args = Args {
            a: rungs[pairing.a].spec.parse::<AgentSpec>()?,
            b: rungs[pairing.b].spec.parse::<AgentSpec>()?,
            games,
            seed: SEED,
            schedule: Schedule::Paired,
            threads: options.threads,
            sprt: None,
            csv: options.csv.clone(),
            log: None,
        };
        let plan: MatchPlan = args.plan();
        let (make_a, make_b) = (|| args.a.build(), || args.b.build());
        let match_started = Instant::now();
        let record = play_match(&make_a, &make_b, &plan)?;
        let elapsed = match_started.elapsed();
        let (w, d, l) = record.wins_draws_losses();
        if options.estimate {
            let whole = elapsed.as_secs_f64() * f64::from(pairing.games) / f64::from(games);
            estimated += whole;
            println!(
                "[{:>2}/{}] {name:<10} {games} games in {:>6.1} s: {:>7.0} s for {}",
                k + 1,
                pairings.len(),
                elapsed.as_secs_f64(),
                whole,
                pairing.games
            );
            continue;
        }
        let rows = rows_of(
            &record,
            rungs[pairing.a].spec,
            rungs[pairing.b].spec,
            SEED,
            &version,
        );
        write_games(&path, &rows)?;
        let row = arena::csv_row(
            &arena::utc(arena::unix_now()),
            &version,
            arena::BUILD,
            &args,
            &record,
            elapsed,
        );
        arena::append_csv(&options.csv, &row)?;
        let line = format!(
            "{} [{:>2}/{}] {name}: {} games +{w} ={d} -{l}, {:.1} s (run {:.1} s)",
            arena::utc(arena::unix_now()),
            k + 1,
            pairings.len(),
            rows.len(),
            elapsed.as_secs_f64(),
            started.elapsed().as_secs_f64()
        );
        println!("{line}");
        let mut log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(options.dir.join("progress.log"))?;
        writeln!(log, "{line}")?;
    }
    if options.estimate {
        println!(
            "estimate: {:.0} s ({:.0} min) for the whole run; {:.0} s spent estimating",
            estimated,
            estimated / 60.0,
            started.elapsed().as_secs_f64()
        );
    }
    Ok(())
}

/// Writes the games to `path` through a temporary file, so that the file appears complete or not
/// at all.
fn write_games(path: &Path, rows: &[GameRow]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut text = format!("{GAMES_HEADER}\n");
    for row in rows {
        text.push_str(&format!("{row}\n"));
    }
    let tmp = path.with_extension("csv.tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)
}

fn rate_command(options: &Options) -> Result<(), Box<dyn Error>> {
    let rungs = options.set.rungs;
    let pairings = options.set.plan();
    let mut rows = Vec::with_capacity(pairings.len());
    let mut versions: Vec<String> = Vec::new();
    let mut wall = Some(0.0);
    let mut threads: Vec<usize> = Vec::new();
    let arena_rows = fs::read_to_string(&options.csv).unwrap_or_default();
    for pairing in &pairings {
        let path = games_path(options, pairing);
        let games = match read_games(&path)? {
            Some(games) => {
                check_games(&path, rungs, pairing, &games, None)?;
                games
            }
            None => {
                eprintln!(
                    "dama ladder: warning: {} not played yet",
                    pairing.name(rungs)
                );
                Vec::new()
            }
        };
        if let Some(first) = games.first() {
            if pairing.reused {
                println!(
                    "{}: the ladder's games, version {}",
                    pairing.name(rungs),
                    first.version
                );
            }
            for row in games.iter().filter(|_| !pairing.reused) {
                if !versions.contains(&row.version) {
                    versions.push(row.version.clone());
                }
            }
            // The arena CSV names agents as `AgentSpec` writes them (`w=tuned` left out).
            let name = |i: usize| rungs[i].spec.parse::<AgentSpec>().map(|s| s.to_string());
            let found = match_row(
                &arena_rows,
                &first.version,
                &name(pairing.a)?,
                &name(pairing.b)?,
                games.len(),
            );
            if let Some((_, t)) = found
                && !threads.contains(&t)
            {
                threads.push(t);
            }
            wall = wall.zip(found).map(|(sum, (s, _))| sum + s);
        }
        rows.push(games);
    }
    let zero = options.set.zero_index();
    let report = rate(rungs, &pairings, &rows, zero, options.samples, options.seed);
    print!("{}", report.tables(rungs));
    if !report.fit.diverged.is_empty() {
        let ids: Vec<&str> = report.fit.diverged.iter().map(|&i| rungs[i].id).collect();
        eprintln!(
            "dama ladder: warning: no finite Elo for {}: they won or lost every point against the \
             rest",
            ids.join(", ")
        );
    }
    if report.bootstrap.diverged > 0 {
        eprintln!(
            "dama ladder: warning: {} of {} bootstrap samples had a version with no finite Elo",
            report.bootstrap.diverged, report.bootstrap.samples
        );
    }
    if versions.len() > 1 {
        eprintln!(
            "dama ladder: warning: the games come from {} versions: {}",
            versions.len(),
            versions.join(", ")
        );
    }
    let meta = Meta {
        commit: versions.join(" "),
        date: arena::utc(arena::unix_now()),
        threads: match threads[..] {
            [t] => t,
            _ => {
                eprintln!("dama ladder: warning: threads per match {threads:?}, writing 0");
                0
            }
        },
        wall_s: wall,
    };
    let path = options.dir.join(options.set.json);
    fs::create_dir_all(&options.dir)?;
    fs::write(&path, report.json(&meta, &options.set))?;
    println!("written to {}", path.display());
    Ok(())
}

/// The `wall_s` and `threads` of the last row of the arena CSV for this match, if there is one.
fn match_row(csv: &str, version: &str, a: &str, b: &str, games: usize) -> Option<(f64, usize)> {
    csv.lines().rev().find_map(|line| {
        let f: Vec<&str> = line.split(',').collect();
        let matches = f.len() > 25
            && f[1] == version
            && f[3] == a
            && f[4] == b
            && f[5] == games.to_string()
            && f[22] == "paired";
        matches
            .then(|| Some((f[25].parse().ok()?, f[24].parse().ok()?)))
            .flatten()
    })
}
