// Writes data/scenes.js: four positions from our own games for the live demo, each checked here
// with the engine (pkg/) exactly as the page uses it: the moves from the start, then fresh agents
// with fixed seeds. The games come from replaying two ablation pairings with move logs:
//   dama arena idqrot:n20000:w=texel1 tuned --games 160 --seed 1 --log DIR/v10_v0.tsv --csv DIR/a.csv
//   dama arena idqrot:n20000:w=texel1 idrot:n20000:w=texel1 --games 100 --seed 1 --log DIR/v10_v10q.tsv --csv DIR/a.csv
// (the same games as results/ablation/games/, which keep no moves), then: node site/tools/scenes.js DIR
'use strict';
const fs = require('fs');
const path = require('path');
const pkg = path.join(__dirname, '..', 'pkg');
const W = new Function(fs.readFileSync(path.join(pkg, 'dama_web.js'), 'utf8') + '\nreturn wasm_bindgen;')();
W.initSync({ module: fs.readFileSync(path.join(pkg, 'dama_web_bg.wasm')) });
const J = (s) => {
  const o = JSON.parse(s);
  if (o && o.error) throw new Error(o.error);
  return o;
};
// A session at the position after `moves` (played from the start).
function session(moves) {
  const s = new W.Session();
  for (const m of moves) J(s.play(m));
  return s;
}
function readLog(file) {
  const lines = fs.readFileSync(file, 'utf8').trim().split('\n');
  const head = lines[0].split('\t');
  return lines.slice(1).map((l) => {
    const f = l.split('\t');
    const o = {};
    head.forEach((h, i) => (o[h] = f[i]));
    o.game = +o.game;
    o.plies = +o.plies;
    o.moves = o.moves ? o.moves.split(' ') : [];
    return o;
  });
}

const V10 = 'idqrot:n20000:w=texel1';
const V11 = 'idqrotf:n20000:w=texel1:fs=1';
const dir = process.argv[2] || '.';
const logs = { v0: readLog(path.join(dir, 'v10_v0.tsv')), q: readLog(path.join(dir, 'v10_v10q.tsv')) };
const game = (set, id) => logs[set].find((g) => g.game === id);
const fmt = (v) => (v >= 0 ? '+' : '−') + Math.abs(v).toFixed(2);
const check = (ok, what) => { if (!ok) throw new Error('scene check failed: ' + what); console.log('  ok:', what); };

const scenes = [];

// 1. One ply is blind: V0 (the A3 agent) to move, game 33 of V10–V0, after the 4 random opening plies.
{
  const g = game('v0', 33), ply = 4;
  const s = session(g.moves.slice(0, ply));
  J(s.set_agent('white', 'tuned', 101)); J(s.set_agent('black', V10, 102));
  const r0 = J(s.think('white', '{}'));
  check(r0.best === 'e4-e5' && r0.ties.length === 1, `V0 plays e4-e5 and nothing ties with it (${r0.best}, ties ${r0.ties.length})`);
  J(s.play(r0.best));
  const r1 = J(s.think('black', '{}'));
  check(r1.best === 'f5xd5xd3xd1=K', `V10 answers f5xd5xd3xd1=K (${r1.best})`);
  const t = session(g.moves.slice(0, ply));
  const full = J(t.analyze(V10, '{"seed":1}'));
  console.log('  V10 in V0\'s place:', full.best, full.value.toFixed(2), 'ties', full.ties.join(' '));
  scenes.push({
    id: 'blind', key: '1', title: 'One ply is blind',
    note: 'V0, the A3 agent, scores only the board after its own move. Step: e4-e5 looks best to it. Step again: the reply takes three men and crowns a king.',
    source: 'V10–V0, game 33, after the 4 random opening plies · results/ablation/games/V10_vs_V0.csv',
    replay: 'dama arena idqrot:n20000:w=texel1 tuned --games 160 --seed 1 --log games.tsv',
    moves: g.moves.slice(0, ply), white: 'L1', black: 'L5', seeds: { white: 101, black: 102 }, mode: 'user',
  });
}

