// Regenerates testdata/*.json from the JS prototype (rules engine + baseline agent).
// Usage, from the repository root:  node reference/js/make_testdata.js
// Formats are described in docs/reference.md. Everything is seeded, so the output is reproducible.
const fs = require('fs');
const path = require('path');
const D = require('./engine.js');
const A = require('./agent.js');
const mk = require('./rng.js');

const OUT = path.join(__dirname, '..', '..', 'testdata');
const TUNED = { man: 1, king: 2.5, mob: -0.05 };
const FIRST = { man: 1, king: 3, mob: 0.1 };

// ---------- formats ----------
const CH = { 1: 'w', 2: 'W', '-1': 'b', '-2': 'B', 0: '.' };
function boardString(b) {
  const rows = [];
  for (let r = 7; r >= 0; r--) { let s = ''; for (let c = 0; c < 8; c++) s += CH[b[r * 8 + c]]; rows.push(s); }
  return rows.join('/');
}
function parseBoard(s) {
  const rows = s.split('/'); const b = new Int8Array(64);
  rows.forEach((row, i) => { const r = 7 - i; [...row].forEach((ch, c) => { b[r * 8 + c] = { w: 1, W: 2, b: -1, B: -2, '.': 0 }[ch]; }); });
  return b;
}
const sideName = (s) => (s === 1 ? 'white' : 'black');
const moveList = (b, side) => D.moves(b, side).map(D.notation).sort();
const write = (name, obj) => { fs.writeFileSync(path.join(OUT, name), JSON.stringify(obj, null, 1) + '\n'); console.log('wrote testdata/' + name); };

// ---------- perft ----------
const PERFT = { 1: 8, 2: 64, 3: 708, 4: 7538, 5: 85090, 6: 931312, 7: 10782382, 8: 123290300, 9: 1454144462 };
for (let d = 1; d <= 6; d++) { const n = D.perft(D.initial(), 1, d); if (n !== PERFT[d]) throw new Error(`perft ${d}: ${n} != ${PERFT[d]}`); }
write('perft.json', {
  board: boardString(D.initial()), side: 'white', counts: PERFT,
  leafRule: 'a position with no legal move counts as one leaf',
  source: 'depths 1-8 equal the regression tests of kish 1.1.2 (crates.io/crates/kish); depth 9 from our JS engine only',
});

// ---------- rule cases ----------
const RULES = [
  ['king_lands_anywhere_beyond', 'A king may land on any empty square beyond the captured piece.', ['a4'], ['d4'], ['a4'], 1],
  ['king_no_180_up_down', 'No 180-degree turn between two captures (vertical).', ['d5'], ['d3', 'd7'], ['d5'], 1],
  ['king_no_180_left_right', 'No 180-degree turn between two captures (horizontal).', ['d4'], ['b4', 'f4'], ['d4'], 1],
  ['man_continues_as_man', 'A man that reaches the last row during a capture continues as a man and is crowned at the end.', ['d6'], ['d7', 'c8'], [], 1],
  ['sideways_capture_on_last_row', 'A man may capture sideways along the last row.', ['d6'], ['d7', 'e8'], [], 1],
  ['black_man_promotes', 'Black promotes on rank 1.', ['d2'], ['d3'], [], -1],
  ['king_moves_from_centre', 'A king flies orthogonally: 14 moves from d4 on an empty board.', ['d4'], ['h8'], ['d4'], 1],
  ['man_moves', 'A man moves one square forward or sideways, never backward.', ['d4'], ['h8'], [], 1],
  ['capture_is_mandatory', 'If a capture exists, only captures are legal.', ['d4'], ['d5'], [], 1],
  ['man_never_captures_backward', 'A man never captures backward.', ['d4'], ['d3', 'h8'], [], 1],
  ['longest_capture_only', 'Only the sequence that captures the most pieces is legal.', ['d4'], ['d5', 'd7', 'e4'], [], 1],
];
const rules = RULES.map(([name, rule, wh, bl, kings, side]) => {
  const b = D.pos(wh, bl, kings);
  return { name, rule, board: boardString(b), side: sideName(side), moves: moveList(b, side) };
});

// a capture whose path crosses the square of a piece it captured earlier (pieces are removed at once)
function crossesCaptured(m) {
  for (let j = 1; j < m.path.length; j++) {
    const a = m.path[j - 1], z = m.path[j];
    const dx = Math.sign((z & 7) - (a & 7)), dy = Math.sign((z >> 3) - (a >> 3));
    for (let s = a + dy * 8 + dx; s !== z; s += dy * 8 + dx) if (m.caps.slice(0, j - 1).includes(s)) return true;
  }
  return false;
}

