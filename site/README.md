# Site: the live demo

An agent plays an opponent while the screen shows the moves it considers, what each one is worth to it
and why it plays the one it plays. Developer mode shows its search iterations, the search tree node by
node, the evaluation feature by feature, and what-if searches with parts of the engine switched off.

Every move, value and tree on the page comes from our Rust engine, compiled to WebAssembly in `pkg/`
(docs/web-api.md). The page itself computes no game logic.

## Open it

- One file, no server: `dist/dama.html` (build it with `node site/tools/bundle.js`). Double-click works.
- From the folder, with a small web server (a page opened from disk may not load the `.wasm` file):

      cd site
      python -m http.server 8000

  then open http://localhost:8000.

Keys: Space play or pause, Right arrow one move, D switch view, T search tree, U undo, F flip, N new game,
Esc cancel.

When agents play each other, a move takes at least 1.2 s, thinking included: the decision stays on screen
for a moment, then the piece moves. A longer search (a time budget, depth 8) sets its own pace; scene 3
plays fast.

Point at a move anywhere on the page (the moves considered, the expected line, the game record, the search
tables, the tree, the capture routes to choose from) and the board shows it with an arrow, from the position where it was considered; a banner
names it. Move away and the game comes back.

## The search tree as a picture

T, the Search tree button (user view) or Open as a tree diagram (Tree tab) draws the search behind the last
agent move as a tree that branches from left to right (`js/treeview.js`): MAX nodes (the agent to move) and
MIN nodes (the opponent), each with its move and its value, ≤ and ≥ for bounds, the expected line in blue,
the move that cut a node off and how many moves were never searched, answers from the transposition table,
captures past the depth limit. It opens with every root move and the replies to the best one; click a node
to open or close it, drag to move, scroll (or + and −) to zoom, Fit or 0 for the whole tree, Esc to close.
The node under the mouse is shown on a small board with its window, value and why it ended where it did.
A decision made in the user view has no recorded tree, so it is searched again with a fresh agent (an
empty table), as the subtitle says; in the developer view the tree is the search that chose the move.

## Search budget

The Budget menu sets how long levels 3 to 5 (iterative deepening) search each move: positions (2,000, 20,000 as
in the ladder, 200,000), time (0.1 to 3 s) or depth (4, 6 or 8 plies; a depth search also stops at 1,000,000
positions, since the page waits for the engine). The Elo in the menus was measured at 20,000 positions; with
another budget the badge says so. Time budgets are offered when the engine accepts them (checked at start).
What if offers the same budgets. Scenes always use the ladder's budget, the one they were checked with.

The Speed tab (developer view) shows how far V10 gets in one minute natively (results/minute/minute.json) and
measures this browser for one second on the current position.

## Scenes for the demo

Keys 1 to 3 (or the Scenes menu) load three positions from our own games, each checked with the engine:

1. **One ply is blind**: V0, the A3 agent, plays e4-e5 and the reply takes three men and crowns a king.
2. **Quiescence and the horizon**: What if, with quiescence off, picks b4-c4 at +1.97; the full engine,
   searching captures past the depth limit, scores it at most −2.61. Walk the line on the board.
3. **Ahead, and still a draw**: V10 is a man and two kings ahead, yet ten plies later the 50-ply rule ends
   the game: its search does not apply that rule.

Each scene names its source game. The agents join after the moves with fixed seeds, so a scene plays the
same way every time. `tools/scenes.js` rebuilds `data/scenes.js` from replayed games (see its header).

## Files

| path | what |
|---|---|
| `index.html`, `css/site.css`, `fonts/`, `img/` | the page |
| `js/board.js` | the SVG board: pieces, highlights, arrows, labels, move animation, clicks |
| `js/treeview.js` | the search tree as a picture: layout, drawing, pan and zoom, the node's position |
| `js/app.js` | game flow, the user view and the developer view; every number comes from the engine calls |
| `pkg/` | the engine (`dama_web.js`, `dama_web_bg.wasm`), built from `web/` by `scripts/build-web.sh`; never edit |
| `data/results.js` | the Elo ladder, the ablation and one minute of search (`results/*/…json`); written by `node site/tools/data.js` |
| `tools/data.js` | writes `data/results.js`; run it after every `ladder rate` |
| `tools/bundle.js` | packs everything, the engine included, into `dist/dama.html` |
| `data/scenes.js`, `tools/scenes.js` | the demo scenes, and the script that finds and checks them |

## The five opponents

Our ladder versions, with their Elo from `results/ladder/ladder.json` shown in the menus:
Level 1 `tuned` (V0, the A3 agent), Level 2 `ab5` (V2), Level 3 `idqr:n20000` (V6),
Level 4 `idqr:n20000:w=texel1` (V8), Level 5 `idqrot:n20000:w=texel1` (V10, the full engine).
The what-if badges are the ablation from `results/ablation/ablation.json`: V10 against itself with one
part off, 400 games each.
