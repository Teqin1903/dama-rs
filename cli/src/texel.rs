//! `dama texel`: plays a round robin between agents, takes the quiet positions of the games
//! labelled with their results, and fits the weights of the evaluation to them (engine/src/texel.rs).
//! Nothing is written to disk: the command and its seed give the same data and weights again.

use std::error::Error;
use std::process::ExitCode;
use std::time::Instant;

use engine::{
    AgentSpec, DataSet, MatchPlan, Schedule, Weights, derive_seed, fit_k, mse, play_match, tune,
};

const HELP: &str = "\
usage: dama texel [--agents A,B,...] [--games N] [--seed S] [--threads T] [--skip P]
                  [--start W] [--fixed K,...] [--compare W,...]

Plays a paired match of N games between every two of the agents (match i of the round robin with
the seed derived from S and i), takes the positions of every game from ply P on in which the side
to move has no capture and both sides have a legal move, each labelled with the game's result for
White (1, 1/2, 0), and puts every fifth pair of games aside for validation, whole games only. Then
finds the K for which sigmoid(K * value) with the start weights fits the results best (mean
squared error), and with that K and man and the fixed weights left alone, tunes the other weights
from the start ones. Prints the split, the errors on both sets of the compared weights, the start
weights and the weights found (at that K and at each one's own best K), and the weights found.

options:
  --agents A,B,...  the agents, separated by ',' (default idqr:n2000,idq:n2000,ab3q,tuned)
  --games N         games per match, even (default 500)
  --seed S          the seed, 0 to 4294967295 (default 1)
  --threads T       play T games at the same time (default 1); the games are the same with any T
  --skip P          leave out the first P plies of every game (default 8: the 4 random opening
                    plies and the first 4 moves of the agents)
  --start W         the weights to start from, as in an agent name: KEY=VALUE parts separated by
                    ':' on top of the tuned weights, the first part may be w=NAME (default: tuned;
                    for example w=texel1:qmob=0)
  --fixed K,...     weights left at their start value besides man (default: none)
  --compare W,...   named weight sets whose errors are printed too (default tuned,texel1)
";

/// The stream of `derive_seed` for the seeds of the round robin's matches.
const MATCH_SEEDS: u32 = 0x7e3e1;

#[derive(Debug, PartialEq)]
struct Args {
    agents: Vec<AgentSpec>,
    games: u32,
    seed: u32,
    threads: usize,
    skip: usize,
    start: Weights,
    /// The weights `tune` leaves alone: `man` and those of `--fixed`.
    fixed: Vec<&'static str>,
    compare: Vec<(String, Weights)>,
}

/// The key of `Weights::KEYS` spelled `key`.
fn weight_key(key: &str) -> Option<&'static str> {
    Weights::KEYS.into_iter().find(|&k| k == key)
}

