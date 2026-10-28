# Reference: formats and the baseline agent

## Board string
- Eight ranks separated by `/`, rank 8 first; within a rank, files a to h.
- `w` white man, `W` white king, `b` black man, `B` black king, `.` empty square.
- Start position, White to move: `......../bbbbbbbb/bbbbbbbb/......../......../wwwwwwww/wwwwwwww/........`
- White moves up the board (towards rank 8), Black moves down.
- The JS prototype indexes squares as `rank * 8 + file` (a1 = 0, h1 = 7, a8 = 56). The Rust code may use any
  representation inside; only these text formats must match.

## Moves
- Non-capture: `from-to`, for example `d3-d4`.
- Capture: every square the piece lands on, joined by `x`, for example `a6xa2xf2xf5xc5`. Captured squares are
  not written: each one is the single enemy piece between two consecutive landing squares.
- Promotion: suffix `=K`, for example `a4xa6xa8=K`.
- The test files list legal moves sorted as plain strings.

## testdata/
- `perft.json`: counts from the start position. A position with no legal move counts as one leaf.
- `rules.json`: one case per rule; `moves` is the complete legal move list.
- `movegen.json`: 400 positions from seeded random games, tagged by kind (multiCapture, kingCapture,
  promotion, king, quiet); `moves` is the complete legal move list.
- `decisions.json`: 141 positions (the slide-7 position from our talk first). For each legal move: the integer
  features `men`, `kings`, `mob`, the `win` flag and `u`. `best` lists every move with the highest U; the
  prototype breaks ties at random, so any of them is correct.

## Baseline agent (`reference/js/agent.js`)
For every legal move, look at the board after the move (the opponent is to move):
- If the opponent has no legal move, U = 1000 (a win).
- Otherwise U = man × (our men − their men) + king × (our kings − their kings) + mob × (our legal moves − their
  legal moves). Legal moves are counted on that board with the full rules (forced and longest captures).
  Compute it in this order with f64 and compare U values within 1e-9.
- Play a move with the highest U.
- Weights: tuned man 1, king 2.5, mob −0.05. First guess man 1, king 3, mob +0.1.

## Match settings used in our experiments (`playGame` in `reference/js/agent.js`)
- A side with no legal move loses.
- Draw by threefold repetition, after 50 plies without a capture, when each side has one piece left, or at ply 400.
- Weight-landscape experiments: 200 games per pairing, colours alternate, 2 random opening plies,
  the same seeds for every pairing.
- `dama arena` has two schedules (seeds in `engine/src/arena.rs`). `legacy` is the setting above, with one
  generator per match: every row of `results/arena.csv` before the `schedule` column, and the games of
  `bench/playgame_log.js`. `paired` (the default from then on): every game gets its own seeds from (seed, game),
  4 random opening plies, and each opening is played twice with the colours swapped. Either can run on several
  threads (`--threads`) with the same games; times per move count only from `--threads 1`.
