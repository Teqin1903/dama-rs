// ---------- stage, tokens, easing, drawing primitives ----------
const W = 1920, H = 1080;
const C = {
  ground: '#F1F5F4',
  ink: '#18223D',
  ink2: '#4A5470',
  ink3: '#7E879B',
  hair: '#CBD5D2',
  board: '#DCEAE5',
  grid: 'rgba(39,71,163,0.34)',
  cobalt: '#2747A3',
  turq: '#17988F',
  coral: '#C93A2B',
  white: '#FCFDFC',
  black: '#1F2638',
  paper: '#FFFFFF',
};
const F = {
  serif: '"Young Serif", Georgia, serif',
  sans: '"Instrument Sans", "Segoe UI", Arial, sans-serif',
  math: '"STIX Two Text", "Times New Roman", serif',
};
// roundRect for older browsers (Firefox before 112)
if (!CanvasRenderingContext2D.prototype.roundRect) {
  CanvasRenderingContext2D.prototype.roundRect = function (x, y, w, h, r) {
    const rs = Array.isArray(r) ? (r.length === 4 ? r : [r[0], r[0], r[0], r[0]]) : [r, r, r, r];
    const [tl, tr, br, bl] = rs.map((v) => Math.max(0, Math.min(v || 0, Math.abs(w) / 2, Math.abs(h) / 2)));
    this.moveTo(x + tl, y); this.lineTo(x + w - tr, y); this.quadraticCurveTo(x + w, y, x + w, y + tr);
    this.lineTo(x + w, y + h - br); this.quadraticCurveTo(x + w, y + h, x + w - br, y + h);
    this.lineTo(x + bl, y + h); this.quadraticCurveTo(x, y + h, x, y + h - bl);
    this.lineTo(x, y + tl); this.quadraticCurveTo(x, y, x + tl, y); this.closePath();
  };
}
// team logo (Eight Bits), inlined as a data URL by deck/build.py
const LOGO = new Image();
if (window.LOGO_SRC) LOGO.src = window.LOGO_SRC;
function drawLogo(x, y, size, a = 1, sc = 1) {
  if (a <= 0.002 || !LOGO.complete || !LOGO.naturalWidth) return;
  const s = size * sc;
  ctx.save(); ctx.globalAlpha = a; ctx.imageSmoothingEnabled = true; ctx.imageSmoothingQuality = 'high';
  ctx.drawImage(LOGO, x + (size - s) / 2, y + (size - s) / 2, s, s); ctx.restore();
}
const cv = document.getElementById('cv');
const ctx = cv.getContext('2d');

const clamp01 = (x) => (x < 0 ? 0 : x > 1 ? 1 : x);
const lerp = (a, b, t) => a + (b - a) * t;
const E = {
  lin: (t) => t,
  out: (t) => 1 - Math.pow(1 - t, 3),
  out5: (t) => 1 - Math.pow(1 - t, 5),
  io: (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2),
  back: (t) => { const c1 = 1.5, c3 = c1 + 1; return 1 + c3 * Math.pow(t - 1, 3) + c1 * Math.pow(t - 1, 2); },
};
// A(b, start, dur, ease): progress of an animation that belongs to beat b
function mkA(beat, t) {
  return (b, start = 0, dur = 0.6, e = E.out) => (beat < b ? 0 : beat > b ? 1 : e(clamp01((t - start) / dur)));
}
const fmt = (n) => Math.round(n).toLocaleString('en-US');
const fmt1 = (n) => n.toLocaleString('en-US', { minimumFractionDigits: 1, maximumFractionDigits: 1 });
const pct = (x) => { const v = Math.round(x * 1000) / 10; return `${Number.isInteger(v) ? v : v.toFixed(1)}%`; };
function bytes(n) {
  if (n < 1e3) return `${Math.round(n)} B`;
  if (n < 1e6) return `${fmt1(n / 1e3)} KB`;
  if (n < 1e9) return `${fmt1(n / 1e6)} MB`;
  if (n < 1e10) return `${(n / 1e9).toFixed(2)} GB`;
  return `${fmt1(n / 1e9)} GB`;
}

