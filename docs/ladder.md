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

## Monte Carlo tree search

Why alpha-beta and not Monte Carlo? Four forms of Monte Carlo tree search (engine/src/mcts.rs, AIMA 6.4), each adding
one thing to the one before it, play the ladder's V0, V2 and V10 with the same budget as alpha-beta: 20 000 nodes per
move, every position made with `apply` counted (in the tree, in random games and in quiescence).

| id | spec | what it is |
|---|---|---|
| MC | `mc:n20000` | flat Monte Carlo: the root moves in turn, a random game after each, the move with the highest mean |
| UCT | `uct:n20000` | UCT (UCB1, C = √2): a random game from each new position, the move visited most |
| UCTe | `ucte:n20000:w=texel1` | UCT, each new position scored with sigmoid(K · U), texel1, K = 0.2353 (its Texel K) |
| UCTq | `uctq:n20000:w=texel1` | UCT, scored with sigmoid(K · quiescence): captures searched until the position is quiet |

Random games end by the arena's rules (no legal move loses; one piece each, 50 quiet plies and ply 400 draw; the
game's ply and quiet plies carry over), not by a cut at a fixed length: then MC and UCT know nothing but the rules,
and all knowledge of the position comes in with UCTe. The third repetition is not checked in random games. With one
legal move the agents play it without a search (0 nodes), so their nodes per move over a game are below 20 000.

Settings as in the ladder: paired schedule, 4 random opening plies, seed 1, `--threads 6`, 200 games per pairing
(`MCTS_SET` in engine/src/ladder.rs, `dama ladder run --set mcts`, `dama ladder rate --set mcts`). Source:
`results/mcts/games/*.csv`, `results/mcts/mcts.json`, 12 rows of `results/arena.csv` (version `mcts-1`, tag
`mcts-1`), 2400 games in 1445 s.

### Results

Every view is the Monte Carlo form's. Elo and its 95% interval are those of the pairing alone (bootstrap over the
colour-swapped pairs); no pairing scored 0% or 100%, so each has a finite estimate. "Ladder" is the opponent's
Elo on the ladder (V1 = 0); "implied" is that plus the pairing's Elo, where the form would stand on the ladder by
this pairing alone. Draws: repetition / 50 quiet plies / one piece each / ply 400. Nodes and ms per move are the
form's, over the games of the pairing (6 games at once, so the time is not a clean speed measurement; see below).

| form | against | + = − | score | Elo | 95% | ladder | implied | draws rep / 50 / 1-1 / cap | nodes/move | ms/move |
|---|---|---|---:|---:|---|---:|---:|---|---:|---:|
| MC | V0 `tuned` | +114 =86 −0 | 78.5% | +225 | [+193, +263] | +8 | +233 | 40 / 44 / 2 / 0 | 18 704 | 23.2 |
| MC | V2 `ab5` | +4 =20 −176 | 7.0% | −449 | [−531, −387] | +175 | −274 | 16 / 2 / 2 / 0 | 17 501 | 20.5 |
| MC | V10 | +1 =4 −195 | 1.5% | −727 | [−1040, −604] | +672 | −55 | 2 / 0 / 2 / 0 | 16 409 | 18.7 |
| UCT | V0 | +131 =69 −0 | 82.8% | +272 | [+238, +308] | +8 | +280 | 47 / 22 / 0 / 0 | 18 765 | 23.6 |
| UCT | V2 | +17 =27 −156 | 15.2% | −298 | [−359, −249] | +175 | −123 | 19 / 3 / 5 / 0 | 17 574 | 21.3 |
| UCT | V10 | +5 =6 −189 | 4.0% | −552 | [−676, −463] | +672 | +120 | 3 / 0 / 3 / 0 | 16 323 | 19.5 |
| UCTe | V0 | +38 =162 −0 | 59.5% | +67 | [+49, +87] | +8 | +75 | 159 / 3 / 0 / 0 | 18 532 | 121.9 |
| UCTe | V2 | +14 =133 −53 | 40.2% | −69 | [−92, −42] | +175 | +106 | 122 / 2 / 9 / 0 | 17 202 | 106.3 |
| UCTe | V10 | +4 =30 −166 | 9.5% | −392 | [−449, −338] | +672 | +280 | 23 / 0 / 7 / 0 | 15 792 | 87.4 |
| UCTq | V0 | +36 =164 −0 | 59.0% | +63 | [+47, +81] | +8 | +71 | 162 / 2 / 0 / 0 | 18 417 | 89.0 |
| UCTq | V2 | +19 =117 −64 | 38.8% | −80 | [−109, −53] | +175 | +95 | 107 / 4 / 6 / 0 | 17 265 | 75.5 |
| UCTq | V10 | +5 =32 −163 | 10.5% | −372 | [−424, −323] | +672 | +300 | 25 / 0 / 7 / 0 | 15 993 | 74.4 |

Time per move on the same fixed positions, one search each, pinned to one core (`taskset -c 2
target/release/examples/mcts ...`: the 383 positions of testdata/decisions.json and movegen.json with more than one
legal move):

