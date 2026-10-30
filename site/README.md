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

Keys: Space play or pause, Right arrow one move, D switch view, U undo, F flip, N new game, Esc cancel.

## Files

| path | what |
|---|---|
| `index.html`, `css/site.css`, `fonts/`, `img/` | the page |
| `js/board.js` | the SVG board: pieces, highlights, arrows, labels, move animation, clicks |
| `js/app.js` | game flow, the user view and the developer view; every number comes from the engine calls |
| `pkg/` | the engine (`dama_web.js`, `dama_web_bg.wasm`), built from `web/` by `scripts/build-web.sh`; never edit |
| `data/results.js` | the Elo ladder and the ablation (`results/*/…json`); written by `node site/tools/data.js` |
| `tools/data.js` | writes `data/results.js`; run it after every `ladder rate` |
| `tools/bundle.js` | packs everything, the engine included, into `dist/dama.html` |

## The five opponents

Our ladder versions, with their Elo from `results/ladder/ladder.json` shown in the menus:
Level 1 `tuned` (V0, the A3 agent), Level 2 `ab5` (V2), Level 3 `idqr:n20000` (V6),
Level 4 `idqr:n20000:w=texel1` (V8), Level 5 `idqrot:n20000:w=texel1` (V10, the full engine).
The what-if badges are the ablation from `results/ablation/ablation.json`: V10 against itself with one
part off, 400 games each.
