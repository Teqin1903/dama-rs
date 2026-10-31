// =====================================================================
// Assignment 4 deck: our agent, adversarial search and learning, measured.
// Every number comes from data.js (generated from results/ by tools/data.js) or from a file named in
// the slide's footer.
// =====================================================================
const SCENES = [];
const Dm = Dama;
const START = Dm.initial();
const VER = Object.fromEntries(LADDER.versions.map((v) => [v.id, v]));
const ABV = Object.fromEntries(ABLATION.versions.map((v) => [v.id, v]));
const ABP = Object.fromEntries(ABLATION.pairings.map((p) => [p.b, p]));
const signed = (x, d = 0) => (x >= 0 ? '+' : '−') + Math.abs(x).toFixed(d);
const roundAway = (x) => Math.sign(x) * Math.round(Math.abs(x)); // halves away from zero, as docs/ladder.md
const eloTxt = (x) => signed(roundAway(x));
const r0 = (x) => String(roundAway(x));
const ciTxt = (ci) => `[${ci.map((x) => (x < 0 ? '−' : '') + Math.round(Math.abs(x))).join(', ')}]`;
const FACT = (i) => FACTS.sets[i];
const legalLo = Math.min(FACT(0).legal_per_position, FACT(1).legal_per_position);
const legalHi = Math.max(FACT(0).legal_per_position, FACT(1).legal_per_position);
const capLo = Math.min(FACT(0).capture_share, FACT(1).capture_share);
const capHi = Math.max(FACT(0).capture_share, FACT(1).capture_share);
const V10V0 = ABP['V0'];

// ---------- shared pieces ----------
function card(x, y, w, h, o = {}) {
  const a = o.a ?? 1; if (a <= 0.002) return;
  ctx.save(); ctx.globalAlpha = a;
  rr(x, y, w, h, o.r || 24); ctx.fillStyle = o.fill || C.paper; ctx.fill();
  if (o.dash) ctx.setLineDash(o.dash);
  ctx.strokeStyle = o.stroke || C.hair; ctx.lineWidth = o.lw || 1.5; ctx.stroke();
  ctx.restore();
}
function checkItem(x, y, text, o = {}) {
  const a = o.a ?? 1, size = o.size || 21;
  if (a <= 0.002) return;
  const icx = x + 12, icy = y - size * 0.34;
  fillCircle(icx, icy, 12, o.icon || C.turq, a);
  check(icx, icy + 1, 11, C.paper, 1, 2.4);
  ctx.save(); ctx.globalAlpha = a; ctx.restore();
  if (o.wrap) TW(text, x + 36, y, o.wrap, size * 1.3, { size, w: o.w || 500, c: o.c || C.ink, a });
  else T(text, x + 36, y, { size, w: o.w || 500, c: o.c || C.ink, a });
}
function dotItem(x, y, text, o = {}) {
  const a = o.a ?? 1, size = o.size || 22;
  if (a <= 0.002) return 0;
  fillCircle(x + 7, y - size * 0.34, 5, o.dot || C.cobalt, a);
  return TW(text, x + 26, y, o.wrap || 1400, size * 1.32, { size, w: o.w || 400, c: o.c || C.ink, a });
}
function bigStat(x, y, value, caption, o = {}) {
  const a = o.a ?? 1; if (a <= 0.002) return;
  T(value, x, y, { f: 'serif', size: o.size || 76, c: o.c || C.cobalt, al: 'center', a });
  wrapLines(caption, o.wrap || 360, { size: o.csize || 20 }).forEach((ln, i) => T(ln, x, y + 44 + i * 27, { size: o.csize || 20, c: C.ink2, al: 'center', a }));
}
function centreLines(lines, x, y, lh, o) { lines.forEach((ln, i) => T(ln, x, y + i * lh, { ...o, al: 'center' })); }
function backupChrome(label, title, A, foot) {
  const pa = A(0, 0, 0.5);
  T(title, 120, 132, { f: 'serif', size: 54, c: C.ink, a: pa });
  tag(label, 1800, 96, { size: 18, al: 'right', fill: C.paper, stroke: C.hair, c: C.ink2, a: pa });
  if (foot) { let fs = 18; while (fs > 13 && measure(foot, { size: fs }) > 1560) fs -= 0.5; T(foot, 120, 1040, { size: fs, c: C.ink3, a: pa }); }
}

