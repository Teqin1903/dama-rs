# Turkish draughts agent: Rust version

This repository is our draughts agent, written from scratch in Rust. Our first agent was in C++ and is not
in this repository. The executable reference here is the JavaScript prototype in `reference/js/`: its rules
match kish 1.1.2, and every number in our Assignment 3 talk came from it.

## Layout
- `engine/`: library crate with the board, move generation, evaluation (U and its positional features, `eval.rs`),
  search, the arena and its SPRT (`sprt.rs`), and Texel tuning of the weights (`texel.rs`)
- `cli/`: binary crate that speaks the text protocol (position / go / bestmove)
- `reference/js/`: the JS prototype. Read it; never edit it.
- `testdata/`: perft counts, rule cases, move lists of sampled positions, decisions of the baseline agent
- `docs/reference.md`: board and move formats, the baseline agent's U, our match settings
- `docs/decisions.md`: changes we make on purpose after the baseline, with how we measured them
- `docs/benchmarks.md`: measured speed against the JS prototype (perft), with the machine and the method
- `bench/`: timing scripts for the JS prototype (the Rust timer is `engine/examples/perft.rs`), and
  `playgame_log.js`, which checks that the arena plays the same games as `playGame`
- `results/arena.csv`: one row per `dama arena` run; its version column ignores changes under `results/`; the
  `schedule`, `opening_plies`, `threads` and `wall_s` columns say how the match was played (rows before them are
  `legacy,2,1`), and the time per move is empty when more than one thread played; `sprt` (H0, H1 or none), `llr`,
  `elo0`, `elo1`, `alpha`, `beta`, `max_games`, `min_games` are empty unless the match ran with `--sprt`, and `games`
  is the number of games played

## Commands
- Build: `cargo build --release`
- Fast tests: `cargo test`
- Slow tests (perft 7-8 and others marked `#[ignore]`): `cargo test --release -- --ignored`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Format: `cargo fmt`
- Arena: `cargo run --release -p cli -- arena tuned random --games 200 --seed 1 --threads 6` (`--help` lists the
  agents). The default `paired` schedule gives every game its own seeds, 4 random opening plies, each opening twice
  with the colours swapped; `--schedule legacy` repeats the old rows and `bench/playgame_log.js`. The games are the
  same with any `--threads`; measure time per move with `--threads 1` (pinned with `taskset`)
- SPRT arena: `cargo run --release -p cli -- arena idqr:n2000:adv=0.1 idqr:n2000 --sprt --threads 6` stops when the
  test decides (defaults `--elo0 0 --elo1 20 --alpha 0.05 --beta 0.05`, at most `--games 2000`, no verdict before
  `--min-games 100`; paired schedule only). Use it for comparisons; fixed `--games N` stays for measurements
- Texel tuning: `cargo run --release -p cli -- texel --seed 1 --threads 6` plays a paired round robin (default
  idqr:n2000, idq:n2000, ab3q, tuned, 500 games each), fits K and then the weights with man = 1 on the quiet
  positions, and prints the errors on training and validation (every fifth pair) and the weights. The data set
  is not stored; the command and seed make it again. `texel1` in `eval.rs` came from this run
- Regenerate testdata (only when I ask): `node reference/js/make_testdata.js`

## Rules (TÜDAF): the parts that are easy to get wrong
- Men move and capture one square forward or sideways; never backward, never diagonally.
- Kings fly orthogonally and may land on any empty square beyond the captured piece.
- Capturing is mandatory, and a sequence that captures the most pieces must be played.
- Captured pieces are removed immediately, during the sequence.
- No 180-degree turn between two captures.
- A man that reaches the last row during a capture continues as a man and is crowned at the end of the move.
- A player with no legal move loses.

## Referees
1. Perft from the start, depths 1-8 (kish 1.1.2): 8, 64, 708, 7538, 85090, 931312, 10782382, 123290300.
   A position with no legal move counts as one leaf.
2. `testdata/rules.json` and `testdata/movegen.json`: our legal move list must equal the expected list exactly.
3. `testdata/decisions.json`: with the given weights, our features (men, kings, mob, win) must match for every
   move, and the agent's move must be one of `best`.
Never change an expected value to make a test pass: find the bug and explain which rule it broke.
`reference/` and `testdata/` are write-protected for you in `.claude/settings.json`.

## Baseline first, then our changes
- First reach parity with the prototype: all referees green. Then I tag the commit `baseline`.
- After that we change the agent on purpose. Every change gets a line in `docs/decisions.md`: what, why,
  and how we measured it (games against the baseline, fixed seeds).

## How we work
- One module per session: plan first, then implement, run the tests, then commit (ask me first).
- Ask before adding a crate. `kish` may be used only as a dev-dependency, in tests.
- Keep the binaries thin; logic lives in `engine/`.
- Search lives in its own module: one ply for the baseline (`tuned`), depth-limited minimax for `mmN`,
  alpha-beta for `abN` (the same decisions as `mmN` at the same depth, from fewer nodes; no move ordering
  yet), and iterative deepening for `id:dN:nK:tMS` (alpha-beta 1, 2, 3, ... plies within a per-move budget of
  depth, nodes and/or milliseconds; the deepest completed iteration decides, so `id:dN` plays as `abN`;
  official comparisons use node budgets), and quiescence for `abNq` and `idq:...` (past the depth limit,
  captures only until the position is quiet; no stand pat, since capturing is mandatory; its nodes count
  against the budget), and repetitions for `abNr`, `abNqr`, `idr:...` and `idqr:...` (a position of the game or of
the search path that occurs again is a draw, 0; agents learn the game through `Agent::observe`). Keep new search
ideas there, behind the `Agent` trait.
- Evaluation lives in `eval.rs`: U (`man`, `king`, `mob`) plus the positional features `adv`, `back`, `cen`, `edge`,
  `qmob` (defined there), whose weights are 0 unless set. Any agent name but `random` may end in `:KEY=VALUE` parts
  (`idqr:n2000:adv=0.1:king=3`, `tuned:cen=0.05`) on top of the tuned or named weights; a first part `w=NAME`
  starts from a named set of `Weights::NAMED` (material, first, tuned, texel1): `idqr:n2000:w=texel1:adv=0`.
  With every positional weight 0, `evaluate` is U bit for bit: keep it so, and check new features for colour symmetry (`tests/eval_features.rs`).
- Do not add Co-Authored-By trailers or "Generated with" lines to commits or pull requests.

## Talking to me
- Talk to me in Turkish; write code, comments and commit messages in English.
- I am new to Rust and know C++. When you use a Rust idea for the first time (ownership, borrowing,
  enums with data, match, traits, iterators, Result), explain it in two or three sentences and compare it with C++.