// ---------- text ----------
function font(f, size, w = 400, it = false) { return `${it ? 'italic ' : ''}${w} ${size}px ${F[f]}`; }
function T(s, x, y, o = {}) {
  const a = o.a ?? 1;
  if (a <= 0.002) return 0;
  ctx.save();
  ctx.globalAlpha = a;
  ctx.font = font(o.f || 'sans', o.size || 28, o.w || 400, o.it);
  ctx.fillStyle = o.c || C.ink;
  ctx.textAlign = o.al || 'left';
  ctx.textBaseline = o.bl || 'alphabetic';
  ctx.fillText(s, x, y);
  const w = ctx.measureText(s).width;
  ctx.restore();
  return w;
}
function measure(s, o = {}) {
  ctx.save(); ctx.font = font(o.f || 'sans', o.size || 28, o.w || 400, o.it);
  const w = ctx.measureText(s).width; ctx.restore(); return w;
}
function wrapLines(s, maxW, o) {
  const words = s.split(' '); const lines = []; let cur = '';
  for (const wd of words) {
    const test = cur ? cur + ' ' + wd : wd;
    if (measure(test, o) > maxW && cur) { lines.push(cur); cur = wd; } else cur = test;
  }
  if (cur) lines.push(cur);
  return lines;
}
function TW(s, x, y, maxW, lh, o = {}) {
  const lines = wrapLines(s, maxW, o);
  lines.forEach((ln, i) => T(ln, x, y + i * lh, o));
  return lines.length;
}
// Minimal math typesetting: single letters italic (STIX Two Text), other runs upright,
// ^{..} superscript, _{..} subscript.
function parseMath(s) {
  const out = []; let i = 0;
  (function parse(lvl) {
    while (i < s.length) {
      const ch = s[i];
      if (ch === '}') { i++; return; }
      if ((ch === '^' || ch === '_') && s[i + 1] === '{') { i += 2; parse(ch === '^' ? 1 : -1); continue; }
      if (/[A-Za-z]/.test(ch)) {
        let j = i; while (j < s.length && /[A-Za-z]/.test(s[j])) j++;
        const word = s.slice(i, j);
        out.push({ t: word, kind: word.length === 1 && word !== 'O' ? 'it' : 'up', lvl });
        i = j; continue;
      }
      let j = i; while (j < s.length && !/[A-Za-z^_{}]/.test(s[j])) j++;
      out.push({ t: s.slice(i, j), kind: 'up', lvl }); i = j;
    }
  })(0);
  return out;
}
function M(s, x, y, o = {}) {
  const size = o.size || 32; const segs = parseMath(s);
  const fontOf = (sg) => font('math', sg.lvl ? size * 0.68 : size, o.w || 400, sg.kind === 'it');
  ctx.save();
  let total = 0;
  const ws = segs.map((sg) => { ctx.font = fontOf(sg); const w = ctx.measureText(sg.t).width + (sg.kind === 'it' ? size * 0.03 : 0); total += w; return w; });
  if (o.measureOnly) { ctx.restore(); return total; }
  let cx = o.al === 'center' ? x - total / 2 : o.al === 'right' ? x - total : x;
  ctx.globalAlpha = o.a ?? 1; ctx.fillStyle = o.c || C.ink; ctx.textBaseline = 'alphabetic'; ctx.textAlign = 'left';
  segs.forEach((sg, k) => {
    ctx.font = fontOf(sg);
    const dy = sg.lvl === 1 ? -size * 0.36 : sg.lvl === -1 ? size * 0.2 : 0;
    ctx.fillText(sg.t, cx, y + dy); cx += ws[k];
  });
  ctx.restore();
  return total;
}
// Function names of the problem formulation (ACTIONS, RESULT, ...): sans semibold name, math arguments
function FN(s, x, y, o = {}) { return T(s, x, y, { f: 'sans', w: 600, size: o.size || 24, c: o.c || C.cobalt, a: o.a, al: o.al }); }
function FNM(name, args, x, y, o = {}) {
  const size = o.size || 24;
  const w1 = measure(name, { size, w: 600 });
  const w2 = M(args, 0, 0, { size: size * 1.12, measureOnly: true });
  const x0 = o.al === 'center' ? x - (w1 + w2) / 2 : o.al === 'right' ? x - w1 - w2 : x;
  FN(name, x0, y, { size, c: o.c, a: o.a });
  M(args, x0 + w1 + 1, y, { size: size * 1.12, c: o.c || C.cobalt, a: o.a });
  return w1 + w2;
}
// Move notation for display: captures use the multiplication sign
const nota = (m) => Dama.notation(m).replace(/x/g, '×');