// 2. Quiescence and the horizon: game 9 of V10–V10 without quiescence, White to move at ply 20.
{
  const g = game('q', 9), ply = 20;
  const s = session(g.moves.slice(0, ply));
  const alt = J(s.analyze('idrot:n20000:w=texel1', '{"seed":1}'));
  const full = J(s.analyze(V10, '{"seed":1}'));
  check(alt.best === 'b4-c4' && full.best !== 'b4-c4', `without quiescence b4-c4 (${alt.best}, ${alt.value.toFixed(2)}), with it ${full.best} (${full.value.toFixed(2)})`);
  const pair = J(s.analyze(V10, JSON.stringify({ seed: 1, only: [alt.best, full.best] })));
  const a = pair.root.find((x) => x.move === alt.best), f = pair.root.find((x) => x.move === full.best);
  check(f.value - a.value > 2, `the full engine finds b4-c4 worse by more than 2 (${a.bound} ${a.value.toFixed(2)} against ${f.value.toFixed(2)}, depth ${pair.depth})`);
  scenes.push({
    id: 'horizon', key: '2', title: 'Quiescence and the horizon',
    note: `Quiescence off: the search stops right after White crowns a king and scores b4-c4 at ${fmt(alt.value)}. Searching captures past the limit, the full engine sees Black take back and crown too: at most ${fmt(a.value)}.`,
    source: 'V10–V10 without quiescence, game 9, White to move at ply 20 · results/ablation/games/V10_vs_V10-q.csv',
    replay: 'dama arena idqrot:n20000:w=texel1 idrot:n20000:w=texel1 --games 100 --seed 1 --log games.tsv',
    moves: g.moves.slice(0, ply), white: 'L5', black: 'L5', seeds: { white: 201, black: 202 }, mode: 'dev', tab: 'whatif', whatif: { q: false },
  });
}

// 3. Ahead, and still a draw: game 128 of V10–V0, 10 plies before the 50-ply rule ends it (drawn
// from here with each of 10 seed pairs tried).
{
  const g = game('v0', 128), ply = 67;
  const s = session(g.moves.slice(0, ply));
  J(s.set_agent('white', V10, 1000)); J(s.set_agent('black', 'tuned', 1001));
  let st = J(s.state());
  const start = { quiet: st.quiet_plies, pieces: st.pieces };
  let first = null;
  while (st.status === 'playing') {
    const r = J(s.think(st.turn, '{}'));
    if (!first && st.turn === 'white') first = r.value;
    st = J(s.play(r.best));
  }
  check(st.status === 'draw' && st.reason === 'fifty', `fresh agents draw by 50 quiet plies at ply ${st.ply} (start: ${start.quiet} quiet, ${JSON.stringify(start.pieces)}, V10 values it ${first.toFixed(2)})`);
  scenes.push({
    id: 'fifty', key: '3', title: 'Ahead, and still a draw',
    note: `Forty plies without a capture. V10 is a man and two kings ahead (${fmt(first)} in its own eyes), but its search does not apply the 50-ply rule. Ten plies later the game is drawn.`,
    source: 'V10–V0, game 128, drawn by the 50-ply rule at ply 77; here from ply 67 · results/ablation/games/V10_vs_V0.csv',
    replay: 'dama arena idqrot:n20000:w=texel1 tuned --games 160 --seed 1 --log games.tsv',
    moves: g.moves.slice(0, ply), white: 'L5', black: 'L1', seeds: { white: 1000, black: 1001 }, mode: 'user', speed: 60,
    expect: { status: 'draw', reason: 'fifty', ply: st.ply },
  });
}

// 4. The same position with V11 as White: its search applies the 50-ply rule and progress pressure, and it wins
// (docs/ladder.md, "Scene 3 played on": ply 91 with these seeds).
{
  const g = game('v0', 128), ply = 67;
  const s = session(g.moves.slice(0, ply));
  J(s.set_agent('white', V11, 1000)); J(s.set_agent('black', 'tuned', 1001));
  let st = J(s.state());
  while (st.status === 'playing') st = J(s.play(J(s.think(st.turn, '{}')).best));
  check(st.status === 'white', `V11 as White wins, at ply ${st.ply} (${st.reason})`);
  scenes.push({
    id: 'fixed', key: '4', title: 'The same position with V11',
    note: `Scene 3 again, with V11 as White. Its search knows the 50-ply rule, and a lead counts for less as the count grows, so it captures instead of moving its kings to and fro: White wins at ply ${st.ply}.`,
    source: 'V10–V0, game 128, from ply 67 as in scene 3; V11 = V10 with the draw rules in its search · docs/ladder.md, results/fifty/fifty.json',
    replay: 'dama arena idqrot:n20000:w=texel1 tuned --games 160 --seed 1 --log games.tsv',
    moves: g.moves.slice(0, ply), white: 'L6', black: 'L1', seeds: { white: 1000, black: 1001 }, mode: 'user', speed: 60,
    expect: { status: 'white', ply: st.ply },
  });
}

const out = '// Generated by a scene search over our own games; every scene was checked with the engine.\n'
  + '// Each entry: the moves from the start, the players (levels), their seeds, and the view to open.\n'
  + `window.SCENES = ${JSON.stringify(scenes, null, 1)};\n`;
fs.writeFileSync(path.join(__dirname, '..', 'data', 'scenes.js'), out);
console.log('wrote site/data/scenes.js', out.length, 'bytes');
