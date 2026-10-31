# Benchmarks

## Perft from the start position

Time per perft run from the start position, White to move: the Rust engine (`engine/`, release build) and
the JS prototype (`perft` in `reference/js/engine.js`) on the same machine. Measured on 2026-10-25 with the
code of the commit that adds this table.

| Depth | Leaves | Rust | JS | JS ÷ Rust | Rust (M leaves/s) | JS (M leaves/s) |
|---:|---:|---:|---:|---:|---:|---:|
| 1 | 8 | 0.42 µs | 3.39 µs | 8.0 | 19.0 | 2.4 |
| 2 | 64 | 3.95 µs | 31.9 µs | 8.1 | 16.2 | 2.0 |
| 3 | 708 | 39.0 µs | 260 µs | 6.7 | 18.2 | 2.7 |
| 4 | 7,538 | 423 µs | 2.77 ms | 6.5 | 17.8 | 2.7 |
| 5 | 85,090 | 4.84 ms | 29.5 ms | 6.1 | 17.6 | 2.9 |
| 6 | 931,312 | 56.4 ms | 323 ms | 5.7 | 16.5 | 2.9 |
| 7 | 10,782,382 | 659 ms | 3.50 s | 5.3 | 16.4 | 3.1 |
| 8 | 123,290,300 | 7.80 s | 39.5 s | 5.1 | 15.8 | 3.1 |

- Both engines do the same work. They count the last ply without playing it (bulk counting) and count a
  position without legal moves as one leaf, and both report the leaf counts of `testdata/perft.json`.
- The table compares two implementations, not two languages: the Rust engine uses bitboards, the JS
  prototype a 64-square array, and V8 compiles the JS to machine code as well.
- The ratio falls from about 8 at depths 1-2 to 5 at depth 8. One known cost on the Rust side: every
  generated move still allocates a `Vec` for its path. Keeping the path in a fixed-size array is a
  candidate for a measured change after the baseline.

### Machine and method

- CPU: Intel Core i7-9750H (laptop), both programs pinned to CPU 2 with `taskset -c 2`; frequency
  governor `powersave` (intel_pstate); load average about 0.5 during the runs.
- OS: CachyOS, Linux 7.2.9-1-cachyos.
- Rust: rustc 1.99.1, `cargo build --release` with the default release profile.
- JS: node v26.10.0.
- Method, the same in `engine/examples/perft.rs` and `bench/perft.js`:
  - one warm-up run of depth 5;
  - a sample is k back-to-back runs; k starts at 1 and doubles until a sample lasts at least 50 ms, and
    that sample counts as the first one;
  - 5 samples for depths 1-6 and 3 for depths 7-8; the time per run is the median sample divided by k.
- Rust ran first (from 16:55), then JS (16:56 to 16:58), UTC+3.

### Reproduce

```
cargo build --release --example perft
taskset -c 2 target/release/examples/perft 8
taskset -c 2 node bench/perft.js 8
```

### Raw output

Rust (`target/release/examples/perft 8`):

```
depth	leaves	ns_per_run	k	samples
1	8	422	131072	5
2	64	3946	16384	5
3	708	38980	2048	5
4	7538	422866	128	5
5	85090	4840919	16	5
6	931312	56385041	1	5
7	10782382	658754749	1	3
8	123290300	7798511672	1	3
```

JS (`node bench/perft.js 8`):

```
depth	leaves	ns_per_run	k	samples
1	8	3386	16384	5
2	64	31891	2048	5
3	708	260250	256	5
4	7538	2767001	32	5
5	85090	29499148	2	5
6	931312	322657713	1	5
7	10782382	3495450120	1	3
8	123290300	39503205019	1	3
```

## Search for one minute

How far the current engine, V10 (`idqrot:w=texel1`, 16 MB table), gets in one search of 60 seconds:
ten positions of `testdata/decisions.json`, each searched once on an empty game with an empty table,
as a first move would be. Measured on 2026-11-01 with commit c25cf42 (`mcts-1-4-gc25cf42`); the
numbers are in `results/minute/minute.json`, which the site and the deck read.