// =====================================================================
// 1. Our agent on one page
// =====================================================================
const S1 = {
  left: [
    { y: 214, label: 'Week 03 · Assignment 1 · Ch 1–2', topic: 'Intelligent agents',
      items: ['A utility-based agent: U scores each board', 'Fully observable, deterministic, two players'] },
    { y: 464, label: 'Weeks 04–05 · Assignment 2 · Ch 3', topic: 'Solving problems by searching',
      items: ['Longest capture inside a move, by DFS', 'Move generator checked by perft, depths 1–8'] },
    { y: 714, label: 'Weeks 06–07 · Assignment 3 · Ch 4', topic: 'Search in complex environments',
      items: ['Weights of U tuned by local search', 'What does not apply: chance, hiding, online'] },
  ],
  right: [
    { y: 214, h: 390, label: 'Weeks 09–10 · Assignment 4 · Ch 6', topic: 'Adversarial search and games',
      items: ['Minimax, then alpha–beta, then iterative deepening', 'Quiescence: captures past the depth limit', 'Repetitions, move ordering, transposition table', 'Rust: 20,000 positions a move, 5.7 plies deep', 'Monte Carlo tree search tried: below alpha–beta'] },
    { y: 624, h: 340, label: 'Weeks 11–13 · Assignment 4 · Ch 19, 22, 23', topic: 'Learning',
      items: ['Weights fitted to the results of 3,000 games', 'A better fit is not better play: games decide', 'Next: a network of our own, TD self-play'] },
  ],
};
SCENES.push({
  beats: [16, 14, 15],
  notes: [
    'Our agent on one page. Assignments 1 to 3 gave it its frame: a utility-based agent, a move generator checked against a referee, and weights tuned by local search.',
    'This assignment adds the two cards on the right: adversarial search, from minimax to a transposition table, and weights learned from our own games.',
    'The result: 672 Elo above minimax at three plies, and against our Assignment 3 agent 313 wins, 87 draws and no loss in 400 games.',
  ],
  render(beat, t) {
    const A = mkA(beat, t);
    chrome(1, 'Our agent on one page', A, `Elo: one Bradley–Terry fit of the 6,800 ladder games, 95% bootstrap interval (results/ladder/ladder.json). V10 against V0: results/ablation/ablation.json. 5.7 plies: V10's average, same file.`);
    const pa = A(0, 0, 0.5);
    T('Introduction to AI · Assignment 4 · Turkish draughts', 120, 176, { size: 22, c: C.ink2, a: pa });
    // team, top right
    drawLogo(1424, 62, 76, pa);
    T('Team Eight Bits', 1516, 96, { size: 26, w: 600, a: pa });
    tag('Pair 42', 1735, 88, { size: 17, stroke: C.cobalt, c: C.cobalt, a: pa });
    T('Hüseyin Bozan · Murat Çevlik', 1516, 130, { size: 20, c: C.ink2, a: pa });
    // earlier assignments
    S1.left.forEach((b, k) => {
      const p = A(0, 0.15 + 0.12 * k, 0.6);
      card(100, b.y, 600, 230, { a: p });
      T(b.label, 136, b.y + 44, { size: 18, w: 500, c: C.ink3, a: p });
      T(b.topic, 136, b.y + 90, { f: 'serif', size: 30, a: p });
      b.items.forEach((it, i) => checkItem(136, b.y + 146 + i * 46, it, { size: 20, a: p, icon: C.ink3 }));
    });
    // this assignment
    S1.right.forEach((b, k) => {
      const p = A(1, 0.1 + 0.18 * k, 0.6);
      card(1220, b.y, 600, b.h, { a: p, stroke: C.cobalt, lw: 2.5 });
      T(b.label, 1256, b.y + 44, { size: 18, w: 500, c: C.cobalt, a: p });
      T(b.topic, 1256, b.y + 90, { f: 'serif', size: 30, a: p });
      b.items.forEach((it, i) => checkItem(1256, b.y + 148 + i * 50, it, { size: 20, a: p, icon: k === 1 && i === 2 ? C.ink3 : C.turq }));
    });
    // board and headline numbers
    const g = BG(810, 226, 300);
    boardBase(g, { a: A(0, 0, 0.6) });
    boardPieces(g, START, { a: A(0, 0.1, 0.6) });
    const ps = A(2, 0, 0.6);
    bigStat(960, 640, eloTxt(VER.V10.elo), `Elo over minimax at 3 plies (V1), ${ciTxt(VER.V10.ci)}`, { a: ps, wrap: 380 });
    bigStat(960, 838, `${V10V0.w}–${V10V0.d}–${V10V0.l}`, `wins, draws, losses against our A3 agent, ${V10V0.games} games`, { a: A(2, 0.25, 0.6), wrap: 380, c: C.turq });
  },
});

