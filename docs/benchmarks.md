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

What one more ply costs. On the ladder's budget, 20,000 nodes a move, V10 completes 5.68 plies on average
(`avg_depth` of V10 in `results/ladder/ladder.json`). In one minute it completes a median of 10.5 plies
from a median of 28.3 M nodes. So the nodes grow by 28,332,031 / 20,000 = 1416 times while the depth
grows by 10.5 − 5.68 = 4.8 plies, and each extra ply costs about 1416^(1/4.82) = e^(ln 1416 / 4.82) =
e^(7.256 / 4.82) ≈ 4.5 times the nodes. A position has 13 to 16 legal moves (13.4 and 16.0 in
`deck/facts.json`, two sets of replayed ablation games), so plain minimax would pay b ≈ 13–16 times per
ply. Alpha-beta with perfect move ordering pays about √b ≈ 3.7–4.0 (AIMA 6.2: O(b^(m/2)) nodes instead of
O(b^m)). Our 4.5 sits just above that bound: with ordering, killers, history and the table, the search
is close to the best case, much closer than to minimax. Two caveats. The two numbers come from different
positions (5.68 is the mean over every searched move of the ladder's games, 10.5 the median of the ten
fixed positions here), so the ratio is an estimate, not a measurement on one set. And quiescence nodes
are in both counts, so b here is not exactly the b of the formula.

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

## Principal variation search and aspiration windows

The letter `p` and `:asp=W` (docs/decisions.md, `engine/src/search.rs`) change nodes, never decisions. V11 is
`idqrotf:n20000:w=texel1:fs=1`. Nodes are the same on every run; times depend on the machine and the moment.

### Nodes and time at a fixed depth

541 positions, each with its own weights; 3 samples, the median sample's time.

| search | nodes per move | µs per move | slowest position, µs | with `p`: nodes | with `p`: time |
|---|---|---|---|---|---|
| ab6o | 22033.1 | 40792.9 | 361345 | | |
| ab6op | 30614.6 | 56782.0 | 618122 | +38.9% | +39.2% |
| ab6ot | 14795.9 | 26500.9 | 288303 | | |
| ab6otp | 14255.8 | 25392.5 | 224930 | −3.7% | −4.2% |
| ab7ot | 55537.2 | 104232.9 | 1547644 | | |
| ab7otp | 52309.9 | 87377.7 | 792496 | −5.8% | −16.2% |

Without a table, `p` costs more than it saves: a test that lands inside the window is searched twice from nothing.
With a table the second search finds the first one's moves and bounds, and `p` pays a little. The time gain at
ab7ot is larger than its node gain and comes from one set of 3 samples; trust the nodes.

A root move above its test is searched again with the iteration's window (L, H). The tighter (max(L, best − 2ε), H)
was tried on a copy: 30423.9 nodes at ab6op (−0.6%), 14215.4 at ab6otp (−0.3%), 17588.6 at ab5p (−2.1% from 17969.6),
and the same mean depth with V11+p+asp=0.5, so it was not kept.

### Choosing W

`aspiration` on 541 positions with V11's own weights. A row covers the positions that completed that depth with no win
or loss on either side (`wins` is how many had one). |Δ| is the change of the best value from the iteration before, in
units of U (a man = 1); node share is of all completed iterations; "holds" is the share where a window of that W around
the previous value would count (L ≤ best − 2ε and best < H).

| depth | positions | wins | \|Δ\| p50 | p90 | p99 | node share | holds W=0.25 | W=0.5 | W=1 |
|---|---|---|---|---|---|---|---|---|---|
| 2 | 515 | 5 | 0.4645 | 1.1367 | 2.4538 | 0.005 | 0.219 | 0.544 | 0.889 |
| 3 | 510 | 10 | 0.3803 | 0.6999 | 1.8122 | 0.027 | 0.302 | 0.698 | 0.949 |
| 4 | 505 | 15 | 0.3280 | 0.6206 | 1.4427 | 0.085 | 0.337 | 0.762 | 0.962 |
| 5 | 482 | 23 | 0.3116 | 0.5792 | 1.4143 | 0.322 | 0.353 | 0.826 | 0.971 |
| 6 | 327 | 21 | 0.2845 | 0.5595 | 1.0885 | 0.366 | 0.437 | 0.841 | 0.985 |
| 7 | 108 | 10 | 0.2475 | 0.6539 | 1.5027 | 0.138 | 0.509 | 0.769 | 0.926 |
| 8 | 32 | 2 | 0.2487 | 0.4191 | 0.4915 | 0.036 | 0.500 | 1.000 | 1.000 |
| 9 | 12 | 1 | 0.1814 | 0.5215 | 0.7939 | 0.013 | 0.583 | 0.833 | 1.000 |
| 10 | 3 | 1 | 0.3521 | 0.4981 | 0.5309 | 0.005 | 0.333 | 0.667 | 1.000 |
| 11 | 1 | 0 | 0.8270 | 0.8270 | 0.8270 | 0.002 | 0.000 | 0.000 | 1.000 |
| **5+** | 965 | 58 | 0.2963 | 0.5730 | 1.4243 | 0.882 | 0.406 | 0.829 | 0.972 |

