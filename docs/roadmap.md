# Roadmap

One engine, two faces. Both share `engine/` (rules, move generation, evaluation, search), the referees in
`testdata/` and the text protocol.

| | Show build | Performance build |
|---|---|---|
| Goal | explain the engine, let people play with it | play as strong as possible |
| Search | one thread, fixed nodes: the same position always gives the same tree | many threads, time limits |
| Evaluation | explainable: weight × feature | whatever is strongest (patterns, neural network) |
| Runs | in the browser (WASM) or behind a local server (open decision) | native, built for the machine |
| Extras | search trace, eval breakdown, tree view | Lazy SMP, large TT, endgame databases |

- Instrumentation goes through a generic tracer (`fn search<T: Tracer>`). The performance build uses `NoTrace`,
  whose empty methods compile away, so the tree we show is the search that really plays.
- The school version is frozen with a tag (`school-final`). Work continues on `main` with `exp/` branches and SPRT.
- Do not publish the show build before the course allows it: GitHub Pages sites are public even when the
  repository is private.

## How we measure
- Search quality and evaluation: fixed nodes (`n2000`, `n20000`) and SPRT. Simplifications use non-regression
  bounds [-20, 0].
- Speed and parallel search: fixed time per move, because fixed nodes cannot show them. Perft and the move lists
  must stay identical. Engine threads × parallel games ≤ physical cores.
- Elo we report: fixed-length matches (the ladder) with the zero at `v1-minimax`, not the estimate at the moment
  an SPRT stops, which runs high.
- Before a run, estimate the worst-case time and ask first if it exceeds 15 minutes.
- Long runs: charger plugged in, command wrapped in `systemd-inhibit --what=idle:sleep --why="..."`, and the run
  must be resumable.

## School track
1. Elo ladder of the tagged versions (`docs/ladder.md`, `results/ladder/`).
   - Reference opponent and Elo zero: `v1-minimax`, our first real search. The baseline (one-ply U) stays at the
     bottom of the chain as V0 and plays only its neighbours.
   - Matches: each version against the previous one, against `v1-minimax`, against the one two steps back, and
     the upper versions against a mid-ladder opponent (the repetition-aware search), because against
     `v1-minimax` they score above 95% and the comparison saturates.
   - A pairing that appears in several lists is played once. One joint Elo fit over all games, bootstrap
     confidence intervals.
2. Text protocol (`position`, `go`, `info`, `bestmove`, plus `eval` and `trace` for the show build) and a `Game`
   type (history, undo, draw rules). Scan's Hub protocol is a good model.
3. Show front-end, first version: Elo ladder charts (versus `v1-minimax`, versus the previous version, versus
   the mid-ladder opponent, Elo with confidence intervals, kinds of draws); board (play the engine, engine against engine, undo); eval
   breakdown with weight sliders and a square heat map; search panel (iterative deepening table, root move
   scores); technique ladder (minimax / alpha-beta / +ordering / +TT node counts on the same position).
4. Later: search tree explorer, horizon-effect demo (quiescence on and off), replay of arena games (the arena
   must store the moves).

## Performance track, in order
1. Profile with `perf` and `cargo flamegraph`.
2. Move generation speed, lessons from kish: no allocation per move, bulk capture detection, ray masks, a
   `count_moves` that counts without building lists, XOR-delta make/unmake, const generics, LTO and
   `codegen-units = 1`. First experiment: the cost of the `mob` feature (about 0 Elo in the texel1 ablation),
   measured at fixed time.
3. Aspiration windows, PVS.
4. Late move reductions.
5. Single-reply extension; draw rules inside the search (50 plies without a capture, one piece each); contempt.
6. Lazy SMP with a lockless shared TT (`AtomicU64` entries). It only matters under time limits.
7. Tapered eval, piece-square tables, runaway men; better Texel data (quiet positions, games of the current
   engine); SPSA for weights and search parameters.
8. Endgame databases by retrograde analysis, 2 bits per position with mirror and colour symmetry: up to 4 pieces
   about 42 M positions (10 MB), 5 pieces about 1.9 G (450 MB), 6 pieces about 66 G (16 GB).
9. Pattern evaluation (Scan style) or a small neural network.

## Ideas not scheduled yet
IID/IIR, counter-move heuristic, futility pruning and razoring, ProbCut, singular extensions, TT buckets and
replacement policy, enhanced transposition cutoff, null move (risky in draughts: zugzwang is common and having no
move loses), root splitting / YBWC / ABDADA, opening book, time management, pondering, multi-PV, criterion
benchmarks with kish-style scenarios, CI with GitHub Actions, a RULES.md with the edge cases, Python bindings for
training data.

## Open questions
- Mutual block: kish's RULES.md scores a position where neither side can move as a draw; our rule is "the side to
  move with no legal move loses". Perft cannot tell the two apart. Check the TÜDAF text.
- Final matches: time limit or node limit? How many cores? This decides how much Lazy SMP and time management
  are worth.
- Show build: engine in the browser (WASM) or behind a local server.