// =====================================================================
// 2. Ten versions on one scale (progress since A3)
// =====================================================================
const S2 = {
  ids: ['V0', 'V1', 'V2', 'V3', 'V4', 'V6', 'V8', 'V9', 'V10'],
  label: { V0: ['one ply', '(A3 agent)'], V1: ['minimax', '3 plies'], V2: ['alpha–beta', '5 plies'], V3: ['iterative', 'deepening'], V4: ['quiescence', ''], V6: ['repetitions', ''], V8: ['Texel', 'weights'], V9: ['move', 'ordering'], V10: ['transposition', 'table'] },
  x0: 220, x1: 1440, y0: 860, y1: 250, lo: -100, hi: 750,
};
const s2x = (i) => S2.x0 + (i * (S2.x1 - S2.x0)) / (S2.ids.length - 1);
const s2y = (e) => S2.y0 - ((e - S2.lo) * (S2.y0 - S2.y1)) / (S2.hi - S2.lo);
SCENES.push({
  beats: [22, 22, 14],
  notes: [
    'Every version since Assignment 3 played under the same conditions: the same openings with both colours, 20,000 positions a move. One fit puts all of them on one scale, with minimax at three plies as zero.',
    'Each step is one idea from chapter 6. Iterative deepening alone lost 26: it only paid off once move ordering used it. The biggest single step is the Texel weights, plus 250.',
    'And the side rung, V8b: our second tuning fitted the games better and played 120 Elo worse. We come back to that under learning.',
  ],
  render(beat, t) {
    const A = mkA(beat, t);
    chrome(2, 'Since Assignment 3: ten versions on one scale', A, `Bradley–Terry fit of all ${fmt(LADDER.pairings.reduce((s, p) => s + p.games, 0))} games of ${LADDER.pairings.length} pairings (draws count half), V1 at 0; whiskers: 95% bootstrap interval. results/ladder/ladder.json, version ${LADDER.meta.commit}.`);
    const pa = A(0, 0, 0.6);
    // axis and grid
    for (let e = 0; e <= 700; e += 100) {
      gridLine(S2.x0 - 60, s2y(e), S2.x1 + 60, s2y(e), pa);
      T(e === 0 ? '0' : `+${e}`, S2.x0 - 74, s2y(e) + 7, { size: 17, c: C.ink3, al: 'right', a: pa });
    }
    T('Elo over V1', S2.x0 - 74, S2.y1 - 26, { size: 17, w: 600, c: C.ink3, al: 'right', a: pa });
    seg(S2.x0 - 60, s2y(0), S2.x1 + 60, s2y(0), C.ink3, 1.6, pa);
    // line through the main versions
    const pl = A(0, 0.3, 1.2, E.io);
    polyPartial(S2.ids.map((id, i) => [s2x(i), s2y(VER[id].elo)]), pl, C.ink2, 2.5, pa);
    // side rung V8b, between V8 and V9
    const pb = A(2, 0, 0.6);
    const xb = (s2x(6) + s2x(7)) / 2, yb = s2y(VER.V8b.elo);
    seg(s2x(6), s2y(VER.V8.elo), xb, yb, C.coral, 2, pb * 0.8);
    seg(xb, s2y(VER.V8b.ci[0]), xb, s2y(VER.V8b.ci[1]), C.coral, 2.5, pb);
    fillCircle(xb, yb, 9, C.coral, pb);
    T(`V8b ${eloTxt(VER.V8b.elo)}`, xb + 16, yb + 6, { size: 18, w: 600, c: C.coral, a: pb });
    TW(`second tuning: a better fit, ${r0(VER.V8.elo - VER.V8b.elo)} worse`, xb + 16, yb + 32, 300, 22, { size: 16, c: C.coral, a: pb });
    // points, intervals, labels
    S2.ids.forEach((id, i) => {
      const v = VER[id], x = s2x(i), y = s2y(v.elo);
      const p = A(0, 0.25 + 0.08 * i, 0.5);
      seg(x, s2y(v.ci[0]), x, s2y(v.ci[1]), C.cobalt, 2.5, p);
      fillCircle(x, y, 9, id === 'V10' ? C.turq : C.cobalt, p);
      T(id === 'V1' ? '0' : eloTxt(v.elo), x, y - 22, { size: 18, w: 600, c: C.ink, al: 'center', a: p });
      T(id, x, S2.y0 + 40, { size: 21, w: 600, al: 'center', a: p });
      S2.label[id].forEach((ln, k) => T(ln, x, S2.y0 + 66 + k * 22, { size: 16, c: C.ink3, al: 'center', a: p }));
      T(`${v.avg_depth.toFixed(1)} plies`, x, S2.y0 + 126, { size: 15, w: 500, c: C.cobalt, al: 'center', a: p * 0.9 });
      // step over the previous version
      if (i > 0) {
        const ps = A(1, 0.1 + 0.06 * i, 0.5);
        const d = v.elo - VER[S2.ids[i - 1]].elo;
        const xm = (x + s2x(i - 1)) / 2, ym = (y + s2y(VER[S2.ids[i - 1]].elo)) / 2;
        tag(eloTxt(d), xm, ym + (d >= 0 ? 34 : -30), { size: 16, fill: d >= 0 ? 'rgba(23,152,143,0.12)' : 'rgba(201,58,43,0.12)', c: d >= 0 ? '#0F6E68' : C.coral, al: 'center', a: ps });
      }
    });
    T('average depth', S2.x0 - 74, S2.y0 + 126, { size: 15, w: 500, c: C.cobalt, al: 'right', a: pa });
    // conditions
    const pc = A(0, 0.2, 0.6);
    card(1560, 250, 260, 356, { a: pc });
    T('Same conditions', 1586, 292, { size: 20, w: 600, a: pc });
    [['6,800', 'games'], ['25', 'pairings'], ['same openings,', 'both colours'], ['20,000', 'positions a move']].forEach(([a1, b1], k) => {
      T(a1, 1586, 344 + k * 64, { size: 24, w: 600, c: C.cobalt, a: pc });
      T(b1, 1586, 368 + k * 64, { size: 17, c: C.ink2, a: pc });
    });
  },
});

