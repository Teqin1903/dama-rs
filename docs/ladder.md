# Elo ladder of the tagged versions

Every tagged agent on one Elo scale, from fixed-length matches. The zero is V1 = `mm3` (tag `v1-minimax`).
Fixed lengths, not SPRT: an SPRT stops when its estimate happens to be far from 0, so its Elo runs high.

Source: `results/ladder/games/<pairing>.csv` (one row per game), `results/ladder/ladder.json` (written by
`dama ladder rate`), and one row per pairing in `results/arena.csv`. Version `v10-transposition-table-2-g2756964`,
run on 2026-10-29, 6800 games in 4619 s on 6 threads.

## Ratings

| id | tag | spec | adds | Elo | 95% | nodes/move | depth/move |
|---|---|---|---|---:|---|---:|---:|
| V0 | baseline | `tuned` | one ply, U = 1/2.5/−0.05 | +8 | [−2, +18] | 11 | 1.00 |
| V1 | v1-minimax | `mm3` | minimax, 3 plies | **0** | (zero) | 1 543 | 3.00 |
| V2 | v2-alpha-beta | `ab5` | alpha-beta, so 5 plies | +175 | [+160, +191] | 21 397 | 5.00 |
| V3 | v3-iterative-deepening | `id:n20000` | iterative deepening, 20 000 nodes per move | +149 | [+134, +164] | 19 829 | 4.88 |
| V4 | v4-quiescence | `idq:n20000` | captures searched past the limit | +202 | [+185, +218] | 19 815 | 4.73 |
| V5¹ | v5-arena | — | — | — | — | — | — |
| V6 | v6-repetition | `idqr:n20000` | repetitions are draws (mid opponent) | +249 | [+231, +266] | 19 727 | 4.73 |
| V7² | v7-sprt-eval-params | — | — | — | — | — | — |
| V8 | v8-texel1 | `idqr:n20000:w=texel1` | Texel-tuned weights | +499 | [+476, +524] | 19 660 | 4.53 |
| V8b | exp-texel2 | `idqr:n20000:w=texel2` | side rung: lower MSE, weaker engine | +379 | [+353, +405] | 19 648 | 4.53 |
| V9 | v9-move-ordering | `idqro:n20000:w=texel1` | best move of the previous iteration, promotions, killers, history | +594 | [+565, +621] | 19 665 | 5.22 |
| V10 | v10-transposition-table | `idqrot:n20000:w=texel1` | 16 MB transposition table | +672 | [+641, +703] | 19 646 | 5.68 |

¹ V5 (`v5-arena`) is measurement infrastructure: the paired schedule, per-game seeds, parallel games. The agent did
not change, so it has no rung.
² V7 (`v7-sprt-eval-params`) added the SPRT and the parametrised U with every positional weight 0, which is U bit for
bit. The agent did not change, so it has no rung.

Nodes and depth are per move over all the version's games in the ladder (depth: the completed search depth of the
move played). The bootstrap had no diverged sample in 1000.

## Method

- **Pairings** (`plan` in `engine/src/ladder.rs`), 25 matches: each rung against the one before it in ladder order
  (V0 V1 V2 V3 V4 V6 V8 V9 V10; 400 games), V8b against V8 (side, 400), every rung above V1 against V1 (anchor, 200),
  against the rung two steps back (two_back, 200), the rungs above V6 against V6 (mid, 200), and V1 against itself
  (self, 200, left out of the fit). A pair that appears in two roles is played once, with the larger count.
- **Games**: `dama arena` settings: paired schedule (each opening twice with the colours swapped), 4 random opening
  plies, seed 1 for every pairing, `--threads 6` (the games do not depend on it); draws by the third repetition, 50
  plies without a capture, one piece each, or ply 400.