// ---------- shapes ----------
function rr(x, y, w, h, r) { ctx.beginPath(); ctx.roundRect(x, y, w, h, r); }
function circle(x, y, r) { ctx.beginPath(); ctx.arc(x, y, Math.max(0, r), 0, Math.PI * 2); }
function fillCircle(x, y, r, c, a = 1) { if (a <= 0) return; ctx.save(); ctx.globalAlpha = a; ctx.fillStyle = c; circle(x, y, r); ctx.fill(); ctx.restore(); }
function strokeCircle(x, y, r, c, w, a = 1) { if (a <= 0) return; ctx.save(); ctx.globalAlpha = a; ctx.strokeStyle = c; ctx.lineWidth = w; circle(x, y, r); ctx.stroke(); ctx.restore(); }
function seg(x1, y1, x2, y2, c, w, a = 1, p = 1) {
  if (a <= 0 || p <= 0) return;
  ctx.save(); ctx.globalAlpha = a; ctx.strokeStyle = c; ctx.lineWidth = w; ctx.lineCap = 'round';
  ctx.beginPath(); ctx.moveTo(x1, y1); ctx.lineTo(lerp(x1, x2, p), lerp(y1, y2, p)); ctx.stroke(); ctx.restore();
}
function polyPartial(pts, p, c, w, a = 1) {
  if (p <= 0 || a <= 0) return;
  let total = 0; const L = [];
  for (let i = 1; i < pts.length; i++) { const l = Math.hypot(pts[i][0] - pts[i - 1][0], pts[i][1] - pts[i - 1][1]); L.push(l); total += l; }
  let rem = total * clamp01(p);
  ctx.save(); ctx.globalAlpha = a; ctx.strokeStyle = c; ctx.lineWidth = w; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
  ctx.beginPath(); ctx.moveTo(pts[0][0], pts[0][1]);
  for (let i = 1; i < pts.length && rem > 0; i++) {
    const f = Math.min(1, rem / L[i - 1]);
    ctx.lineTo(lerp(pts[i - 1][0], pts[i][0], f), lerp(pts[i - 1][1], pts[i][1], f));
    rem -= L[i - 1];
  }
  ctx.stroke(); ctx.restore();
}
function check(x, y, s, c, p = 1, w) { polyPartial([[x - 0.5 * s, y], [x - 0.14 * s, y + 0.34 * s], [x + 0.5 * s, y - 0.38 * s]], p, c, w || s * 0.17); }
function cross(x, y, s, c, p = 1, w) {
  const lw = w || s * 0.17, h = s / 2;
  polyPartial([[x - h, y - h], [x + h, y + h]], clamp01(p * 2), c, lw);
  polyPartial([[x + h, y - h], [x - h, y + h]], clamp01(p * 2 - 1), c, lw);
}
// quadratic curve with optional arrow head; bend = sideways offset of the control point (fraction of length)
function curve(x1, y1, x2, y2, o = {}) {
  const p = o.p ?? 1; if (p <= 0 || (o.a ?? 1) <= 0) return null;
  const dx = x2 - x1, dy = y2 - y1, len = Math.hypot(dx, dy) || 1, bend = o.bend || 0;
  const qx = (x1 + x2) / 2 - (dy / len) * bend * len, qy = (y1 + y2) / 2 + (dx / len) * bend * len;
  const at = (t) => { const u = 1 - t; return [u * u * x1 + 2 * u * t * qx + t * t * x2, u * u * y1 + 2 * u * t * qy + t * t * y2]; };
  ctx.save(); ctx.globalAlpha = o.a ?? 1; ctx.strokeStyle = o.c || C.ink; ctx.lineWidth = o.w || 2; ctx.lineCap = 'round';
  if (o.dash) ctx.setLineDash(o.dash);
  ctx.beginPath(); const N = 40;
  for (let i = 0; i <= N; i++) { const [px, py] = at((i / N) * p); if (i) ctx.lineTo(px, py); else ctx.moveTo(px, py); }
  ctx.stroke(); ctx.setLineDash([]);
  if (o.head && p > 0.04) {
    const [ex, ey] = at(p), [bx, by] = at(Math.max(0, p - 0.03));
    const ang = Math.atan2(ey - by, ex - bx), hs = o.head;
    ctx.fillStyle = o.c || C.ink; ctx.beginPath();
    ctx.moveTo(ex, ey);
    ctx.lineTo(ex - hs * Math.cos(ang - 0.42), ey - hs * Math.sin(ang - 0.42));
    ctx.lineTo(ex - hs * Math.cos(ang + 0.42), ey - hs * Math.sin(ang + 0.42));
    ctx.closePath(); ctx.fill();
  }
  ctx.restore();
  return at;
}
function pill(x, y, w, h, fill, a = 1, stroke) {
  if (a <= 0) return;
  ctx.save(); ctx.globalAlpha = a; rr(x, y, w, h, h / 2); ctx.fillStyle = fill; ctx.fill();
  if (stroke) { ctx.strokeStyle = stroke; ctx.lineWidth = 2; ctx.stroke(); }
  ctx.restore();
}
// A centred label inside a pill
function tag(s, x, y, o = {}) {
  const size = o.size || 22, padX = size * 0.62, h = size * 1.7;
  const w = measure(s, { size, w: o.w || 600 }) + padX * 2;
  const x0 = o.al === 'center' ? x - w / 2 : o.al === 'right' ? x - w : x;
  pill(x0, y - h / 2, w, h, o.fill || C.paper, o.a ?? 1, o.stroke);
  T(s, x0 + w / 2, y + size * 0.36, { size, w: o.w || 600, c: o.c || C.ink, al: 'center', a: o.a ?? 1 });
  return w;
}

