# Web API: the contract between the engine and the site

The site (`site/`) shows our agent at work: the moves it considers, what each one is worth to it, why it
plays the one it plays, its search tree and its evaluation. It never computes game logic itself. Everything
comes from the Rust engine compiled to WebAssembly, through the calls below. Inputs and outputs are JSON
strings, so the same calls can be tested natively.

## Build

- New crate `web/` (`crate-type = ["cdylib", "rlib"]`), depending on `engine` and `wasm-bindgen`.
  The logic lives in `engine` (for example `engine::api`); `web/` only wraps it.
- Build: `scripts/build-web.sh`, which runs `wasm-pack build web --target no-modules --release --out-dir
  ../site/pkg --out-name dama_web --no-typescript --no-pack` and keeps only the two files below
  (`rustup target add wasm32-unknown-unknown` once). This writes `site/pkg/dama_web.js`, which defines the
  global `wasm_bindgen`, and `site/pkg/dama_web_bg.wasm`. `--target no-modules` matters: the site is also
  packed into one offline HTML file that works from `file://`.
- No threads and no `std::time::Instant` in the WebAssembly build (it panics there): budgets are node
  budgets; `ms` may come from `js_sys::Date::now()` or be 0.
- A native mirror for tests and for building the site without a browser: `dama api` reads one JSON request
  per line on stdin, `{"call": "think", "args": ["white", "{}"]}`, and prints the same JSON the WebAssembly
  call returns. One `Session` lives for the whole stdin stream.

## Exports

```
version() -> {"engine": "src.<7 hex digits: git tree of the WebAssembly sources, web/build.rs>", "specs": ["tuned", "mm3", "ab5", "id[q][r][o][t]:nN[:w=W]", ...]}

class Session
  new()                                  start position, White to move, no agents
  reset(board, turn)        -> State     board string as in docs/reference.md, turn "white" | "black"
  state()                   -> State
  legal()                   -> Move[]
  play(move)                -> State     move in our notation; must be legal
  undo()                    -> State
  set_agent(side, spec, seed) -> {"ok": true, "spec": "<normalised spec>"}
  think(side, options)      -> Report    that side's agent decides on the current position (see below)
  analyze(spec, options)    -> Report    a fresh agent searches a position of this game
  eval(weights, options)    -> Eval
  position(options)         -> Position
```

Every call that fails returns `{"error": "<plain sentence>"}` and leaves the session unchanged. Nothing may
panic across the boundary (validate input; install `console_error_panic_hook` in the WebAssembly build).

`options` is a JSON object; every field is optional:

| field | used by | meaning |
|---|---|---|
| `back` | analyze, eval, position | the position `back` plies before the current one, with the game history up to it (repetition) |
| `line` | eval, position | moves to play from that position first |
| `seed` | analyze | seed of the fresh agent (default 1) |
| `only` | analyze | search only these root moves (like UCI `searchmoves`) |
| `trace` | think, analyze | `{"iteration": -1, "max_nodes": 80000}`: record the search tree (see Trace) |

## Semantics

- `think` is the arena's decision, unchanged. The agent set by `set_agent` keeps its memory between calls
  (transposition table, killers, history, random generator) exactly as in an arena game, and `best` is the
  move it plays. The site then calls `play(best)`. A test plays one scripted game through `Session` and
  through the arena with the same seeds and checks that every move is the same.
- Values use the engine's scale (U: one man = 1.0). Report values are from the side to move at the root.
  Wins keep the engine's own scores (for example 1000 minus plies); the site treats |value| >= 800 as a
  forced result.
- `root` lists every legal root move of the last completed iteration: `value` and `bound` as alpha-beta
  left them. With our root window only the best moves are exact; the others are `"upper"` (at most this).
  A move the iteration never reached has `value: null`.
- One-ply agents (`tuned`, `first`) report depth 1, one iteration, and U of each move as an exact value.
  Fixed-depth agents (`mmN`, `abN`) report one iteration.
- Tracing must not change the search. The tracer is a generic parameter (`NoTrace` compiles away) and keeps
  its own tables; it adds nothing to the transposition table or the search state. Test: on the 141 positions
  of `testdata/decisions.json` and on positions from arena games, a traced and an untraced search give the
  same best move, value and node count.
- `eval` returns the engine's static evaluation, broken into its features. `total` equals the value the
  search uses for that position (test it), from `side`'s view: by default the side that has just moved
  into that position (see Eval).
- Draw rules and game end are the arena's: third repetition, 50 plies without a capture, one piece each,
  ply 400, and a side with no legal move loses.

## JSON types