- **Fit**: one Bradley-Terry model over all games, draws as half a win, E = 1 / (1 + 10^(−(Ra − Rb)/400)), solved
  by maximum likelihood (Newton with a line search), V1 = 0 (`engine/src/elo.rs`). A fit exists only if every
  rung can be reached from every other through "scored against" (Ford's condition); this held.
- **Intervals**: 1000 bootstrap samples (mulberry32, seed 1); in each, the colour-swapped pairs of every pairing
  are drawn again with replacement and the model refitted; 2.5 and 97.5 percentiles.
- Commands: `dama ladder run` (resumable), `dama ladder rate` (plays nothing).

## Pairings

Elo here is each pairing alone (its score); "fit" is the difference of the two ratings above.

| pairing | roles | games | + = − | score | Elo | 95% | fit | draws: rep / 50 / one each | avg plies |
|---|---|---:|---|---:|---:|---|---:|---|---:|
| V1–V0 | previous | 400 | +25 =375 −0 | 53.1% | +22 | [+13, +31] | −8 | 333 / 42 / 0 | 47 |
| V2–V1 | previous, anchor | 400 | +209 =189 −2 | 75.9% | +199 | [+176, +224] | +175 | 167 / 18 / 4 | 83 |
| V3–V2 | previous | 400 | +59 =232 −109 | 43.8% | **−44** | [−64, −23] | −26 | 166 / 47 / 19 | 111 |
| V4–V3 | previous | 400 | +90 =272 −38 | 56.5% | +45 | [+30, +62] | +53 | 193 / 73 / 6 | 106 |
| V6–V4 | previous | 400 | +84 =284 −32 | 56.5% | +45 | [+29, +62] | +47 | 203 / 71 / 10 | 93 |
| V8–V6 | previous, mid | 400 | +300 =53 −47 | 81.6% | +259 | [+225, +298] | +250 | 3 / 16 / 34 | 121 |
| V9–V8 | previous | 400 | +213 =93 −94 | 64.9% | +107 | [+81, +131] | +95 | 17 / 27 / 49 | 107 |
| V10–V9 | previous | 400 | +196 =97 −107 | 61.1% | +79 | [+52, +104] | +77 | 6 / 35 / 56 | 106 |
| V8b–V8 | side | 400 | +90 =67 −243 | 30.9% | **−140** | [−169, −112] | −120 | 6 / 13 / 48 | 103 |
| V3–V1 | anchor, two_back | 200 | +104 =92 −4 | 75.0% | +191 | [+158, +228] | +149 | 74 / 17 / 1 | 96 |
| V4–V1 | anchor | 200 | +100 =99 −1 | 74.8% | +189 | [+153, +228] | +202 | 84 / 13 / 2 | 83 |
| V6–V1 | anchor | 200 | +140 =59 −1 | 84.8% | +298 | [+258, +342] | +249 | 34 / 22 / 3 | 96 |
| V8–V1 | anchor | 200 | +175 =25 −0 | 93.8% | +470 | [+413, +541] | +499 | 18 / 5 / 2 | 92 |
| V8b–V1 | anchor | 200 | +170 =26 −4 | 91.5% | +413 | [+359, +478] | +379 | 13 / 7 / 6 | 103 |
| V9–V1 | anchor | 200 | +176 =24 −0 | 94.0% | +478 | [+418, +552] | +594 | 17 / 2 / 5 | 89 |
| V10–V1 | anchor | 200 | +184 =16 −0 | 96.0% | +552 | [+478, +655] | +672 | 11 / 3 / 2 | 90 |
| V1–V1 | self | 200 | +15 =166 −19 | 49.0% | −7 | [−26, +12] | (0) | 135 / 21 / 10 | — |
| V2–V0 | two_back | 200 | +55 =145 −0 | 63.7% | +98 | [+76, +119] | +167 | 130 / 14 / 1 | 65 |
| V4–V2 | two_back | 200 | +54 =114 −32 | 55.5% | +38 | [+10, +65] | +27 | 90 / 19 / 5 | 104 |
| V6–V3 | two_back | 200 | +77 =110 −13 | 66.0% | +115 | [+91, +141] | +100 | 76 / 26 / 8 | 108 |
| V8–V4 | two_back | 200 | +145 =43 −12 | 83.2% | +279 | [+235, +323] | +297 | 35 / 4 / 4 | 109 |
| V9–V6 | two_back, mid | 200 | +164 =24 −12 | 88.0% | +346 | [+298, +413] | +345 | 0 / 6 / 18 | 119 |
| V10–V8 | two_back | 200 | +132 =32 −36 | 74.0% | +182 | [+143, +228] | +172 | 3 / 12 / 17 | 105 |
| V8b–V6 | mid | 200 | +133 =19 −48 | 71.2% | +158 | [+121, +200] | +130 | 3 / 7 / 9 | 122 |
| V10–V6 | mid | 200 | +177 =16 −7 | 92.5% | +436 | [+372, +521] | +423 | 4 / 4 / 8 | 114 |

No game reached ply 400. V1 against itself scores 49.0%, within its interval of 50%: the schedule is fair to both
colours.

## Steps that go backward

- **V3 below V2 (−26 in the fit, −44 head to head).** Iterative deepening was played as it was measured, with a
  budget of 20 000 nodes, against ab5's fixed 5 plies (about 21 400 nodes per move). Its cost shows at once: it
  searches depths 1, 2, 3, ... again before the last one, and the last iteration, when the budget runs out inside
  it, is thrown away. So with a slightly smaller budget it completes 4.88 plies per move on average, not 5. Its gain
  comes later: the budget in nodes or time makes every later idea (quiescence, repetitions) fit into the same cost,
  and V9's move ordering searches the previous iteration's best move first, which only iterative deepening has.
  From V9 on the completed depth passes 5 (5.22, then 5.68 with the table).
- **V8b below V8 (−120).** texel2 fitted the game results better than texel1 (lower MSE) but plays clearly worse
  (30.9% head to head): a better fit of the evaluation is not a stronger engine. V8b is a side rung; it is not in
  the chain.
- **V0 and V1 are level (+8 for V0, interval [−2, +18]).** Head to head mm3 scores 53.1%, but against ab5 the
  one-ply baseline holds 145 draws in 200 games, mostly by repetition, where mm3 holds only 189 in 400. The fit
  weighs both.

## Intransitivity

Where the fit and a pairing disagree by more than its interval, the scale is not one line:
- **V2–V0**: +98 against +167 in the fit. The baseline draws by repetition against a deeper searcher; ab5 has no idea
  of repetitions and lets it.
- **V9–V1, V10–V1**: +478 and +552 against +594 and +672. This is saturation (see below), not an inversion: 94–96%
  scores, no losses.
- **V3–V1 and V6–V1**: +191 and +298 against +149 and +249, the fit below both intervals. V3 does about as
  well as V2 against V1, yet loses to V2 head to head.

Among the upper rungs the scale is close to transitive: V8–V6 +259 and V9–V8 +107 against V9–V6 +346; V10–V8 +182
against +95 + +77 via V9; V10–V6 +436 against the fit's +423. The side rung fits too: −140 against V8, +158 against
V6 and +413 against V1 give +359, +407 and +413 for V8b, rated +379.

## Kinds of draws

| pairings | games | draws | repetition | 50 plies | one piece each |
|---|---:|---:|---:|---:|---:|
| among V0–V6 (with self) | 3200 | 1971 (62%) | 1550 | 362 | 59 |
| with V8 or later | 3400 | 535 (16%) | 136 | 141 | 258 |

Up to V6 most draws are repetitions; V6 sees repetitions in its search but still often repeats against an opponent
of its strength. From V8 on (texel1 weights) repetition draws nearly vanish (V8–V6: 3 in 400), and the draws left
are mostly endgames with one piece each. The games show the change; why texel1 avoids repetitions is not measured
here.

## Caveats

- **Self-play Elo.** The ratings are differences between our own versions in our own matches. They cannot be
  compared with chess Elo, with FMJD ratings, or with another team's numbers.
- **Fixed depth, then fixed nodes.** V0–V2 search to a fixed depth; V3 on search within 20 000 nodes per move. The
  budget was chosen to be close to ab5's cost (21 400 nodes per move), but the two kinds of rung are not matched
  exactly; time was not the budget.
- **Saturation against V1.** Scores against V1 reach 94–96% at the top, where one game more or less moves the Elo a
  lot (V10–V1's interval is [+478, +655]). The differences between the upper rungs come from the previous, two_back
  and mid matches.
- Every pairing uses seed 1, so all pairings start from the same openings; the intervals cover the choice of
  openings within a pairing, not other seeds.
- `ms_per_move` in ladder.json was taken with six games at once and is not a speed measurement.

## Ablation

What each part of V10 is worth now: V10 (`idqrot:n20000:w=texel1`) against itself with one part turned off, and
against the A3 agent V0. Same settings as the ladder (paired, 4 opening plies, seed 1, `--threads 6`), 400 games
each. Source: `results/ablation/games/`, `results/ablation/ablation.json` (`dama ladder run --set ablation`,
`dama ladder rate --set ablation`), version `ladder-1-1-g11a369d`, 2000 new games in 1535 s. V10–V9 is V10 without
its table; it is the ladder's pairing (version `v10-transposition-table-2-g2756964`, same code for the agents),
read from `results/ladder/games/` and not played again.

The fit has V10 at 0, and every pairing is against V10 (a star), so each rating is exactly its pairing's own Elo.
"Lost" is V10's Elo over the version without the part.

| part off | spec | + = − (V10's view) | score | lost | 95% | draws: rep / 50 / one each | nodes/move | depth/move |
|---|---|---|---:|---:|---|---|---:|---:|
| quiescence | `idrot:n20000:w=texel1` | +297 =63 −40 | 82.1% | **265** | [230, 301] | 6 / 12 / 45 | 19 558 | 5.99 |
| repetitions | `idqot:n20000:w=texel1` | +134 =140 −126 | 51.0% | 7 | [−15, +29] | 81 / 20 / 39 | 19 696 | 5.88 |
| move ordering¹ | `idqrt:n20000:w=texel1` | +199 =92 −109 | 61.3% | 80 | [52, 108] | 11 / 27 / 54 | 19 667 | 5.32 |
| transposition table (V9) | `idqro:n20000:w=texel1` | +196 =97 −107 | 61.1% | 79 | [53, 107] | 6 / 35 / 56 | 19 676 | 5.31 |
| Texel weights | `idqrot:n20000:w=tuned` | +308 =51 −41 | 83.4% | **280** | [246, 323] | 2 / 20 / 29 | 19 695 | 5.71 |
| everything since A3 (V0) | `tuned` | +313 =87 −0 | 89.1% | 365 | [332, 405] | 44 / 43 / 0 | 9 | 1.00 |

V10 itself: 19 651 nodes and 5.68 plies per move over these games. Nodes and depth above are the weaker side's.

¹ Without `o` the search still tries the table's move first at every node (`bounded_value` in
engine/src/search.rs); only the previous iteration's best move at the root, promotions, killers and history are off.
So this row measures the ordering the table does not already give.

- **Quiescence and the weights are worth the most** (265 and 280). Without quiescence V10 completes more plies
  (5.99 against 5.68), since no node goes into captures past the limit, and plays far worse: depth that ends in the
  middle of a capture sequence misjudges it.
- **Repetitions are worth nothing measurable at the top** (+7, interval across 0). In the ladder they were worth
  +45 (V6–V4, tuned weights). Without them V10's games end by repetition far more often (81 draws against 6–11 in
  the other rows), but it loses about as often as it wins (126 against 134). Why repetitions stopped mattering is not measured
  here; texel1 already avoids most of them (see Kinds of draws).
- **Ordering and the table are worth the same** (80 and 79), and each about what it was worth as a rung (V9–V8
  +107, V10–V9 +79). Both buy depth within the same 20 000 nodes: 5.32 and 5.31 plies against V10's 5.68.
- **V0: +365 here, +664 on the ladder's scale** (V10 +672, V0 +8). V10 never loses to the baseline, but 87 of 400
  games are drawn, by repetition and 50 quiet plies. The same as V2–V0 in the ladder: the one-ply baseline holds
  draws against much stronger agents, so it rates higher head to head than through the chain. Since A3 the agent is
  +365 against V0 directly; on the ladder's scale +664.
- Each part is measured with all the others on. Their losses do not add up to V0's 365, and need not: the parts
  depend on each other (ordering needs iterative deepening, the weights need depth to matter).

### Quiescence with the tuned weights

Quiescence is worth 265 to V10 but was worth only +53 as a rung (V4–V3, 202 against 149). The guess: the Texel
weights were fitted on quiet positions only, and without quiescence they are applied in the middle of capture
sequences, where they are out of their training range. The test is the same pair with the A3 weights in place of
texel1 (`arena idqrot:n20000:w=tuned idrot:n20000:w=tuned --games 400 --seed 1 --threads 6`, same settings,
version `ablation-1-3-gc32f484`, 387 s, row in `results/arena.csv`):

| weights | with q | without q | + = − (with q's view) | score | q is worth | 95% | draws: rep / 50 / one each |
|---|---|---|---|---:|---:|---|---|
| texel1 | `idqrot:n20000:w=texel1` | `idrot:n20000:w=texel1` | +297 =63 −40 | 82.1% | **265** | [230, 301] | 6 / 12 / 45 |
| tuned | `idqrot:n20000:w=tuned` | `idrot:n20000:w=tuned` | +142 =176 −82 | 57.5% | **53** | [27, 78] | 30 / 115 / 31 |

(Elo from the score; the interval is score ± 1.96 SE, SE 1.84 points for tuned.)

- With the tuned weights quiescence is worth +53, the same as the ladder's V4–V3 (+53), although r, o and t
  are now on in both agents. The intervals of the two rows do not overlap. Most of the 265 therefore comes from the weights, not from
  the rest of V10: quiescence matters five times more with texel1 than with the weights it was added with.
- This agrees with the guess and does not prove it. The tuned weights are also U alone (`man`, `king`, `mob`, every
  positional weight 0), so they may simply misjudge fewer unquiet positions, whether or not those were in a training set. A direct
  test would fit weights on all positions, quiet or not, and repeat the pair.
- With the tuned weights the games are longer (123 plies against 88) and end drawn far more often (176 of
  400, 115 of them by 50 quiet plies), as in the ladder's lower rungs.