// ---------- board and pieces ----------
function BG(x, y, size) {
  const s = size / 8;
  return { x, y, size, s, cx: (c) => x + (c + 0.5) * s, cy: (r) => y + (7 - r + 0.5) * s, sqx: (sq) => x + ((sq & 7) + 0.5) * s, sqy: (sq) => y + (7 - (sq >> 3) + 0.5) * s };
}
function boardBase(g, o = {}) {
  const a = o.a ?? 1; if (a <= 0.002) return;
  const { x, y, size, s } = g;
  ctx.save(); ctx.globalAlpha = a;
  if (!o.flat) { ctx.fillStyle = 'rgba(24,34,61,0.07)'; rr(x + s * 0.05, y + s * 0.1, size, size, s * 0.12); ctx.fill(); }
  ctx.fillStyle = o.fill || C.board; rr(x, y, size, size, Math.max(3, s * 0.1)); ctx.fill();
  if (o.hl) for (const h of o.hl) {
    if ((h.a ?? 0.3) <= 0) continue;
    ctx.globalAlpha = a * (h.a ?? 0.3); ctx.fillStyle = h.c;
    ctx.fillRect(x + (h.sq & 7) * s, y + (7 - (h.sq >> 3)) * s, s, s);
  }
  ctx.globalAlpha = a;
  ctx.strokeStyle = o.gridColor || C.grid; ctx.lineWidth = Math.max(0.8, s * 0.022);
  ctx.beginPath();
  for (let i = 1; i < 8; i++) { ctx.moveTo(x + i * s, y); ctx.lineTo(x + i * s, y + size); ctx.moveTo(x, y + i * s); ctx.lineTo(x + size, y + i * s); }
  ctx.stroke();
  ctx.strokeStyle = o.frame || 'rgba(39,71,163,0.55)'; ctx.lineWidth = Math.max(1, s * 0.035);
  rr(x, y, size, size, Math.max(3, s * 0.1)); ctx.stroke();
  if (o.coords) {
    const fs = Math.max(11, s * 0.2);
    for (let i = 0; i < 8; i++) {
      T('abcdefgh'[i], x + (i + 0.5) * s, y + size + fs * 1.35, { size: fs, w: 500, c: C.ink3, al: 'center', a });
      T(String(i + 1), x - fs * 0.85, y + (7 - i + 0.5) * s + fs * 0.36, { size: fs, w: 500, c: C.ink3, al: 'center', a });
    }
  }
  ctx.restore();
}
// v: 1 white man, 2 white king, -1 black man, -2 black king
function drawPiece(cx, cy, r, v, o = {}) {
  const a = o.a ?? 1; if (a <= 0.002 || !v) return;
  r *= o.sc ?? 1;
  const white = v > 0, king = Math.abs(v) === 2;
  ctx.save(); ctx.globalAlpha = a;
  if (o.ring) { ctx.strokeStyle = o.ring; ctx.lineWidth = Math.max(2, r * 0.17); ctx.globalAlpha = a * (o.ringA ?? 1); circle(cx, cy, r * 1.3); ctx.stroke(); ctx.globalAlpha = a; }
  if (r < 9) { // flat style for small boards
    ctx.fillStyle = white ? C.white : C.black; circle(cx, cy, r); ctx.fill();
    if (white) { ctx.strokeStyle = C.ink; ctx.lineWidth = Math.max(0.8, r * 0.22); ctx.stroke(); }
    if (king) { ctx.fillStyle = white ? C.cobalt : '#9FB0C9'; circle(cx, cy, r * 0.42); ctx.fill(); }
    ctx.restore(); return;
  }
  const lift = king ? r * 0.16 : 0;
  ctx.fillStyle = 'rgba(24,34,61,0.20)'; circle(cx + r * 0.05, cy + r * 0.14, r); ctx.fill();
  const body = (yy) => {
    ctx.fillStyle = white ? C.white : C.black; circle(cx, yy, r); ctx.fill();
    ctx.strokeStyle = white ? C.ink : '#0E1322'; ctx.lineWidth = Math.max(1.2, r * 0.075); ctx.stroke();
  };
  if (king) body(cy + lift);
  body(cy - lift);
  ctx.strokeStyle = white ? 'rgba(24,34,61,0.30)' : 'rgba(255,255,255,0.22)'; ctx.lineWidth = Math.max(1, r * 0.06);
  circle(cx, cy - lift, r * 0.62); ctx.stroke();
  if (!white) { ctx.fillStyle = 'rgba(255,255,255,0.10)'; ctx.beginPath(); ctx.arc(cx, cy - lift, r * 0.86, Math.PI * 1.1, Math.PI * 1.65); ctx.arc(cx, cy - lift, r * 0.6, Math.PI * 1.65, Math.PI * 1.1, true); ctx.fill(); }
  ctx.restore();
}
function boardPieces(g, b, o = {}) {
  for (let sq = 0; sq < 64; sq++) {
    if (!b[sq]) continue;
    const opt = o.per ? o.per(sq) : null;
    if (opt === false) continue;
    drawPiece(g.sqx(sq), g.sqy(sq), g.s * 0.36, b[sq], { a: o.a, ...(opt || {}) });
  }
}
// Mini board used as a search-tree node
function miniBoard(x, y, size, b, o = {}) {
  const g = BG(x, y, size);
  boardBase(g, { a: o.a, flat: true, hl: o.hl, frame: o.frame });
  boardPieces(g, b, { a: o.a, per: o.per });
  if (o.outline) { ctx.save(); ctx.globalAlpha = o.a ?? 1; ctx.strokeStyle = o.outline; ctx.lineWidth = o.outlineW || 3; rr(x - 3, y - 3, size + 6, size + 6, Math.max(4, size * 0.06)); ctx.stroke(); ctx.restore(); }
  return g;
}

