# Turkish draughts agent: Rust version

This repository is our draughts agent, written from scratch in Rust. Our first agent was in C++ and is not
in this repository. The executable reference here is the JavaScript prototype in `reference/js/`: its rules
match kish 1.1.2, and every number in our Assignment 3 talk came from it.

## Layout
- `engine/`: library crate with the board, move generation, evaluation (U and its positional features, `eval.rs`),
  search, Monte Carlo tree search (`mcts.rs`), Zobrist keys (`zobrist.rs`) and the transposition table (`table.rs`), the arena and its SPRT (`sprt.rs`),
  Texel tuning of the weights (`texel.rs`), and the Elo ladder (`ladder.rs`: rungs, pairings, game rows) with its
  Bradley-Terry fit and bootstrap (`elo.rs`), and the web API (`api.rs`: `Session`, JSON in and out, docs/web-api.md)
  with its search tracer (`trace.rs`: `Tracer`, `NoTrace`, `Recorder`) and a small JSON reader and writer (`json.rs`)
- `cli/`: binary crate that speaks the text protocol (position / go / bestmove)
- `web/`: the engine for the site in WebAssembly (`wasm-bindgen`, `console_error_panic_hook`: used only here); a thin
  wrapper of `engine::Session`
- `site/` and `deck/` (the talk's slide deck) are written by someone else: we only produce `site/pkg/`
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
- Search speed and depth (541 fixed positions, pin with `taskset -c 2`): `target/release/examples/search ab6o ab6ot`
  (nodes and time per move), `target/release/examples/depth idqrot:n20000:w=texel1` (depth reached within a budget, and
  how many positions stopped by proof), `target/release/examples/aspiration idqrotf:n20000:w=texel1:fs=1 0.25 0.5 1`
  (how far the value moves between iterations, how often a window of each W would hold),
  `target/release/examples/forced games.tsv` (from a `dama arena --log`: how many positions have a single legal move and
  of what kind, and positions where `abNx` and `abN` decide differently)
- SPRT arena: `cargo run --release -p cli -- arena idqr:n2000:adv=0.1 idqr:n2000 --sprt --threads 6` stops when the
  test decides (defaults `--elo0 0 --elo1 20 --alpha 0.05 --beta 0.05`, at most `--games 2000`, no verdict before
  `--min-games 100`; paired schedule only). Use it for comparisons; fixed `--games N` stays for measurements
- Texel tuning: `cargo run --release -p cli -- texel --seed 1 --threads 6` plays a paired round robin (default
  idqr:n2000, idq:n2000, ab3q, tuned, 500 games each), fits K and then the weights with man = 1 on the quiet
  positions, and prints the errors on training and validation (every fifth pair) and the weights. The data set
  is not stored; the command and seed make it again. `texel1` in `eval.rs` came from this run; it beats the tuned
  weights clearly (SPRT H1 at n2000 and n20000, docs/decisions.md, with an ablation of each weight).
  `--agents`, `--games`, `--start W` (as in an agent name, `w=texel1:qmob=0`), `--fixed KEYS` (left at the start
  value besides man) and `--compare NAMES` set up other runs; the split is by whole games and is reported.
  `texel2` and `texel2s` came from the second run (seed 2, 6000 games; the commands are in docs/decisions.md)
- Elo ladder (docs/ladder.md): `cargo run --release -p cli -- ladder run --estimate` (12 games per pairing, prints
  the time of the whole run), `ladder run` (25 pairings, 6800 games; resumable: one file per finished pairing in
  `results/ladder/games/`, skipped on a rerun; `results/ladder/progress.log` is not committed), `ladder rate`
  (writes `results/ladder/ladder.json` from the games, plays nothing). Rungs and pairings are `LADDER` and `plan`
  in `engine/src/ladder.rs`; the zero is V1 (`mm3`), the mid opponent V6 (`idqr:n20000`)
- Ablation (docs/ladder.md): `ladder run --set ablation` (and `--estimate`, `rate --set ablation`): V10 against
  itself with one part off (`ABLATION`: q, r, o, t, the Texel weights) and against V0, 400 games each, into
  `results/ablation/` and `ablation.json`; V10–V9 (no table) is the ladder's pairing, read from `results/ladder/games/`
- Monte Carlo tree search (docs/ladder.md): `ladder run --set mcts` (and `--estimate`, `rate --set mcts`): the four
  forms (`MCTS`: `mc`, `uct`, `ucte`, `uctq`, 20000 nodes per move) against V0, V2 and V10 (`MCTS_OPPONENTS`), 200
  games each, into `results/mcts/` and `mcts.json`
- Draw rules in the search (docs/ladder.md): `ladder run --set fifty` (and `--estimate`, `rate --set fifty`): V10f
  (`idqrotf:n20000:w=texel1`) and V10fs (`…:fs=1`) against V10 and V0 (`FIFTY_PAIRS`), 400 games each, into
  `results/fifty/` and `fifty.json`
- Web API: `scripts/build-web.sh` builds `site/pkg/` (wasm-pack, keeping only `dama_web.js` and
  `dama_web_bg.wasm`); `target/release/dama api < requests.jsonl` answers the same calls natively, one JSON
  request per line (`{"call": "think", "args": ["white", "{}"]}`). Tracing must never change a search: the searches take a generic
  `Tracer` (`*_traced` in search.rs; the old functions pass `NoTrace`), and `tests/api.rs` checks traced against
  untraced searches and a session's games against the arena's
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
- Before starting a measurement (arena, SPRT, texel), estimate its worst-case time: the most games it can play
  (`--games`, the SPRT maximum) times the time per game from earlier `wall_s` rows of `results/arena.csv`. If that is
  more than 15 minutes, ask me first.
- Ask before adding a crate. `kish` may be used only as a dev-dependency, in tests.
- Every commit that changes engine code (`engine/`, `web/`) rebuilds `site/pkg/` with `scripts/build-web.sh` and
  commits `dama_web.js` and `dama_web_bg.wasm` with it. The site's top bar shows `version()`, so a stale pkg shows
  there. `version()` is `src.` and the git tree of the WebAssembly sources (`SOURCES` in `web/build.rs`: the
  `src/` folders and `Cargo.toml`s of engine and web, `web/build.rs`, the root `Cargo.toml` and `Cargo.lock`), and
  `web/tests/fresh.rs` fails with "site/pkg is stale" until the pkg holds the source's version.
- The repository is the single source: everything the talk and the submission zip use (code, results, docs, the
  site, the deck) is in git. Not `target/`, `results/ladder/progress.log`, or intermediate data over 50 MB; if such
  data comes up, ask me first.
- Keep the binaries thin; logic lives in `engine/`.
- Search lives in its own module: one ply for the baseline (`tuned`), depth-limited minimax for `mmN`,
  alpha-beta for `abN` (the same decisions as `mmN` at the same depth, from fewer nodes; legal move order unless
  the name has `o`), and iterative deepening for `id:dN:nK:tMS` (alpha-beta 1, 2, 3, ... plies within a per-move budget of
  depth, nodes and/or milliseconds; the deepest completed iteration decides, so `id:dN` plays as `abN`;
  official comparisons use node budgets), and quiescence for `abNq` and `idq:...` (past the depth limit,
  captures only until the position is quiet; no stand pat, since capturing is mandatory; its nodes count
  against the budget), and repetitions for `abNr`, `abNqr`, `idr:...` and `idqr:...` (a position of the game or of
the search path that occurs again is a draw, 0; agents learn the game through `Agent::observe`), and move ordering
for a final `o` (`ab6o`, `idqro:...`: the previous iteration's best move at the root, then promotions, killers,
history; the same decisions at a fixed depth, so deeper within a budget), and a transposition table for a final `t`
(`ab6ot`, `idqrot:n20000`), and the game's other draw rules for a final `f` (`idqrotf:n20000`: a position of the
search after 50 quiet plies, counted from the game's count from `Agent::observe`, or with one piece each, is a draw;
`:fs=1` adds progress pressure, U scaled by (100 − quiet plies) / 100 at the depth limit, with the count in the table's
key), and principal variation search for a final `p` (`ab6otp`, `idqrotfp:n20000`: all moves of a node but the first are
tested with an empty window, those that land inside it searched again; the same decisions at a fixed depth), and the single-reply extension for a
final `x` (`idqrotfx:n20000`: below the root, above the depth limit, a single legal move that is a capture spends no ply;
only a capture, for the reasons in search.rs; a line holds at most 31 captures without `f`, 30 with it; this one changes the
decisions, its referee is a minimax in tests/extension.rs) (letters always in the order q, r, o, t, f, p, x). Iterative deepening may have aspiration windows, `:asp=W` (`idqrotf:n20000:asp=0.5`:
from depth 5, unless the last value is a win or a loss, the window is W around the last value in units of U, the side that
fails widens W, 4W, open; the same decisions at the depth completed, tries count against the budget). The table belongs to the agent, is cleared at
every game, is 16 MB unless `:hash=MB`, and does not keep a value that scored a repetition below it; `:tt=strict` is
the referee's mode (same draft only and, with repetitions, only where the value cannot depend on the path), whose
games equal those without a table: check new table ideas against it. Keep new search ideas there, behind the
`Agent` trait. Monte Carlo tree search is its own module, `mcts.rs`: `mc:nK` (flat), `uct:nK` (random games),
`ucte:nK` (sigmoid(K U)) and `uctq:nK` (sigmoid(K quiescence)), budgets in nodes and/or time but not depth, counted
as alpha-beta counts them (every position made with `apply`, in random games and quiescence too). Random games end
by the arena's rules without the third repetition; the game's ply and quiet plies come from `Agent::observe`.
- Evaluation lives in `eval.rs`: U (`man`, `king`, `mob`) plus the positional features `adv`, `back`, `cen`, `edge`,
  `qmob` (defined there), whose weights are 0 unless set. Any agent name but `random` may end in `:KEY=VALUE` parts
  (`idqr:n2000:adv=0.1:king=3`, `tuned:cen=0.05`) on top of the tuned or named weights; a first part `w=NAME`
  starts from a named set of `Weights::NAMED` (material, first, tuned, texel1, texel2,
  texel2s): `idqr:n2000:w=texel1:adv=0`.
  With every positional weight 0, `evaluate` is U bit for bit: keep it so, and check new features for colour symmetry (`tests/eval_features.rs`).
- Do not add Co-Authored-By trailers or "Generated with" lines to commits or pull requests.

## Talking to me
- Talk to me in Turkish; write code, comments and commit messages in English.
- I am new to Rust and know C++. When you use a Rust idea for the first time (ownership, borrowing,
  enums with data, match, traits, iterators, Result), explain it in two or three sentences and compare it with C++.