// ---------- random games: positions for move generation ----------
function* randomPositions(seed, games) {
  const rnd = mk(seed);
  for (let g = 0; g < games; g++) {
    let b = D.initial(), side = 1;
    for (let ply = 0; ply < 300; ply++) {
      const ms = D.moves(b, side); if (!ms.length) break;
      yield { b, side, ms };
      b = D.apply(b, ms[Math.floor(rnd() * ms.length)]); side = -side;
    }
  }
}
const seen = new Set(); const buckets = { multiCapture: [], kingCapture: [], promotion: [], king: [], quiet: [] };
const QUOTA = { multiCapture: 120, kingCapture: 80, promotion: 40, king: 60, quiet: 100 };
let crossing = null;
const pick = mk(424242);
for (const { b, side, ms } of randomPositions(2026, 4000)) {
  const k = D.key(b, side); if (seen.has(k)) continue;
  if (!crossing) { const m = ms.find((x) => x.caps.length >= 3 && crossesCaptured(x)); if (m) crossing = { b, side, m }; }
  const hasKing = b.some((v) => Math.abs(v) === 2);
  const cap = ms[0].caps.length;
  const kind = cap >= 2 ? 'multiCapture' : (cap >= 1 && ms.some((m) => Math.abs(b[m.from]) === 2)) ? 'kingCapture'
    : ms.some((m) => m.promo) ? 'promotion' : hasKing ? 'king' : 'quiet';
  if (buckets[kind].length >= QUOTA[kind] || pick() > 0.08) continue;
  seen.add(k); buckets[kind].push({ kind, board: boardString(b), side: sideName(side), moves: ms.map(D.notation).sort() });
  if (Object.keys(QUOTA).every((q) => buckets[q].length >= QUOTA[q]) && crossing) break;
}
if (crossing) rules.push({ name: 'captured_pieces_removed_at_once', rule: 'Captured pieces leave the board during the sequence, so a later jump may cross their squares.',
  board: boardString(crossing.b), side: sideName(crossing.side), moves: moveList(crossing.b, crossing.side), example: D.notation(crossing.m) });
write('rules.json', rules);
write('movegen.json', Object.values(buckets).flat());

// ---------- baseline agent decisions ----------
function decisionEntry(name, b, side, w) {
  const { cands } = A.decide(b, side, w);
  const best = Math.max(...cands.map((c) => c.u));
  return {
    name, board: boardString(b), side: sideName(side), weights: w,
    candidates: cands.map((c) => ({ move: D.notation(c.m), men: c.men, kings: c.kings, mob: c.mob, win: !!c.win, u: c.u }))
      .sort((p, q) => q.u - p.u || (p.move < q.move ? -1 : 1)),
    best: cands.filter((c) => c.u === best).map((c) => D.notation(c.m)).sort(),
  };
}
const decisions = [decisionEntry('talk_slide7_game0_ply36', parseBoard('......../b.b...../.b....../......../......../.wwwwwww/.wwwwww./B.......'), 1, TUNED)];
{ // agent (tuned) vs a random player, both colours; sample about one decision in six
  const rnd = mk(7070), pickD = mk(8080); let tuned = 0, first = 0;
  for (let g = 0; g < 400 && (tuned < 100 || first < 40); g++) {
    const agentSide = g % 2 === 0 ? 1 : -1, w = g % 4 < 2 ? TUNED : FIRST;
    let b = D.initial(), side = 1;
    for (let ply = 0; ply < 200; ply++) {
      const ms = D.moves(b, side); if (!ms.length) break;
      let m;
      if (side === agentSide) {
        if (ms.length > 1 && pickD() < 0.17) {
          if (w === TUNED && tuned < 100) { decisions.push(decisionEntry(`agent_g${g}_ply${ply}`, b, side, w)); tuned++; }
          else if (w === FIRST && first < 40) { decisions.push(decisionEntry(`agent_g${g}_ply${ply}`, b, side, w)); first++; }
        }
        m = A.decide(b, side, w, rnd).best.m;
      } else m = ms[Math.floor(rnd() * ms.length)];
      b = D.apply(b, m); side = -side;
    }
  }
}
write('decisions.json', decisions);
console.log(`rules ${rules.length}, movegen ${Object.values(buckets).flat().length}, decisions ${decisions.length}`);