So the window starts at depth 5: depths 2 to 4 hold 11.7% of the nodes and fail most. W = 0.5 is about the 83rd
percentile of |Δ|. The agent really searching with each W, over the completed iterations that had a window:

| W | iterations with a window | first window held | failed tries per iteration | most failed tries | nodes in failed tries |
|---|---|---|---|---|---|
| 0.25 | 991 | 41.4% | 0.630 | 2 | 8.3% |
| 0.5 | 990 | 82.4% | 0.192 | 2 | 2.8% |
| 1 | 989 | 95.7% | 0.060 | 2 | 0.6% |

### Depth within 20,000 nodes

541 positions with V11's weights, no game history, a table emptied before each position; nodes per move 19224 in every row.
Mean depth is the sum of the completed depths over 541. "Proved" stopped because the last iteration reached no leaf at the
depth limit: it is 21 in every row, all of them at depth 1, so early proofs do not change the comparison.

| agent | mean depth | sum of depths | mean depth, not proved | proved | ms per move |
|---|---|---|---|---|---|
| V11 | 5.774 | 3124 | 5.97 | 21 | 32.04 |
| V11+p | 5.828 | 3153 | 6.02 | 21 | 31.67 |
| V11+asp=0.25 | 5.793 | 3134 | 5.99 | 21 | 31.70 |
| V11+asp=0.5 | 5.789 | 3132 | 5.98 | 21 | 31.77 |
| V11+asp=1 | 5.787 | 3131 | 5.98 | 21 | 32.01 |
| V11+p+asp=0.25 | 5.830 | 3154 | 6.03 | 21 | 31.72 |
| V11+p+asp=0.5 | 5.837 | 3158 | 6.03 | 21 | 31.74 |
| V11+p+asp=1 | 5.837 | 3158 | 6.03 | 21 | 31.82 |

W = 0.5 and W = 1 tie with `p`, to the ply. The best combination gains 0.063 plies on V11; at about 4.5 times the nodes
per ply (above), that is what 4.5^0.063 ≈ 10% more nodes would buy.

### Arena

`idqrotfp:n20000:w=texel1:fs=1:asp=0.5` against V11, seed 1, SPRT with the default bounds (`results/arena.csv`): no
verdict after 2000 games (the maximum), LLR 0.870 within ±2.944; +789 =487 −724, Elo of A over B +11.3 ± 10.7 (95%), 1689 s
with 6 threads. The row's version says `-dirty` because the match ran before the commit that holds the code.

### Machine and method

- CPU: Intel Core i7-9750H @ 2.60GHz (laptop), pinned to CPU 2 with `taskset -c 2`; frequency governor
  `powersave` (intel_pstate); load average 1.2 to 2.0 during the run, nothing else of ours running.
- OS: CachyOS, Linux 7.2.9-1-cachyos. Rust: rustc 1.99.1, `cargo build --release`.
- The fixed-depth searches ran from 20:36 to 20:48, the depth runs from 20:48 to 20:51, UTC+3, 2026-11-01.

### Reproduce

```
cargo build --release --all-targets
taskset -c 2 target/release/examples/search ab6o ab6op ab6ot ab6otp ab7ot ab7otp
taskset -c 2 target/release/examples/aspiration idqrotf:n20000:w=texel1:fs=1 0.25 0.5 1
taskset -c 2 target/release/examples/depth idqrotf:n20000:w=texel1:fs=1 idqrotfp:n20000:w=texel1:fs=1 \
    idqrotf:n20000:w=texel1:fs=1:asp=0.5 idqrotf:n20000:w=texel1:fs=1:asp=0.25 idqrotf:n20000:w=texel1:fs=1:asp=1 \
    idqrotfp:n20000:w=texel1:fs=1:asp=0.5 idqrotfp:n20000:w=texel1:fs=1:asp=0.25 idqrotfp:n20000:w=texel1:fs=1:asp=1
systemd-inhibit target/release/dama arena idqrotfp:n20000:w=texel1:fs=1:asp=0.5 idqrotf:n20000:w=texel1:fs=1 \
    --sprt --threads 6 --seed 1
```