```
State = {"board": "......../bbbbbbbb/...", "turn": "white", "ply": 12, "moves": ["c3-c4", ...],
         "status": "playing" | "white" | "black" | "draw",
         "reason": null | "no_moves" | "repetition" | "fifty" | "one_each" | "ply_cap",
         "quiet_plies": 3, "repetition_count": 1,
         "pieces": {"white_men": 16, "white_kings": 0, "black_men": 16, "black_kings": 0}}

Move = {"move": "a6xa2xf2", "from": "a6", "to": "f2", "path": ["a6", "a2", "f2"],
        "captured": ["a4", "c2"], "promotes": false}

Report = {"spec": "idqrot:n20000:w=texel1", "side": "white", "board": "<board at the decision>",
          "turn": "white", "ply": 12, "legal": 9,
          "best": "d3-d4", "value": 0.42, "bound": "exact", "depth": 6, "nodes": 19834, "qnodes": 7340, "ms": 48,
          "pv": ["d3-d4", "e6-e5", ...], "ties": ["d3-d4"],
          "iterations": [{"depth": 1, "best": "c3-c4", "value": 0.10, "nodes": 12, "ms": 0, "pv": ["c3-c4"]}, ...],
          "root": [{"move": "d3-d4", "value": 0.42, "bound": "exact", "order": 1, "reason": "tt", "nodes": 8120}, ...],
          "stats": {"qnodes": 7340, "tt_probes": 9000, "tt_hits": 2100, "tt_cutoffs": 800,
                    "beta_cutoffs": 3000, "first_move_cutoffs": 2700, "repetition_draws": 4},
          "trace": Trace (only when asked)}

reason (why a move was tried where it was) = "tt" | "pv" | "killer" | "history" | "promotion" | "capture" | "none"

Trace = {"iteration": 6, "nodes_total": 8732, "truncated": false,
         "nodes": [{"id": 0, "parent": null, "move": null, "ply": 0, "depth": 6,
                    "kind": "main" | "quiescence",
                    "alpha": null, "beta": null,          (null = infinite; from the root side)
                    "value": 0.42, "bound": "exact" | "upper" | "lower",   (from the root side)
                    "static": null | 0.12,                (when the node returned its static evaluation)
                    "children": [1, 57, ...],             (in the order tried)
                    "legal": 9, "order": null | reason,   (why this node's move was tried there)
                    "cutoff": false,                      (true: the last child caused a cutoff)
                    "tt": null | "miss" | "hit" | "cut",
                    "draw": null | "repetition" | "one_each" | "fifty",
                    "sub": 8732,                          (nodes in this subtree, itself included)
                    "in": 0, "out": 17463}]}              (visit and exit order, for step-by-step replay)

Values and windows in a trace are from the root side: MAX nodes have the root side to move, MIN nodes the
other side. Our search already keeps every value from the root side's view (search.rs), so they are reported
as they are, with no sign change. Nodes are numbered in visit order; recording stops at max_nodes
(truncated: true) while the search itself runs on unchanged. iteration -1 means the last completed iteration.

Eval = {"weights": "texel1", "side": "white", "board": "<board>", "total": 0.33, "win": null,
        "features": [{"key": "adv", "name": "advancement", "white": 12, "black": 9, "weight": 0.0831, "contribution": 0.249}, ...],
        "squares": [{"square": "d4", "piece": "w", "contribution": 1.21}, ...]}

  As in eval.rs, every number is "ours minus theirs" from `side`: by default the side that has just moved
  into that position (the opposite of the side to move), which is how the search scores a leaf. An options
  field "side": "white" | "black" asks for the other view. features: the engine's eight (man, king, mob, adv,
  back, cen, edge, qmob), only those with a weight that is not 0, `key` the engine's name, `name` for people:
  men, kings, mobility, advancement, back row, centre, edge, quiet mobility. squares: each piece's share of
  the features that are sums over pieces (man or king, adv, back, cen, edge, its own quiet moves for qmob);
  mob belongs to no single piece and appears only in features.

Position = {"board": "<board>", "turn": "black", "legal": Move[]}
```

## What the site calls, and when

| moment | calls |
|---|---|
| new game, player change | `reset`, `set_agent` for each agent side |
| an agent's turn | `think(side, {"trace": ...})` (trace only in developer mode), `eval(w, {"line": pv})`, then `play(best)` |
| a human move | `legal`, `play` |
| expected-line arrows, tree browsing | `position({"back": k, "line": [...]})` |
| what if | `analyze(spec, {"back": k})`, and `analyze(full, {"back": k, "only": [move]})` to score the alternative |

The five opponents (difficulty levels) are our ladder versions: `tuned` (V0), `ab5` (V2), `idqr:n20000`
(V6), `idqr:n20000:w=texel1` (V8), `idqrot:n20000:w=texel1` (V10). What-if uses `id` with any subset of
q, r, o, t, budgets n2000, n20000 and n200000, and weights texel1 or tuned.

## Notes for our engine (from reading engine/src)

- PV and iterations are new: `root_alpha_beta` keeps no principal variation today. Add a triangular PV
  table filled as alpha rises (it changes no value and no node count), and record each completed iteration
  of `iterative_deepening_with` (depth, first of `top`, its value, nodes so far, PV).
- Root `order`, `reason` and `nodes`: count the nodes under each root move in `root_alpha_beta`; `reason`
  is the `MoveOrder` category that placed the move (table move, previous best, promotion, killer, history,
  plain order). Without ordering every move is "none", except the table move when there is a table.
- The session plays like the arena: `set_agent` builds the agent from its spec, calls `new_game(seed)` and
  `observe`s every position of the game so far; `play` makes both agents `observe` the new position. After
  `undo`, an agent's game history (repetitions) must match the session again; its table may keep its
  entries. If an agent cannot drop a position from its history, rebuild the history and keep the table.
- Specs are `AgentSpec` names, overrides included: what-if switches evaluation features off with names like
  `idqrot:n20000:w=texel1:adv=0`.
- `reason` in State maps `EndReason`: Repetition "repetition", NoMove "no_moves", OnePieceEach "one_each",
  QuietPlies "fifty", Cap "ply_cap".
- Stats: count probes, hits and cutoffs of the table, beta cutoffs and cutoffs by the first move tried, and
  repetition draws. They may come from a counting tracer, so the arena's search stays untouched.
- `Instant::now()` panics in WebAssembly: keep `Limits::time` at None there and take `ms` from
  `js_sys::Date::now()` in the `web` crate (0 natively is fine).
- At the root, the moves are tried in legal order, or with the previous iteration's best move first (with
  ordering, `o`): a root move's `reason` is `"pv"` or `"none"`. The root takes no move from the table, so
  `"tt"` never appears there; it does below the root.
- A trace's `draw` is in practice only `"repetition"`: the search does not apply the 50-ply and one-piece-each
  rules (only the game does), so `"fifty"` and `"one_each"` do not occur in it.