// ---------- slide chrome ----------
const SLIDE_COUNT = 9; // main slides; the backup slides after them carry no number
function chrome(n, title, A, foot) {
  const pa = A(0, 0, 0.5);
  if (title) T(title, 120, 132, { f: 'serif', size: 54, c: C.ink, a: pa });
  if (n) T(`${n} / ${SLIDE_COUNT}`, 1800, 1040, { size: 18, w: 500, c: C.ink3, al: 'right', a: pa });
  if (foot) { let fs = 18; while (fs > 13 && measure(foot, { size: fs }) > 1560) fs -= 0.5; T(foot, 120, 1040, { size: fs, c: C.ink3, a: pa }); }
}

// =====================================================================
// Assignment 3 additions: mono font, prototype badge, move playback, fog,
// log scales, charts, isometric landscape, terminal panel
// =====================================================================
F.mono = '"JetBrains Mono", Menlo, Consolas, monospace';
const PROTO = 'agent prototype in JS';
function protoBadge(x, y, a = 1, o = {}) {
  if (a <= 0.002) return 0;
  return tag(PROTO, x, y, { size: o.size || 16, w: 500, fill: o.fill || C.paper, stroke: o.stroke || C.hair, c: o.c || C.ink2, al: o.al || 'left', a });
}
const mix = (c1, c2, t) => c1.map((v, i) => Math.round(lerp(v, c2[i], t)));
const rgb = (c, a = 1) => `rgba(${c[0]},${c[1]},${c[2]},${a})`;
const hex2rgb = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
const logT = (v, vmin, vmax) => clamp01((Math.log10(Math.max(v, vmin)) - Math.log10(vmin)) / (Math.log10(vmax) - Math.log10(vmin)));
function poly(pts, fill, stroke, lw = 1, a = 1) {
  if (a <= 0.002) return;
  ctx.save(); ctx.globalAlpha = a; ctx.beginPath(); ctx.moveTo(pts[0][0], pts[0][1]);
  for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i][0], pts[i][1]);
  ctx.closePath(); if (fill) { ctx.fillStyle = fill; ctx.fill(); } if (stroke) { ctx.strokeStyle = stroke; ctx.lineWidth = lw; ctx.stroke(); }
  ctx.restore();
}
// Draw board position b while move m is played (p: 0..1). The mover glides along m.path, one segment
// after another; each captured piece fades as the mover passes it; a man that promotes is crowned at the end.
function playMove(g, b, m, p, o = {}) {
  const a0 = o.a ?? 1; if (a0 <= 0.002) return;
  const segs = m.path.length - 1, sp = clamp01(p) * segs;
  let k = Math.min(segs - 1, Math.floor(sp)), f = sp - k; if (p >= 1) { k = segs - 1; f = 1; }
  const e = E.io(f);
  const pos = (s) => [g.sqx(s), g.sqy(s)];
  const [x0, y0] = pos(m.path[k]), [x1, y1] = pos(m.path[k + 1]);
  const cx = lerp(x0, x1, e), cy = lerp(y0, y1, e);
  if (o.trail && p > 0) {
    const pts = m.path.slice(0, k + 1).map(pos); pts.push([cx, cy]);
    ctx.save(); ctx.globalAlpha = a0 * (o.trailA ?? 0.8); ctx.strokeStyle = o.trail; ctx.lineWidth = o.trailW || Math.max(2, g.s * 0.08);
    ctx.lineCap = 'round'; ctx.lineJoin = 'round'; if (o.dash) ctx.setLineDash(o.dash);
    ctx.beginPath(); ctx.moveTo(pts[0][0], pts[0][1]); for (let i = 1; i < pts.length; i++) ctx.lineTo(pts[i][0], pts[i][1]); ctx.stroke(); ctx.restore();
  }
  for (let sq = 0; sq < 64; sq++) {
    const v = b[sq]; if (!v || sq === m.from) continue;
    let a = a0; const ci = m.caps.indexOf(sq);
    if (ci >= 0) a *= 1 - clamp01((sp - ci - 0.42) / 0.4);
    const opt = o.per ? o.per(sq) : null; if (opt === false) continue;
    drawPiece(g.sqx(sq), g.sqy(sq), g.s * 0.36, v, { a, ...(opt || {}) });
  }
  const lift = m.caps.length ? Math.sin(Math.PI * f) * g.s * 0.16 : 0;
  const v0 = b[m.from], crowned = m.promo && p >= 1;
  drawPiece(cx, cy - lift, g.s * 0.36, crowned ? v0 * 2 : v0, { a: a0, ring: o.ring, sc: 1 + lift / (g.s * 1.6) });
}
// find a legal move by its notation
const findMove = (b, side, name) => Dama.moves(b, side).find((m) => Dama.notation(m) === name);
// drifting fog over a rectangle (t in seconds keeps it moving); o.soft: height of a soft lower edge
function fog(x, y, w, h, t, a = 1, o = {}) {
  if (a <= 0.002 || h <= 0) return;
  const soft = o.soft || 0;
  ctx.save(); ctx.globalAlpha = a; rr(x, y, w, h + soft, o.r ?? 10); ctx.clip();
  const base = o.base || [232, 238, 236], ba = o.baseA ?? 0.86;
  if (soft) { const lg = ctx.createLinearGradient(0, y, 0, y + h + soft); lg.addColorStop(0, rgb(base, ba)); lg.addColorStop(h / (h + soft), rgb(base, ba)); lg.addColorStop(1, rgb(base, 0)); ctx.fillStyle = lg; }
  else ctx.fillStyle = rgb(base, ba);
  ctx.fillRect(x, y, w, h + soft);
  for (let i = 0; i < 10; i++) {
    const sp = 0.018 + 0.012 * (i % 4);
    const u = ((i * 0.317 + t * sp) % 1.4) - 0.2, v = 0.12 + 0.72 * ((i * 0.618) % 1);
    const bx = x + u * w, by = y + v * h + Math.sin(t * 0.5 + i) * h * 0.04, r = (0.22 + 0.14 * ((i * 0.43) % 1)) * Math.max(w, h);
    const gr = ctx.createRadialGradient(bx, by, 0, bx, by, r);
    gr.addColorStop(0, o.blob || 'rgba(196,207,205,0.55)'); gr.addColorStop(1, 'rgba(196,207,205,0)');
    ctx.fillStyle = gr; ctx.fillRect(x, y, w, h + soft);
  }
  ctx.restore();
}
// chart axes helpers
function axisLine(x1, y1, x2, y2, a) { seg(x1, y1, x2, y2, C.ink3, 1.6, a); }
function gridLine(x1, y1, x2, y2, a) { seg(x1, y1, x2, y2, C.hair, 1, a); }
// waffle: n cells in rows of `cols`; colour(i) per cell; cells appear in order with progress p
function waffle(x, y, n, cols, cell, gap, colour, p, a = 1) {
  const shown = n * clamp01(p);
  for (let i = 0; i < n; i++) {
    const q = clamp01(shown - i); if (q <= 0) break;
    const cx = x + (i % cols) * (cell + gap), cy = y + Math.floor(i / cols) * (cell + gap), s = cell * E.back(q);
    ctx.save(); ctx.globalAlpha = a * clamp01(q * 2); ctx.fillStyle = colour(i);
    rr(cx + (cell - s) / 2, cy + (cell - s) / 2, s, s, Math.max(2, s * 0.18)); ctx.fill(); ctx.restore();
  }
}
// terminal-style panel; lines: [{t, c, w}] or strings; reveal: number of characters typed so far
function terminal(x, y, w, h, title, lines, chars, o = {}) {
  const a = o.a ?? 1; if (a <= 0.002) return;
  ctx.save(); ctx.globalAlpha = a;
  ctx.fillStyle = 'rgba(24,34,61,0.10)'; rr(x + 6, y + 10, w, h, 18); ctx.fill();
  ctx.fillStyle = '#161F38'; rr(x, y, w, h, 18); ctx.fill();
  ctx.fillStyle = '#212C4B'; rr(x, y, w, 52, [18, 18, 0, 0]); ctx.fill();
  ['#C93A2B', '#E0A93B', '#17988F'].forEach((c, i) => { ctx.fillStyle = c; circle(x + 28 + i * 22, y + 26, 6.5); ctx.fill(); });
  ctx.restore();
  T(title, x + 104, y + 33, { f: 'mono', size: 17, c: '#9FB0C9', a });
  const size = o.size || 19, lh = o.lh || 31;
  let left = chars, row = 0;
  ctx.save(); rr(x, y + 52, w, h - 52, [0, 0, 18, 18]); ctx.clip();
  const y0 = y + 92 - (o.scroll || 0);
  for (const ln of lines) {
    const L0 = typeof ln === 'string' ? { t: ln } : ln;
    if (left <= 0) break;
    const s = L0.t.slice(0, Math.max(0, Math.floor(left)));
    left -= L0.t.length + 6;
    const yy = y0 + row * lh; row++;
    if (L0.bg) { ctx.save(); ctx.globalAlpha = a * (L0.bgA ?? 1); ctx.fillStyle = L0.bg; ctx.fillRect(x + 14, yy - lh * 0.72, w - 28, lh); ctx.restore(); }
    T(s, x + 28, yy, { f: 'mono', size, c: L0.c || '#E3E9F2', w: L0.w || 400, a });
  }
  ctx.restore();
}
// isometric projection for the weight landscape: X along king weights, Y along mobility weights, Z = score
function isoProj(P) {
  const ca = Math.cos(P.az), sa = Math.sin(P.az);
  const pr = (X, Y, Z) => [P.ox + (Y * ca - X * sa) * P.u, P.oy + (Y * sa + X * ca) * P.u * P.tilt - Z * P.hz];
  pr.depth = (X, Y) => Y * sa + X * ca;
  return pr;
}
const LAND_LO = [234, 199, 192], LAND_MID = [207, 216, 232], LAND_HI = [23, 152, 143];
const landColour = (f) => (f < 0.5 ? mix(LAND_LO, LAND_MID, f / 0.5) : mix(LAND_MID, LAND_HI, (f - 0.5) / 0.5));
function landscape(pr, F, rise, a = 1, o = {}) {
  const n = F.length, m = F[0].length; const cells = [];
  for (let i = 0; i < n; i++) for (let j = 0; j < m; j++) cells.push([i, j, pr.depth(i, j)]);
  cells.sort((p, q) => p[2] - q[2]);
  const dmin = cells[0][2], dmax = cells[cells.length - 1][2];
  for (const [i, j, d] of cells) {
    const q = clamp01(rise * 1.8 - 0.8 * (d - dmin) / (dmax - dmin));
    const h = Math.max(0.006, (o.H ? o.H(F[i][j]) : F[i][j]) * E.out(q));
    const col = o.col ? o.col(F[i][j]) : landColour(F[i][j]);
    const t0 = pr(i - 0.5, j - 0.5, h), t1 = pr(i - 0.5, j + 0.5, h), t2 = pr(i + 0.5, j + 0.5, h), t3 = pr(i + 0.5, j - 0.5, h);
    const b1 = pr(i - 0.5, j + 0.5, 0), b2 = pr(i + 0.5, j + 0.5, 0), b3 = pr(i + 0.5, j - 0.5, 0);
    const fade = o.fade || 0, G0 = [241, 245, 244];
    const c1 = mix(mix(col, [24, 34, 61], 0.22), G0, fade), c2 = mix(mix(col, [24, 34, 61], 0.36), G0, fade), c0 = mix(col, G0, fade);
    poly([t3, t2, b2, b3], rgb(c1), null, 1, a);
    poly([t1, t2, b2, b1], rgb(c2), null, 1, a);
    poly([t0, t1, t2, t3], rgb(c0), `rgba(24,34,61,${0.16 * (1 - fade)})`, 1, a);
  }
}