// =====================================================================
// 3. Ablation: what each part is worth now
// =====================================================================
const S3 = {
  rows: [
    { id: 'V10-w', name: 'Texel weights', sub: 'the A3 weights instead' },
    { id: 'V10-q', name: 'Quiescence', sub: 'captures past the limit' },
    { id: 'V10-o', name: 'Move ordering', sub: 'killers, history, best first' },
    { id: 'V9', name: 'Transposition table', sub: 'V9: no table' },
    { id: 'V10-r', name: 'Repetitions', sub: 'a repeated position is 0' },
  ],
  added: { 'V10-q': ['V4', 'V3'], 'V10-r': ['V6', 'V4'], 'V10-w': ['V8', 'V6'], 'V10-o': ['V9', 'V8'], V9: ['V10', 'V9'] },
  x: 560, w: 560, max: 420,
};
const s3w = (e) => (Math.max(0, e) * S3.w) / S3.max;
SCENES.push({
  beats: [20, 18, 20],
  notes: [
    'Then the other direction: we switched each part of V10 off and played 400 games against the full agent. The weights and quiescence carry most of it; repetitions are worth nothing measurable any more.',
    'Added and removed are not the same number. Quiescence brought 53 when we added it, and costs 265 when we take it out of the full agent.',
    'So we tested why: with the old weights quiescence is still worth 53. The Texel weights were fitted on quiet positions only, so without quiescence they judge positions they never saw.',
  ],
  render(beat, t) {
    const A = mkA(beat, t);
    chrome(3, 'Ablation: switch one part off', A, `V10 against V10 with one part off, 400 games each, the ladder's settings; Elo lost with a 95% bootstrap interval. results/ablation/ablation.json (${ABLATION.meta.commit}); quiescence with the A3 weights: results/arena.csv (${Q_TUNED.version}).`);
    const pa = A(0, 0, 0.6);
    T('Elo the full agent (V10) loses', S3.x, 214, { size: 18, w: 600, c: C.ink3, a: pa });
    for (let e = 0; e <= 400; e += 100) {
      gridLine(S3.x + s3w(e), 232, S3.x + s3w(e), 760, pa * 0.8);
      T(String(e), S3.x + s3w(e), 784, { size: 16, c: C.ink3, al: 'center', a: pa });
    }
    const row = (k, id, name, sub, p, colour) => {
      const v = ABV[id], y = 262 + k * 96;
      const lost = -v.elo, lo = -v.ci[1], hi = -v.ci[0];
      T(name, 120, y + 30, { size: 24, w: 600, a: p });
      T(sub, 120, y + 58, { size: 17, c: C.ink3, a: p });
      ctx.save(); ctx.globalAlpha = p; rr(S3.x, y + 12, s3w(lost) * E.out(p), 40, 6); ctx.fillStyle = colour; ctx.fill(); ctx.restore();
      seg(S3.x + s3w(lo), y + 32, S3.x + s3w(hi), y + 32, C.ink, 2, p * 0.8);
      seg(S3.x + s3w(lo), y + 22, S3.x + s3w(lo), y + 42, C.ink, 2, p * 0.8);
      seg(S3.x + s3w(hi), y + 22, S3.x + s3w(hi), y + 42, C.ink, 2, p * 0.8);
      const sig = v.ci[1] < 0;
      T(`${r0(lost)}${sig ? '' : ', not significant'}`, S3.x + Math.max(s3w(hi), s3w(lost)) + 14, y + 40, { size: 20, w: 600, c: sig ? C.ink : C.ink3, a: p });
    };
    S3.rows.forEach((r, k) => row(k, r.id, r.name, r.sub, A(0, 0.2 + 0.1 * k, 0.7), k < 2 ? C.cobalt : 'rgba(39,71,163,0.55)'));
    // everything since A3
    const pv = A(0, 0.8, 0.7);
    seg(120, 812, 1160, 812, C.hair, 1.5, pv);
    const v0 = ABV.V0, yv = 836;
    T('Everything since A3', 120, yv + 30, { size: 24, w: 600, a: pv });
    T(`V0, the A3 agent: +${V10V0.w} =${V10V0.d} −${V10V0.l}`, 120, yv + 58, { size: 17, c: C.ink3, a: pv });
    ctx.save(); ctx.globalAlpha = pv; rr(S3.x, yv + 12, s3w(-v0.elo) * E.out(pv), 40, 6); ctx.fillStyle = C.turq; ctx.fill(); ctx.restore();
    T(r0(-v0.elo), S3.x + s3w(-v0.ci[0]) + 14, yv + 40, { size: 20, w: 600, a: pv });
    seg(S3.x + s3w(-v0.ci[1]), yv + 32, S3.x + s3w(-v0.ci[0]), yv + 32, C.ink, 2, pv * 0.8);
    // added vs removed
    const pt = A(1, 0, 0.6);
    card(1260, 214, 560, 470, { a: pt });
    T('Added, then removed', 1292, 260, { size: 24, w: 600, a: pt });
    T('part', 1292, 304, { size: 16, w: 600, c: C.ink3, a: pt });
    T('when added', 1600, 304, { size: 16, w: 600, c: C.ink3, al: 'right', a: pt });
    T('removed now', 1788, 304, { size: 16, w: 600, c: C.ink3, al: 'right', a: pt });
    ['V10-q', 'V10-r', 'V10-w', 'V10-o', 'V9'].forEach((id, k) => {
      const y = 350 + k * 52, [hiV, loV] = S3.added[id];
      const hot = id === 'V10-q' && beat >= 2;
      if (hot) { ctx.save(); ctx.globalAlpha = pt; rr(1276, y - 32, 528, 46, 10); ctx.fillStyle = 'rgba(39,71,163,0.10)'; ctx.fill(); ctx.restore(); }
      T(S3.rows.find((r) => r.id === id).name, 1292, y, { size: 20, w: hot ? 600 : 500, a: pt });
      T(eloTxt(VER[hiV].elo - VER[loV].elo), 1600, y, { size: 20, w: 600, c: '#0F6E68', al: 'right', a: pt });
      T(`${r0(-ABV[id].elo)}`, 1788, y, { size: 20, w: 600, c: C.cobalt, al: 'right', a: pt });
    });
    TW('Added: the ladder step it made. Removed: what V10 loses without it.', 1292, 640, 500, 22, { size: 16, c: C.ink3, a: pt });
    // the test: quiescence with the A3 weights
    const pq = A(2, 0, 0.6);
    card(1260, 708, 560, 260, { a: pq, stroke: C.cobalt, lw: 2.5 });
    T('Why? Quiescence with the A3 weights', 1292, 752, { size: 21, w: 600, a: pq });
    const qx = 1292, qw = 200;
    [[`A3 weights`, Q_TUNED.elo, Q_TUNED.ci], ['Texel weights', -ABV['V10-q'].elo, [-ABV['V10-q'].ci[1], -ABV['V10-q'].ci[0]]]].forEach(([nm, e, ci], k) => {
      const y = 790 + k * 58;
      T(nm, qx, y + 26, { size: 18, a: pq });
      const bx = qx + 136, bw = (e / 300) * qw;
      ctx.save(); ctx.globalAlpha = pq; rr(bx, y + 6, bw * E.out(pq), 30, 5); ctx.fillStyle = k ? C.cobalt : C.turq; ctx.fill(); ctx.restore();
      T(`+${r0(e)} ${ciTxt(ci)}`, bx + bw + 10, y + 28, { size: 17, w: 600, a: pq });
    });
    TW('The Texel weights were fitted on quiet positions only.', 1292, 940, 500, 22, { size: 17, c: C.ink2, a: pq });
  },
});