| Position | Phase | Pieces (kings) | Nodes | Depth | Nodes/s |
|---|---|---:|---:|---:|---:|
| agent_g12_ply0 | start | 32 (0) | 27,776,525 | 11 | 462,942 |
| agent_g0_ply4 | opening | 32 (0) | 27,610,813 | 10 | 460,180 |
| agent_g3_ply9 | opening | 28 (1) | 28,144,314 | 11 | 469,072 |
| agent_g16_ply18 | middlegame | 24 (1) | 26,407,526 | 10 | 440,125 |
| agent_g12_ply40 | middlegame | 23 (1) | 27,692,542 | 10 | 461,542 |
| agent_g36_ply28 | middlegame | 21 (1) | 28,519,748 | 10 | 475,329 |
| agent_g8_ply52 | endgame | 15 (1) | 48,164,269 | 13 | 802,738 |
| agent_g17_ply89 | endgame | 13 (0) | 44,427,922 | 12 | 740,465 |
| agent_g41_ply101 | endgame | 9 (2) | 33,810,819 | 10 | 563,514 |
| agent_g57_ply49 | endgame | 7 (1) | 55,440,248 | 16 | 924,004 |
| **median** | | | **28,332,031** | **10.5** | **472,201** |

With a 256 MB table (`:hash=256`) on three of them:

| Position | Nodes (16 MB → 256 MB) | Depth | Nodes/s |
|---|---:|---:|---:|
| agent_g12_ply0 | 27,776,525 → 26,934,459 | 11 → 12 | 462,942 → 448,908 |
| agent_g12_ply40 | 27,692,542 → 25,743,941 | 10 → 10 | 461,542 → 429,066 |
| agent_g41_ply101 | 33,810,819 → 37,475,539 | 10 → 11 | 563,514 → 624,592 |

- Every search used its full minute (60.0 s); none reached the deepest iteration, `MAX_ID_DEPTH` = 64
  (`depth_limit_hit` is false everywhere). Depth is the deepest completed iteration; the interrupted one is
  thrown away, but its nodes are counted.
- In the opening and the middlegame the engine makes about 0.46 M nodes per second and completes 10 to
  11 plies. With fewer pieces a node is cheaper (fewer moves to generate, fewer captures for quiescence):
  up to 0.92 M nodes per second and 12 to 16 plies with 7 to 15 pieces. The ending with two kings
  (agent_g41_ply101) is the exception: flying kings give many moves, and it gets only 10 plies.
- The larger table gives one more ply at the start and in the king ending, not in the middlegame. Its
  nodes per second are about 3 to 7 % lower in the two positions with many pieces (more cache misses
  in a table of 256 MB) and 11 % higher in the ending, where the table cuts more of the search. Three
  single runs are not enough to tell a ply apart from noise. An SPRT match would decide whether
  256 MB is worth it in games.
- A time budget depends on the machine and the clock: a second run gives other node counts and here and
  there another depth. Node budgets are the ones we compare agents with.

### Machine and method

- CPU: Intel Core i7-9750H @ 2.60GHz (laptop), pinned to CPU 2 with `taskset -c 2`; frequency governor
  `powersave` (intel_pstate); load average about 1.1 to 2.0 during the run. One search at a time.
- OS: CachyOS, Linux 7.2.9-1-cachyos. Rust: rustc 1.99.1, `cargo build --release`.
- The thirteen searches ran one after the other from 00:06 to 00:19, UTC+3 (about 13 minutes).
- `engine/examples/minute.rs` runs `iterative_deepening_with` with the agent's own limits, options,
  weights and table (built new for each search), with an empty game history, and times it with `Instant`.

### Reproduce

```
cargo build --release --example minute
taskset -c 2 target/release/examples/minute
```

`--seconds S` changes the time per search; `--out PATH` writes the JSON elsewhere.