// ---------- verdicts: does an idea apply to our game? ----------
// kind: 'yes' applies, 'no' does not apply, 'part' only partly
const VCOL = { yes: C.turq, no: C.coral, part: C.cobalt };
function verdictIcon(cx, cy, r, kind, a = 1) {
  if (a <= 0.002) return VCOL[kind];
  fillCircle(cx, cy, r, VCOL[kind], a);
  ctx.save(); ctx.globalAlpha = a; ctx.strokeStyle = C.paper; ctx.lineWidth = Math.max(2.4, r * 0.2); ctx.lineCap = 'round'; ctx.lineJoin = 'round'; ctx.beginPath();
  if (kind === 'yes') { ctx.moveTo(cx - r * 0.45, cy + r * 0.02); ctx.lineTo(cx - r * 0.12, cy + r * 0.34); ctx.lineTo(cx + r * 0.48, cy - r * 0.36); }
  else if (kind === 'no') { ctx.moveTo(cx - r * 0.36, cy - r * 0.36); ctx.lineTo(cx + r * 0.36, cy + r * 0.36); ctx.moveTo(cx + r * 0.36, cy - r * 0.36); ctx.lineTo(cx - r * 0.36, cy + r * 0.36); }
  else { ctx.moveTo(cx - r * 0.42, cy); ctx.lineTo(cx + r * 0.42, cy); }
  ctx.stroke(); ctx.restore();
  return VCOL[kind];
}
// one verdict line: icon, bold head, then the reason on the same line if it fits, else wrapped below the head
function verdict(x, y, kind, head, reason, a = 1, o = {}) {
  if (a <= 0.002) return 0;
  const size = o.size || 24, r = size * 0.6, maxW = o.maxW || 1680, rs = o.rsize || size - 2;
  const col = verdictIcon(x + r, y - size * 0.34, r, kind, a);
  const hx = x + 2 * r + 14;
  const wh = T(head, hx, y, { size, w: 600, c: col, a });
  if (!reason) return 1;
  if (hx + wh + 12 + measure(reason, { size: rs }) <= x + maxW) { T(reason, hx + wh + 12, y, { size: rs, c: o.rc || C.ink2, a }); return 1; }
  return 1 + TW(reason, hx, y + size * 1.4, x + maxW - hx, size * 1.3, { size: rs, c: o.rc || C.ink2, a });
}
// dashed frame around a "what if" picture (something our game does not have), label on its top edge
function whatIf(x, y, w, h, label, a = 1) {
  if (a <= 0.002) return;
  ctx.save(); ctx.globalAlpha = a; ctx.setLineDash([10, 8]); ctx.strokeStyle = C.cobalt; ctx.lineWidth = 2.2; rr(x, y, w, h, 20); ctx.stroke(); ctx.restore();
  tag(label, x + 28, y, { size: 18, fill: C.ground, stroke: C.cobalt, c: C.cobalt, a });
}
// polyline with rounded corners, drawn up to fraction o.p, with optional dash and arrow head
function pathArrow(pts, o = {}) {
  const p = clamp01(o.p ?? 1), a = o.a ?? 1; if (p <= 0 || a <= 0.002) return;
  const r = o.r ?? 14, P = [pts[0]];
  for (let i = 1; i < pts.length - 1; i++) {
    const [x0, y0] = pts[i - 1], [x1, y1] = pts[i], [x2, y2] = pts[i + 1];
    const d1 = Math.hypot(x1 - x0, y1 - y0), d2 = Math.hypot(x2 - x1, y2 - y1), k = Math.min(r, d1 / 2, d2 / 2);
    const ax = x1 + (x0 - x1) * k / d1, ay = y1 + (y0 - y1) * k / d1, bx = x1 + (x2 - x1) * k / d2, by = y1 + (y2 - y1) * k / d2;
    for (let s = 0; s <= 8; s++) { const t = s / 8, u = 1 - t; P.push([u * u * ax + 2 * u * t * x1 + t * t * bx, u * u * ay + 2 * u * t * y1 + t * t * by]); }
  }
  P.push(pts[pts.length - 1]);
  const L = [0]; for (let i = 1; i < P.length; i++) L.push(L[i - 1] + Math.hypot(P[i][0] - P[i - 1][0], P[i][1] - P[i - 1][1]));
  const goal = L[L.length - 1] * p;
  ctx.save(); ctx.globalAlpha = a; ctx.strokeStyle = o.c || C.ink; ctx.lineWidth = o.w || 2; ctx.lineCap = 'round'; ctx.lineJoin = 'round';
  if (o.dash) ctx.setLineDash(o.dash);
  ctx.beginPath(); ctx.moveTo(P[0][0], P[0][1]);
  let ex = P[0][0], ey = P[0][1], ang = 0;
  for (let i = 1; i < P.length; i++) {
    const dl = L[i] - L[i - 1]; if (dl < 1e-6) continue;
    ang = Math.atan2(P[i][1] - P[i - 1][1], P[i][0] - P[i - 1][0]);
    if (L[i] <= goal) { ex = P[i][0]; ey = P[i][1]; ctx.lineTo(ex, ey); continue; }
    const f = (goal - L[i - 1]) / dl; ex = lerp(P[i - 1][0], P[i][0], f); ey = lerp(P[i - 1][1], P[i][1], f); ctx.lineTo(ex, ey); break;
  }
  ctx.stroke(); ctx.setLineDash([]);
  if (o.head && p > 0.03) {
    const hs = o.head; ctx.fillStyle = o.c || C.ink; ctx.beginPath(); ctx.moveTo(ex, ey);
    ctx.lineTo(ex - hs * Math.cos(ang - 0.42), ey - hs * Math.sin(ang - 0.42)); ctx.lineTo(ex - hs * Math.cos(ang + 0.42), ey - hs * Math.sin(ang + 0.42));
    ctx.closePath(); ctx.fill();
  }
  ctx.restore();
}