// =====================================================================
// 4. Live demo
// =====================================================================
SCENES.push({
  beats: [210],
  notes: [
    'Over to the demo (about three and a half minutes): user view, developer view, then the three scenes with keys 1, 2 and 3. Come back with the next slide.',
  ],
  render(beat, t) {
    const A = mkA(beat, t);
    chrome(4, 'Live demo: what it considers, and why', A, 'site/dist/dama.html: the Rust engine compiled to WebAssembly, in one file that opens without a server. Scenes: site/data/scenes.js, each from a replayed game of results/ablation/games.');
    const p1 = A(0, 0.1, 0.6), p2 = A(0, 0.3, 0.6), p3 = A(0, 0.5, 0.6);
    card(120, 220, 800, 460, { a: p1 });
    T('User view', 160, 280, { f: 'serif', size: 36, a: p1 });
    ['The moves it considered, each with its value', 'Upper bounds: what alpha–beta never had to finish', 'Why it plays the one it plays, and the line it expects', 'Five opponents: our versions V0, V2, V6, V8 and V10'].forEach((s, k) => checkItem(160, 350 + k * 70, s, { size: 23, a: p1 }));
    card(1000, 220, 800, 460, { a: p2, stroke: C.cobalt, lw: 2.5 });
    T('Developer view', 1040, 280, { f: 'serif', size: 36, a: p2 });
    ['Search: iterative deepening, one row per depth', 'Tree: any node, with its window and its cutoffs', 'Evaluation: each feature, each piece', 'What if: switch a part off in this very position'].forEach((s, k) => checkItem(1040, 350 + k * 70, s, { size: 23, a: p2 }));
    T('Scenes from our own games', 120, 760, { size: 24, w: 600, a: p3 });
    [['1', 'One ply is blind'], ['2', 'Quiescence and the horizon'], ['3', 'Ahead, and still a draw']].forEach(([k, s], i) => {
      const x = 120 + i * 560;
      card(x, 790, 520, 110, { a: p3 });
      ctx.save(); ctx.globalAlpha = p3; rr(x + 28, 818, 54, 54, 12); ctx.fillStyle = C.cobalt; ctx.fill(); ctx.restore();
      T(k, x + 55, 857, { size: 30, w: 600, c: C.paper, al: 'center', a: p3 });
      T(s, x + 104, 856, { size: 24, w: 600, a: p3 });
    });
  },
});

// =====================================================================
// 5. Why alpha-beta, not Monte Carlo tree search? (A3 feedback; measured: results/mcts)
// =====================================================================
const S5 = {
  forms: [['MC', 'Flat Monte Carlo', 'random games'], ['UCT', 'UCT', 'random games'], ['UCTe', 'UCT', 'texel1 at the leaves'], ['UCTq', 'UCT', 'texel1 + quiescence']],
  opp: ['V0', 'V2', 'V10'],
};
const mctsPair = (a, b) => MCTS.pairings.find((p) => p.a === a && p.b === b);
// Score in % to one decimal, halves to even, as docs/ladder.md prints it (Rust's {:.1}).
const scorePct = (p) => { const tenths = ((p.w + p.d / 2) * 1000) / p.games; const f = Math.floor(tenths); const r = Math.abs(tenths - f - 0.5) < 1e-9 ? (f % 2 ? f + 1 : f) : Math.round(tenths); return `${(r / 10).toFixed(1)}%`; };
const scoreFill = (s) => (s >= 0.5 ? `rgba(23,152,143,${0.10 + 0.5 * (s - 0.5)})` : `rgba(201,58,43,${0.10 + 0.5 * (0.5 - s)})`);
SCENES.push({
  beats: [18, 22, 18],
  notes: [
    'You asked last time whether we tried other approaches, like Monte Carlo tree search. The book\'s test first: Monte Carlo pays off when moves are many and a good evaluation is hard to write. Our positions have 13 to 16 moves, material judges them well, and captures are compulsory.',
    'So we tried it: four forms of Monte Carlo tree search, from flat random games to UCT with our evaluation and quiescence, 200 games each against V0, V2 and V10, at the same 20,000 positions a move. Every form beats our A3 agent without losing a game, and every form loses to plain alpha–beta at five plies.',
    'The best form scores 10.5 percent against V10. And quiescence, worth 265 to alpha–beta, is worth nothing to Monte Carlo: alpha–beta drops a move after one refutation, while UCT needs many visits to lower an average. That is why we chose alpha–beta.',
  ],
  render(beat, t) {
    const A = mkA(beat, t);
    const games = MCTS.pairings.reduce((n, p) => n + p.games, 0);
    chrome(5, 'Why alpha–beta, and not Monte Carlo tree search?', A, `Monte Carlo: four forms, 20,000 positions a move, ${MCTS.pairings[0].games} games a pairing, paired openings; results/mcts/mcts.json (${MCTS.meta.commit}), docs/ladder.md. Moves, captures: deck/facts.json. AIMA 4th ed. §6.4.`);
    // the book's test, against our game
    const pa = A(0, 0.1, 0.6);
    T('Monte Carlo search pays off when…', 120, 220, { size: 22, w: 600, c: C.ink2, a: pa });
    const rows = [
      ['moves are many', `${legalLo.toFixed(0)}–${legalHi.toFixed(0)}`, 'legal moves a position in our games; Go starts at 361'],
      ['a good evaluation is hard to write', 'material', 'judges most positions; Texel weights add the rest'],
      ['no single move decides', `${Math.round(capLo * 100)}–${Math.round(capHi * 100)}%`, 'of positions force a capture; one missed reply loses men'],
    ];
    rows.forEach(([cond, big, note], k) => {
      const p = A(0, 0.2 + 0.15 * k, 0.6), y = 246 + k * 166;
      card(120, y, 860, 146, { a: p });
      T(`…${cond}`, 152, y + 44, { size: 22, w: 600, a: p });
      cross(930, y + 38, 22, C.coral, p, 4.5);
      T(big, 152, y + 110, { f: 'serif', size: 46, c: C.cobalt, a: p });
      const wb = measure(big, { f: 'serif', size: 46 });
      TW(note, 152 + wb + 22, y + 100, 860 - 90 - wb, 25, { size: 19, c: C.ink2, a: p });
    });
    TW('AIMA §6.4: Monte Carlo search has the edge when the branching factor is high or a good evaluation function is hard to define, and a weakness when a single move can change the course of the game.', 120, 778, 860, 27, { size: 19, c: C.ink2, a: A(0, 0.7, 0.6) });
    // what we measured
    const pg = A(1, 0, 0.6);
    card(1040, 214, 780, 540, { a: pg, stroke: C.cobalt, lw: 2.5 });
    T(`We tried it: ${games.toLocaleString('en-US')} games`, 1072, 262, { f: 'serif', size: 30, a: pg });
    T('score of the Monte Carlo form against', 1452, 304, { size: 16, w: 600, c: C.ink3, al: 'center', a: pg });
    const cx = (j) => 1388 + j * 136;
    S5.opp.forEach((o, j) => {
      T(o, cx(j) + 60, 338, { size: 20, w: 600, al: 'center', a: pg });
      T(`ladder ${eloTxt(VER[o].elo)}`, cx(j) + 60, 360, { size: 15, c: C.ink3, al: 'center', a: pg });
    });
    S5.forms.forEach(([id, name, sub], i) => {
      const y = 384 + i * 76, p = A(1, 0.15 + 0.1 * i, 0.5);
      T(name, 1072, y + 32, { size: 20, w: 600, a: p });
      T(sub, 1072, y + 56, { size: 16, c: C.ink3, a: p });
      S5.opp.forEach((o, j) => {
        const q = mctsPair(id, o);
        ctx.save(); ctx.globalAlpha = p; rr(cx(j), y + 6, 120, 62, 10); ctx.fillStyle = scoreFill(q.score); ctx.fill(); ctx.restore();
        T(scorePct(q), cx(j) + 60, y + 38, { size: 22, w: 600, al: 'center', a: p });
        T(`${eloTxt(q.elo_diff)} Elo`, cx(j) + 60, y + 60, { size: 14, c: C.ink2, al: 'center', a: p });
      });
    });
    TW('Every form beats our A3 agent without losing a game, and loses to plain alpha–beta at 5 plies (V2).', 1072, 696, 720, 24, { size: 18, c: C.ink2, a: A(1, 0.6, 0.6) });
    // why
    const pw = A(2, 0, 0.6);
    const qUctq = mctsPair('UCTq', 'V10'), qUcte = mctsPair('UCTe', 'V10');
    card(1040, 774, 780, 190, { a: pw });
    T('What quiescence is worth', 1072, 814, { size: 21, w: 600, a: pw });
    T(`to alpha–beta: ${r0(-ABV['V10-q'].elo)} Elo;  to Monte Carlo: ${scorePct(qUcte)} → ${scorePct(qUctq)} against V10, within the intervals`, 1072, 848, { size: 18, c: C.ink2, a: pw });
    TW('Alpha–beta drops a move after one refutation; UCT needs many visits to lower a mean.', 1072, 892, 720, 26, { size: 20, w: 500, a: pw });
  },
});

