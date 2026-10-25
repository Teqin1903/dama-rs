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
