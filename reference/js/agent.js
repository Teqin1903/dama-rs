// Assignment 3 prototype agent and match runner (JS, same rules engine as Assignment 2).
// Agent: one-ply hill climbing on U. U (Assignment 1) = material + kings + mobility.
const D = require('./engine.js');

const W0 = { man: 1, king: 3, mob: 0.1 };   // first-guess weights
const WIN = 1000;

function counts(b) {
  let wm = 0, wk = 0, bm = 0, bk = 0;
  for (let i = 0; i < 64; i++) {
    const v = b[i];
    if (v === 1) wm++; else if (v === 2) wk++; else if (v === -1) bm++; else if (v === -2) bk++;
  }
  return { wm, wk, bm, bk };
}
// U of board b for `side`, evaluated right after `side` has moved (so the opponent is to move)
function U(b, side, w = W0) {
  const oppMoves = D.moves(b, -side).length;
  if (oppMoves === 0) return { u: WIN, men: 0, kings: 0, mob: 0, win: true };
  const c = counts(b);
  const men = side === 1 ? c.wm - c.bm : c.bm - c.wm;
  const kings = side === 1 ? c.wk - c.bk : c.bk - c.wk;
  const mob = D.moves(b, side).length - oppMoves;
  return { u: w.man * men + w.king * kings + w.mob * mob, men, kings, mob };
}
// material from side's view, counting a king as w.king men
function material(b, side, w = W0) {
  const c = counts(b);
  const men = side === 1 ? c.wm - c.bm : c.bm - c.wm;
  const kings = side === 1 ? c.wk - c.bk : c.bk - c.wk;
  return w.man * men + w.king * kings;
}
// one-ply hill climbing: evaluate every successor with U, take the best (ties: first, or random if rnd given)
function decide(b, side, w = W0, rnd = null) {
  const ms = D.moves(b, side);
  const cands = ms.map((m) => { const nb = D.apply(b, m); return { m, nb, ...U(nb, side, w) }; });
  let bestU = -Infinity;
  for (const c of cands) if (c.u > bestU) bestU = c.u;
  const top = cands.filter((c) => c.u === bestU);
  const best = rnd ? top[Math.floor(rnd() * top.length)] : top[0];
  return { best, cands, ties: top.length };
}
const agentPolicy = (w = W0, rnd = null) => (b, side) => decide(b, side, w, rnd).best.m;
const randomPolicy = (rnd) => (b, side) => { const ms = D.moves(b, side); return ms[Math.floor(rnd() * ms.length)]; };

// Match settings (ours): threefold repetition, 50 plies without a capture, one piece each, 400-ply cap.
function playGame(white, black, o = {}) {
  let b = o.start ? Int8Array.from(o.start) : D.initial();
  let side = o.side || 1, ply = 0, quiet = 0;
  const seen = new Map();
  const rec = o.record ? [] : null;
  const pre = o.randomPlies || 0, rnd = o.rnd;
  for (;;) {
    const k = D.key(b, side);
    const n = (seen.get(k) || 0) + 1; seen.set(k, n);
    if (n >= 3) return { result: 0, reason: 'repetition', plies: ply, rec };
    const ms = D.moves(b, side);
    if (!ms.length) return { result: -side, reason: 'no move', plies: ply, rec };
    const c = counts(b);
    if (c.wm + c.wk === 1 && c.bm + c.bk === 1) return { result: 0, reason: 'one piece each', plies: ply, rec };
    if (quiet >= 50) return { result: 0, reason: '50 quiet plies', plies: ply, rec };
    if (ply >= (o.maxPlies || 400)) return { result: 0, reason: 'cap', plies: ply, rec };
    let m;
    if (ply < pre) m = ms[Math.floor(rnd() * ms.length)];
    else m = (side === 1 ? white : black)(b, side, ply);
    if (rec) rec.push({ b: b.slice(), side, m, ply });
    quiet = m.caps.length ? 0 : quiet + 1;
    b = D.apply(b, m); side = -side; ply++;
  }
}
module.exports = { D, W0, WIN, U, counts, material, decide, agentPolicy, randomPolicy, playGame };