// =====================================================================
// 6. Learning the weights from our own games (Ch 19, 23, 4)
// =====================================================================
const S6 = {
  boxes: [
    { x: 120, title: '3,000 games', text: 'four of our agents, round robin', ch: '' },
    { x: 560, title: 'quiet positions', text: 'labelled with the result: 1, ½, 0', ch: 'Ch 23: value from the outcome' },
    { x: 1000, title: 'fit σ(K · w · f)', text: 'squared error; every fifth pair kept for validation', ch: 'Ch 19: regression' },
    { x: 1440, title: 'texel1', text: 'one weight at a time, ± a step', ch: 'Ch 4: local search' },
  ],
  weights: [['king', 2.5134], ['cen', -0.3085], ['back', -0.3041], ['adv', 0.0831], ['qmob', 0.0813], ['edge', -0.0226], ['mob', 0.0148]],
};
SCENES.push({
  beats: [24, 22],
  notes: [
    'The Texel weights are our learning part. Each quiet position of 3,000 games gets the game result as its label, so its value is estimated from how the game ended: direct utility estimation, chapter 23. We fit a sigmoid of the evaluation to those labels, regression from chapter 19, with a local search over the weights from chapter 4.',
    'The validation error hardly moved, yet the agent gained 250 Elo. And a second tuning on 6,000 games fitted better and played 120 worse. The loss is only a stand-in: games decide.',
  ],
  render(beat, t) {
    const A = mkA(beat, t);
    chrome(6, 'Learning the weights from our own games', A, 'Texel tuning (Österlund 2014, Chess Programming Wiki): dama texel --seed 1, 3,000 games; second run seed 2, 6,000 games. Errors and weights: docs/decisions.md (2026-10-28), engine/src/eval.rs. Elo: results/ladder/ladder.json.');
    S6.boxes.forEach((b, k) => {
      const p = A(0, 0.1 + 0.15 * k, 0.6);
      card(b.x, 220, 360, 190, { a: p, stroke: k === 3 ? C.cobalt : C.hair, lw: k === 3 ? 2.5 : 1.5 });
      T(b.title, b.x + 30, 278, k === 3 ? { f: 'mono', size: 30, w: 600, a: p } : { f: 'serif', size: 32, a: p });
      TW(b.text, b.x + 30, 320, 300, 26, { size: 20, c: C.ink2, a: p });
      if (b.ch) tag(b.ch, b.x + 180, 446, { size: 17, fill: 'rgba(39,71,163,0.10)', c: C.cobalt, al: 'center', a: p });
      if (k < 3) curve(b.x + 368, 315, b.x + 432, 315, { c: C.ink3, w: 2.5, head: 14, a: p, p: A(0, 0.2 + 0.15 * k, 0.5) });
    });
    // weights of texel1
    const pw = A(0, 0.8, 0.6);
    card(120, 500, 700, 450, { a: pw });
    T('texel1, from White minus Black (man = 1)', 156, 548, { size: 20, w: 600, a: pw });
    const bx = 470, sc = 120;
    seg(bx, 572, bx, 920, C.ink3, 1.5, pw);
    S6.weights.forEach(([k, v], i) => {
      const y = 600 + i * 46;
      T(k, 156, y + 8, { size: 20, f: 'mono', a: pw });
      T(v.toFixed(4).replace('-', '−'), 400, y + 8, { size: 18, f: 'mono', c: C.ink2, al: 'right', a: pw });
      const w = Math.max(-2.8, Math.min(2.8, v)) * sc;
      ctx.save(); ctx.globalAlpha = pw; rr(w >= 0 ? bx : bx + w, y - 10, Math.abs(w) || 1, 24, 4); ctx.fillStyle = v >= 0 ? C.turq : C.coral; ctx.fill(); ctx.restore();
    });
    // what the numbers say
    const pr = A(1, 0, 0.6);
    card(900, 500, 920, 450, { a: pr, stroke: C.cobalt, lw: 2.5 });
    T('A better fit is not better play', 936, 552, { f: 'serif', size: 32, a: pr });
    T('validation error', 936, 616, { size: 18, w: 600, c: C.ink3, a: pr });
    T('0.060454 → 0.060373', 936, 660, { f: 'mono', size: 30, a: pr });
    T('the A3 weights → texel1: almost no change', 936, 694, { size: 18, c: C.ink2, a: pr });
    T('in games', 1400, 616, { size: 18, w: 600, c: C.ink3, a: pr });
    T(eloTxt(VER.V8.elo - VER.V6.elo), 1400, 664, { f: 'serif', size: 46, c: '#0F6E68', a: pr });
    T('Elo, V6 → V8', 1400, 694, { size: 18, c: C.ink2, a: pr });
    seg(936, 730, 1784, 730, C.hair, 1.5, pr);
    T('second tuning, 6,000 games: the best fit', 936, 780, { size: 21, w: 600, a: pr });
    T(eloTxt(VER.V8b.elo - VER.V8.elo), 936, 846, { f: 'serif', size: 46, c: C.coral, a: pr });
    TW('Elo against texel1 (V8b against V8): it fits the results better and plays worse. The loss is a stand-in; games decide.', 1090, 816, 690, 26, { size: 20, c: C.ink2, a: pr });
  },
});