/// Runs `dama texel` with the arguments that follow `texel`.
pub fn main(args: impl Iterator<Item = String>) -> ExitCode {
    let args = match parse_args(args) {
        Ok(Some(args)) => args,
        Ok(None) => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("dama texel: {message}\n\n{HELP}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("dama texel: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The arguments, or `None` if they ask for help.
fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<Args>, String> {
    let mut parsed = Args {
        agents: ["idqr:n2000", "idq:n2000", "ab3q", "tuned"]
            .map(|name| name.parse().expect("a valid agent"))
            .to_vec(),
        games: 500,
        seed: 1,
        threads: 1,
        skip: 8,
        start: Weights::TUNED,
        fixed: vec!["man"],
        compare: ["tuned", "texel1"]
            .map(|name| (name.to_string(), Weights::named(name).expect("a named set")))
            .to_vec(),
    };
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "-h" || arg == "--help" {
            return Ok(None);
        }
        let value = args.next().ok_or(format!("{arg} needs a value"))?;
        let bad = || format!("{arg} {value}: expected a positive whole number");
        match arg.as_str() {
            "--agents" => {
                parsed.agents = value
                    .split(',')
                    .map(|name| name.parse::<AgentSpec>().map_err(|e| e.to_string()))
                    .collect::<Result<_, _>>()?;
            }
            "--games" => {
                parsed.games = value
                    .parse()
                    .ok()
                    .filter(|&n: &u32| n > 0 && n.is_multiple_of(2))
                    .ok_or(format!("--games {value}: expected a positive even number"))?;
            }
            "--seed" => {
                parsed.seed = value
                    .parse()
                    .map_err(|_| format!("--seed {value}: expected 0 to 4294967295"))?;
            }
            "--threads" => {
                parsed.threads = value.parse().ok().filter(|&t| t > 0).ok_or_else(bad)?
            }
            "--skip" => parsed.skip = value.parse().map_err(|_| bad())?,
            "--start" => {
                parsed.start = Weights::TUNED
                    .with_overrides(&value)
                    .map_err(|e| format!("--start {value}: {e}"))?;
            }
            "--fixed" => {
                parsed.fixed = vec!["man"];
                for key in value.split(',') {
                    let key = weight_key(key).ok_or(format!(
                        "--fixed {value}: {key:?} is not one of {}",
                        Weights::KEYS.join(", ")
                    ))?;
                    if !parsed.fixed.contains(&key) {
                        parsed.fixed.push(key);
                    }
                }
            }
            "--compare" => {
                parsed.compare = value
                    .split(',')
                    .map(|name| {
                        Weights::named(name)
                            .map(|w| (name.to_string(), w))
                            .ok_or(format!("--compare {value}: no weight set {name:?}"))
                    })
                    .collect::<Result<_, _>>()?;
            }
            _ => return Err(format!("unknown option {arg}")),
        }
    }
    if parsed.agents.len() < 2 {
        return Err("--agents: expected at least two agents".to_string());
    }
    Ok(Some(parsed))
}

fn run(args: &Args) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    let mut data = DataSet::default();
    let mut pairings = Vec::new();
    for (i, a) in args.agents.iter().enumerate() {
        for b in &args.agents[i + 1..] {
            pairings.push((a, b));
        }
    }
    println!(
        "{} matches of {} games, seed {}, {} threads, positions from ply {}",
        pairings.len(),
        args.games,
        args.seed,
        args.threads,
        args.skip
    );
    for (m, (a, b)) in pairings.iter().enumerate() {
        let plan = MatchPlan {
            games: args.games,
            seed: derive_seed(args.seed, MATCH_SEEDS, m as u32),
            schedule: Schedule::Paired,
            threads: args.threads,
            sprt: None,
        };
        let (make_a, make_b) = (|| a.build(), || b.build());
        let match_started = Instant::now();
        let record = play_match(&make_a, &make_b, &plan)?;
        let (wins, draws, losses) = record.wins_draws_losses();
        let before = data.train.len() + data.validation.len();
        data.add_match(record.games.iter().map(|game| &game.record), args.skip);
        println!(
            "  {a} vs {b} (seed {}): +{wins} ={draws} -{losses}, {:.1} plies per game, \
             {} positions, {:.1} s",
            plan.seed,
            record.mean_plies().unwrap_or(0.0),
            data.train.len() + data.validation.len() - before,
            match_started.elapsed().as_secs_f64()
        );
    }
    let games_s = started.elapsed().as_secs_f64();
    let [train_games, validation_games] = data.games;
    let (train, validation) = (&data.train, &data.validation);
    println!(
        "split by whole games (every fifth pair, both games of a pair together): \
         training {train_games} games ({} pairs), {} positions; validation {validation_games} \
         games ({} pairs), {} positions; games took {games_s:.1} s",
        train_games / 2,
        train.len(),
        validation_games / 2,
        validation.len()
    );
    println!(
        "  of the {} distinct validation positions, {} come up in a training game too",
        data.validation_positions(),
        data.shared()
    );
    if train.is_empty() || validation.is_empty() {
        return Err("no positions to fit".into());
    }

    let fit_started = Instant::now();
    let start = args.start;
    let k = fit_k(train, &start);
    let tuning = tune(train, &start, k, &args.fixed);
    let w = tuning.weights;
    println!(
        "K = {k:.4} (fitted to the training set with the start weights); fixed: {}; \
         {} errors computed, {:.1} s",
        args.fixed.join(", "),
        tuning.evaluations,
        fit_started.elapsed().as_secs_f64()
    );
    let mut sets: Vec<(&str, Weights)> = args
        .compare
        .iter()
        .map(|(name, weights)| (name.as_str(), *weights))
        .collect();
    sets.push(("start", start));
    sets.push(("tuned by Texel", w));
    println!("mean squared error   training   validation   own K   training   validation");
    for (name, weights) in sets {
        let own = fit_k(train, &weights);
        println!(
            "  {name:<17}  {:.6}   {:.6}     {own:.4}  {:.6}   {:.6}",
            mse(train, &weights, k),
            mse(validation, &weights, k),
            mse(train, &weights, own),
            mse(validation, &weights, own)
        );
    }
    println!("weights:");
    for key in Weights::KEYS {
        println!(
            "  {key:<4} {:>8} -> {}",
            start.get(key).expect("a key"),
            w.get(key).expect("a key")
        );
    }
    println!("as an agent name: idqr:n2000{}", w.suffix());
    println!("total {:.1} s", started.elapsed().as_secs_f64());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Option<Args>, String> {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_and_options() {
        let args = parse(&[]).unwrap().unwrap();
        let names: Vec<String> = args.agents.iter().map(|a| a.to_string()).collect();
        assert_eq!(names, ["idqr:n2000", "idq:n2000", "ab3q", "tuned"]);
        assert_eq!(
            (args.games, args.seed, args.threads, args.skip),
            (500, 1, 1, 8)
        );
        let args = parse(&[
            "--agents",
            "ab2,random",
            "--games",
            "10",
            "--seed",
            "7",
            "--threads",
            "6",
            "--skip",
            "0",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(args.agents.len(), 2);
        assert_eq!(
            (args.games, args.seed, args.threads, args.skip),
            (10, 7, 6, 0)
        );
        assert_eq!(parse(&["--help"]), Ok(None));
    }

    #[test]
    fn start_fixed_and_compare() {
        let args = parse(&[]).unwrap().unwrap();
        assert_eq!(args.start, Weights::TUNED);
        assert_eq!(args.fixed, ["man"]);
        let names: Vec<&str> = args.compare.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["tuned", "texel1"]);
        let args = parse(&[
            "--start",
            "w=texel1:qmob=0",
            "--fixed",
            "qmob,back,qmob",
            "--compare",
            "material",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(
            args.start,
            Weights {
                qmob: 0.0,
                ..Weights::TEXEL1
            }
        );
        assert_eq!(args.fixed, ["man", "qmob", "back"]);
        assert_eq!(args.compare, [("material".to_string(), Weights::MATERIAL)]);
        assert_eq!(
            parse(&["--start", "adv=0.1"]).unwrap().unwrap().start.adv,
            0.1
        );
    }

    #[test]
    fn bad_options() {
        for args in [
            &["--games", "11"][..],
            &["--games", "0"],
            &["--threads", "0"],
            &["--agents", "tuned"],
            &["--agents", "tuned,nobody"],
            &["--skip"],
            &["--speed", "1"],
            &["--start", "w=nobody"],
            &["--start", "adv"],
            &["--fixed", "nobody"],
            &["--fixed", "man,"],
            &["--compare", "tuned,nobody"],
        ] {
            assert!(parse(args).is_err(), "{args:?}");
        }
    }
}