| agent | nodes/move | random games/move | iterations/move | ms/move | tree depth (search depth) |
|---|---:|---:|---:|---:|---:|
| MC `mc:n20000` | 20 000 | 1 177 | 1 177 | 13.3 | 1 |
| UCT `uct:n20000` | 20 000 | 1 423 | 1 423 | 14.8 | 4.98 |
| UCTe `ucte:n20000:w=texel1` | 20 000 | — | 20 000 | 69.4 | 8.21 |
| UCTq `uctq:n20000:w=texel1` | 20 000 | — | 14 746 | 55.9 | 7.90 |
| V10 `idqrot:n20000:w=texel1` | 19 687 | — | — | 33.9 | 5.69 |
| V2 `ab5` | 20 843 | — | — | 34.6 | 5.00 |
| V0 `tuned` | 12 | — | — | 0.02 | 1 |

A random game is short: 20 000 nodes are 1 177 games for MC, about 17 plies each (captures are mandatory, so the
pieces go fast). UCTe spends one node per iteration and is the slowest per node (a new tree node, its legal moves and
U); UCTq spends a quarter of its nodes in quiescence.

- **No form comes near alpha-beta.** Each beats V0 without losing a game and loses to V2; against V10 the best form
  scores 10.5%. By head-to-head scores the forms stand between V0 (+8) and V2 (+175) on the ladder; V2 (5 plies,
  the tuned U, no quiescence, no repetitions) is already stronger than all of them with the same 20 000 nodes.
- **The implied places disagree** (MC: +233, −274, −55): the ladder's scale is not one line here either (see
  Intransitivity). The one-ply V0 draws by repetition against agents that do not avoid it, which pulls the forms
  with U down against V0; MC and UCT lose almost every game to V10, where an Elo from 1.5–4% is very uncertain.
- **The tree is worth something, U more.** UCT is above MC in every pairing (V2: 15.2% against 7.0%). U at the leaves
  instead of random games more than doubles the score against V2 (40.2%) and V10 (9.5%), but is worse against V0 (see the
  repetitions below).
- **Quiescence is worth nothing to MCTS** (UCTq against UCTe: 59.0 and 59.5%, 38.8 and 40.2%, 10.5 and 9.5%; every
  interval overlaps), while it is worth 265 to V10 (Ablation). The question was whether MCTS is weak tactically, at
  leaves in the middle of a capture sequence: it is not, or not mainly. UCTe's tree is already 8 plies deep on average,
  probably deepest along forced captures (few children), and quiescence costs a quarter of the iterations. What alpha-beta has
  and UCT does not is a proof that a move is bad: one refutation cuts a move, where UCT needs many visits to lower
  a mean.

### Three properties of these forms, and what each does to the results

Found by the tests (`engine/tests/mcts.rs`) and in the games; the draws were played again and their last boards
counted with `target/release/examples/endings results/mcts/games/<pairing>.csv` (every game equal to the stored one;
"ahead" counts pieces, "ahead by 2" counts a king as 2.51 men, texel1's king).

1. **The forms with random games do not prefer a quicker win.** In a won ending every move wins every random game,
   so all have the mean 1 and the first in legal order is played (`a_move_that_wins_at_once_is_played`). Effect: the
   wins of MC and UCT are long. Against V0 they take 157 and 175 plies on average, those of UCTe, UCTq and V10 69,
   70 and 93 (`V10_vs_V0` of the ablation). It does not turn their wins into draws by 50 quiet plies: in 34 of MC's
   44 such draws against V0, MC has fewer pieces (UCT: 15 of 22), so those are V0's unfinished wins, not MC's. So it
   costs time, not points, in these games.
2. **UCTq sees a win after forced captures as a win.** Quiescence finds the loss of the side that must capture into
   a position without a move; sigmoid(0.2353 · ~1000) is exactly 1 in f64, so such a move scores 1, as a move that
   wins at once does (the test checks that only UCTe must play the move that wins at once). Effect: none measurable;
   it is part of what quiescence adds, and UCTq is not stronger than UCTe in any pairing. Values that saturate at 1
   do mean that UCTq, like the forms with random games, does not prefer the quicker of two forced wins.
3. **MCTS does not know repetitions.** Neither in its random games nor in its tree; UCTe and UCTq are also
   deterministic (no random numbers at all), so the same position gives the same move and a game can cycle. V0 and
   V2 do not know repetitions either; V10 does. Effect, in both directions:
   - **against V0 it costs points**: UCTe draws 159 of 200 games by repetition (UCTq 162); its games last 52 plies on average. In
     63 of those 159 UCTe is ahead by at least two men (UCTq: 57 of 162), and has more pieces in 75. Had the 63 been
     wins, UCTe would have scored 75% instead of 59.5% against V0. This is why UCTe is below UCT against V0.
   - **against V2 it saves points**: in 74 of UCTe's 122 repetition draws UCTe has fewer pieces (UCTq: 48 of 107);
     ab5 is ahead and repeats. Without these draws UCTe's 40.2% against V2 would be lower.
   - against V10 it matters little (23 and 25 repetition draws of 200).

### Caveats

- One budget (20 000 nodes), one exploration constant (√2) and no tuning of either; MCTS is known to need far more
  iterations than alpha-beta needs nodes, and a larger budget could change the order of the forms, not likely the
  gap to V10 (the alpha-beta agents grow stronger with nodes too).
- No form uses repetitions, a transposition table or an ordering of its expansions; they are the plain algorithms of
  the textbook, compared with V2 (plain alpha-beta, 5 plies) and V10 (every improvement).
- 200 games per pairing: intervals of ±20–40 Elo near 50% and far wider near 0%.