// =====================================================================
// 7. Where this leads: deep and reinforcement learning (literature)
// =====================================================================
const S7 = [
  { year: 1959, name: 'Samuel', what: 'checkers: weights learned by playing itself', ch: 'Ch 23' },
  { year: 1995, name: 'TD-Gammon', what: 'TD(λ) trains a neural network by self-play', ch: 'Ch 22, 23' },
  { year: 2000, name: 'KnightCap', what: 'TD-Leaf: TD learning at alpha–beta leaves', ch: 'Ch 23' },
  { year: 2007, name: 'Chinook', what: 'checkers solved: alpha–beta and endgame databases', ch: 'Ch 6' },
  { year: 2015, name: 'Scan', what: 'draughts: alpha–beta, patterns from self-play', ch: 'Ch 6, 19' },
  { year: 2018, name: 'AlphaZero', what: 'self-play; a deep network guides MCTS', ch: 'Ch 22, 23' },
  { year: 2018, name: 'NNUE', what: 'a small network inside alpha–beta', ch: 'Ch 22' },
];
SCENES.push({
  beats: [16, 12],
  notes: [
    'Where this leads, from the literature. Samuel learned checkers weights by self-play in 1959; TD-Gammon and KnightCap learned by temporal differences; Scan, a top draughts engine, still uses alpha–beta with patterns learned from its own games; AlphaZero and NNUE brought deep networks.',
    'Our Texel weights sit next to Samuel and Scan. Our next steps follow the arrows: TD learning by self-play, chapter 23, and a small NNUE-style network trained on our positions, chapter 22.',
  ],
  render(beat, t) {
    const A = mkA(beat, t);
    chrome(7, 'Where it leads: deep and reinforcement learning', A, 'Samuel 1959 (IBM J. Res. Dev.); Tesauro 1995 (CACM); Baxter, Tridgell & Weaver 2000 (Machine Learning); Schaeffer et al. 2007 (Science); Scan (F. Letouzey; Computer Olympiad 2015); Silver et al. 2018 (Science); Nasu 2018 (NNUE).');
    const pa = A(0, 0, 0.6);
    const x0 = 255, x1 = 1665, yl = 520;
    seg(x0 - 40, yl, x1 + 40, yl, C.ink3, 2, pa);
    S7.forEach((m, i) => {
      const x = x0 + (i * (x1 - x0)) / (S7.length - 1), up = i % 2 === 0;
      const p = A(0, 0.15 + 0.1 * i, 0.6);
      fillCircle(x, yl, 10, C.cobalt, p);
      seg(x, yl + (up ? -14 : 14), x, yl + (up ? -76 : 76), C.hair, 2, p);
      const yt = up ? yl - 226 : yl + 76;
      card(x - 135, yt, 270, 150, { a: p });
      T(String(m.year), x - 110, yt + 38, { size: 18, w: 600, c: C.cobalt, a: p });
      T(m.name, x - 110, yt + 72, { f: 'serif', size: 26, a: p });
      TW(m.what, x - 110, yt + 102, 225, 21, { size: 16, c: C.ink2, a: p });
      tag(m.ch, x + 112, yt + 32, { size: 14, fill: 'rgba(39,71,163,0.10)', c: C.cobalt, al: 'right', a: p });
    });
    const pu = A(1, 0, 0.6);
    card(120, 820, 1680, 150, { a: pu, stroke: C.cobalt, lw: 2.5 });
    T('Us', 156, 876, { f: 'serif', size: 32, a: pu });
    TW('Texel weights fitted to game results: the idea of Samuel and Scan. Next: TD learning by self-play (Ch 23), and a small NNUE-style network trained on the same positions (Ch 22), each measured against V10.', 260, 868, 1500, 30, { size: 22, c: C.ink, a: pu });
  },
});

