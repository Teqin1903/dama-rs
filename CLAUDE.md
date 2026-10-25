# Turkish draughts agent: Rust version

This repository is our draughts agent, written from scratch in Rust. Our first agent was in C++ and is not
in this repository. The executable reference here is the JavaScript prototype in `reference/js/`: its rules
match kish 1.1.2, and every number in our Assignment 3 talk came from it.

## Layout
- `engine/`: library crate with the board, move generation, evaluation (U) and search
- `cli/`: binary crate that speaks the text protocol (position / go / bestmove)
- `reference/js/`: the JS prototype. Read it; never edit it.
- `testdata/`: perft counts, rule cases, move lists of sampled positions, decisions of the baseline agent
- `docs/reference.md`: board and move formats, the baseline agent's U, our match settings
- `docs/decisions.md`: changes we make on purpose after the baseline, with how we measured them
- `docs/benchmarks.md`: measured speed against the JS prototype (perft), with the machine and the method
- `bench/`: timing scripts for the JS prototype; the Rust timer is `engine/examples/perft.rs`
- `results/arena.csv`: one row per `dama arena` run; its version column ignores changes under `results/`

## Commands
- Build: `cargo build --release`
- Fast tests: `cargo test`
- Slow tests (perft 7-8 and others marked `#[ignore]`): `cargo test --release -- --ignored`
- Lint: `cargo clippy --all-targets -- -D warnings`
- Format: `cargo fmt`
- Arena: `cargo run --release -p cli -- arena tuned random --games 200 --seed 1` (`--help` lists the agents)
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
- Search is one ply for now, like the baseline. Keep it behind its own module so minimax can come later.
- Do not add Co-Authored-By trailers or "Generated with" lines to commits or pull requests.

## Talking to me
- Talk to me in Turkish; write code, comments and commit messages in English.
- I am new to Rust and know C++. When you use a Rust idea for the first time (ownership, borrowing,
  enums with data, match, traits, iterators, Result), explain it in two or three sentences and compare it with C++.