// =====================================================================
// 8. Next steps, and thank you
// =====================================================================
SCENES.push({
  beats: [18, 6],
  notes: [
    'Next steps. The search does not apply the 50-ply rule, which ended 43 of V10\'s 87 draws against our Assignment 3 agent: that was scene 3. Repetitions are worth nothing measurable now, and we want to know why. Then Monte Carlo with what the plain forms lacked, our own network, TD self-play, and play on the clock.',
    'Thank you. We are happy to take questions.',
  ],
  render(beat, t) {
    const A = mkA(beat, t);
    chrome(8, 'Next steps', A, 'Draws of V10 against V0 by kind and Elo of each part: results/ablation/ablation.json. Repetitions when added: V6 against V4, results/ladder/ladder.json.');
    const items = [
      ['The 50-ply rule in the search', `It ended ${V10V0.draws_by.fifty} of V10's ${V10V0.d} draws against V0 (scene 3); the other ${V10V0.draws_by.repetition} were repetitions.`],
      ['Why repetitions stopped mattering', `${eloTxt(VER.V6.elo - VER.V4.elo)} Elo when added (V4 → V6), ${r0(-ABV['V10-r'].elo)} now, not significant.`],
      ['Monte Carlo, given what it lacked', 'Repetitions, a table, more iterations, a tuned exploration constant.'],
      ['A network of our own, and TD self-play', 'Ch 22 and 23, trained on the positions we already have.'],
      ['Play on the clock', 'A time limit instead of 20,000 positions a move.'],
    ];
    items.forEach(([h, s], k) => {
      const p = A(0, 0.15 + 0.12 * k, 0.6), y = 240 + k * 132;
      ctx.save(); ctx.globalAlpha = p; rr(120, y - 36, 52, 52, 12); ctx.fillStyle = k === 0 ? C.cobalt : 'rgba(39,71,163,0.14)'; ctx.fill(); ctx.restore();
      T(String(k + 1), 146, y - 1, { size: 26, w: 600, c: k === 0 ? C.paper : C.cobalt, al: 'center', a: p });
      T(h, 200, y - 2, { size: 28, w: 600, a: p });
      T(s, 200, y + 34, { size: 21, c: C.ink2, a: p });
    });
    const pt = A(1, 0, 0.8);
    T('Thank you.', 1500, 520, { f: 'serif', size: 72, al: 'center', a: pt });
    T('Questions?', 1500, 590, { f: 'serif', size: 40, c: C.cobalt, al: 'center', a: pt });
    drawLogo(1450, 640, 100, pt);
  },
});

// =====================================================================
// Backup 1. How we measure
// =====================================================================
SCENES.push({
  beats: [30],
  notes: ['Backup: how every number was made.'],
  render(beat, t) {
    const A = mkA(beat, t);
    backupChrome('Backup', 'How we measure', A, 'docs/ladder.md, docs/decisions.md, CLAUDE.md (referees). Code: engine/src/arena.rs (schedule), engine/src/elo.rs (fit and bootstrap), engine/src/sprt.rs.');
    const pa = A(0, 0.1, 0.6);
    const L = [
      ['Paired openings', 'Every opening (4 random plies) is played twice with the colours swapped; every game has its own seeds.'],
      ['A budget of positions', '20,000 positions a move, not milliseconds: a decision does not depend on the machine or its load.'],
      ['One scale', 'Bradley–Terry by maximum likelihood over all games, draws count half, V1 at 0; 1,000 bootstrap samples of the colour-swapped pairs.'],
      ['Quick checks, not ratings', 'SPRT stops as soon as it decides, so its Elo runs high: we used it for yes or no, the ladder for the numbers.'],
      ['Referees', 'Perft depths 1–8 equal kish 1.1.2; legal moves equal the expected lists; a traced search equals the untraced one; the site plays the arena\'s games.'],
    ];
    L.forEach(([h, s], k) => {
      const y = 240 + k * 150;
      T(h, 120, y, { size: 26, w: 600, a: pa });
      TW(s, 120, y + 38, 1640, 30, { size: 22, c: C.ink2, a: pa });
    });
  },
});

// =====================================================================
// Backup 2. Sources
// =====================================================================
SCENES.push({
  beats: [30],
  notes: ['Backup: sources.'],
  render(beat, t) {
    const A = mkA(beat, t);
    backupChrome('Backup', 'Sources', A, 'Every number on the slides names its file in the footer; the files are in the repository and in the submission zip.');
    const pa = A(0, 0.1, 0.6);
    const S = [
      'Russell, S. & Norvig, P. (2021). Artificial Intelligence: A Modern Approach, 4th ed., Global Edition. Pearson. Ch 1–4, 6 (§6.4 Monte Carlo tree search), 19, 22, 23.',
      'Samuel, A. L. (1959). Some studies in machine learning using the game of checkers. IBM Journal of Research and Development 3(3), 210–229.',
      'Tesauro, G. (1995). Temporal difference learning and TD-Gammon. Communications of the ACM 38(3), 58–68.',
      'Baxter, J., Tridgell, A. & Weaver, L. (2000). Learning to play chess using temporal differences. Machine Learning 40(3), 243–263.',
      'Schaeffer, J. et al. (2007). Checkers is solved. Science 317(5844), 1518–1522.',
      'Ramanujan, R., Sabharwal, A. & Selman, B. (2010). On adversarial search spaces and sampling-based planning. ICAPS 2010.',
      'Silver, D. et al. (2018). A general reinforcement learning algorithm that masters chess, shogi, and Go through self-play. Science 362(6419), 1140–1144.',
      'Nasu, Y. (2018). Efficiently updatable neural-network-based evaluation functions for computer shogi (NNUE).',
      'Österlund, P. (2014). Texel\'s tuning method. Chess Programming Wiki. · Letouzey, F. Scan, a draughts engine: won the 18th Computer Olympiad, Leiden 2015 (Chess Programming Wiki, Draughts); “traditional alpha–beta”, patterns from self-play (lidraughts.org blog).',
      'Bradley, R. A. & Terry, M. E. (1952). Rank analysis of incomplete block designs. Biometrika 39(3/4), 324–345. · kish 1.1.2 (rules referee).',
    ];
    let y = 220;
    S.forEach((s) => { y += TW(s, 120, y, 1680, 29, { size: 21, c: C.ink, a: pa }) * 29 + 22; });
  },
});
