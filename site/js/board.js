// SVG board view: draws a position, highlights, arrows and labels, animates moves, reports clicks.
// Squares are "a1".."h8"; the board string is the one in docs/reference.md (rank 8 first).
(function (root) {
  'use strict';
  const NS = 'http://www.w3.org/2000/svg';
  const S = 100; // one square in SVG units
  const FILES = 'abcdefgh';

  const el = (tag, attrs, parent) => {
    const n = document.createElementNS(NS, tag);
    for (const k in attrs || {}) n.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(n);
    return n;
  };
  const sqIndex = (name) => (name.charCodeAt(0) - 97) + 8 * (parseInt(name.slice(1), 10) - 1);
  const sqName = (i) => FILES[i & 7] + ((i >> 3) + 1);
  function parseBoard(str) {
    const rows = str.split('/');
    const cells = new Array(64).fill('.');
    rows.forEach((row, i) => { for (let f = 0; f < 8; f++) cells[(7 - i) * 8 + f] = row[f]; });
    return cells;
  }

  class BoardView {
    constructor(host) {
      this.host = host;
      this.flip = false;
      this.onSquare = null;
      this.svg = el('svg', { viewBox: '-34 -8 842 842', class: 'board-svg', role: 'img', 'aria-label': 'Board' }, host);
      const defs = el('defs', {}, this.svg);
      for (const [id, color] of [['ah-turq', 'var(--turq)'], ['ah-coral', 'var(--coral)'], ['ah-cobalt', 'var(--cobalt)'], ['ah-ink', 'var(--ink3)']]) {
        const m = el('marker', { id, viewBox: '0 0 10 10', refX: 6.2, refY: 5, markerWidth: 3.2, markerHeight: 3.2, orient: 'auto-start-reverse' }, defs);
        el('path', { d: 'M0,0 L10,5 L0,10 z', style: `fill:${color}` }, m);
      }
      this.gBase = el('g', {}, this.svg);
      this.gHi = el('g', {}, this.svg);
      this.gPieces = el('g', {}, this.svg);
      this.gArrows = el('g', { class: 'arrows' }, this.svg);
      this.gLabels = el('g', { class: 'labels' }, this.svg);
      this.gHit = el('g', {}, this.svg);
      this.cells = new Array(64).fill('.');
      this.drawBase();
    }
    xy(i) {
      let f = i & 7, r = i >> 3;
      if (this.flip) { f = 7 - f; r = 7 - r; }
      return [f * S, (7 - r) * S];
    }
    center(i) { const [x, y] = this.xy(i); return [x + S / 2, y + S / 2]; }
    drawBase() {
      const g = this.gBase;
      g.replaceChildren();
      el('rect', { x: 4, y: 10, width: 800, height: 800, rx: 10, class: 'b-shadow' }, g);
      el('rect', { x: 0, y: 0, width: 800, height: 800, rx: 10, class: 'b-fill' }, g);
      const grid = el('path', { class: 'b-grid', d: '' }, g);
      let d = '';
      for (let i = 1; i < 8; i++) d += `M${i * S},0V800M0,${i * S}H800`;
      grid.setAttribute('d', d);
      el('rect', { x: 0, y: 0, width: 800, height: 800, rx: 10, class: 'b-frame' }, g);
      for (let i = 0; i < 8; i++) {
        const fi = this.flip ? 7 - i : i;
        el('text', { x: i * S + S / 2, y: 832, class: 'b-coord', 'text-anchor': 'middle' }, g).textContent = FILES[fi];
        const ri = this.flip ? i + 1 : 8 - i;
        el('text', { x: -17, y: i * S + S / 2 + 9, class: 'b-coord', 'text-anchor': 'middle' }, g).textContent = String(ri);
      }
      this.gHit.replaceChildren();
      for (let i = 0; i < 64; i++) {
        const [x, y] = this.xy(i);
        const r = el('rect', { x, y, width: S, height: S, class: 'b-hit', 'data-sq': sqName(i) }, this.gHit);
        r.addEventListener('click', () => this.onSquare && this.onSquare(sqName(i)));
      }
    }
    setFlip(f) { if (f !== this.flip) { this.flip = f; this.drawBase(); this.drawPieces(); } }
    setBoard(str) { this.cells = parseBoard(str); this.drawPieces(); }
    piece(i, c, g, opts = {}) {
      const [cx, cy] = this.center(i);
      const grp = el('g', { class: 'pc', transform: `translate(${cx},${cy})`, 'data-sq': sqName(i) }, g || this.gPieces);
      const white = c === 'w' || c === 'W', king = c === 'W' || c === 'B';
      const r = 36, lift = king ? 6 : 0;
      el('circle', { cx: 2, cy: 5, r, class: 'pc-shadow' }, grp);
      const body = (dy) => el('circle', { cx: 0, cy: dy, r, class: white ? 'pc-w' : 'pc-b' }, grp);
      if (king) body(lift);
      body(-lift);
      el('circle', { cx: 0, cy: -lift, r: r * 0.62, class: white ? 'pc-ring-w' : 'pc-ring-b' }, grp);
      if (king) el('path', { d: 'M-13,-1 L-8,-13 L0,-5 L8,-13 L13,-1 Z', transform: `translate(0,${-lift + 3})`, class: white ? 'pc-crown-w' : 'pc-crown-b' }, grp);
      if (opts.ghost) grp.setAttribute('opacity', '0.35');
      return grp;
    }
    drawPieces() {
      this.gPieces.replaceChildren();
      for (let i = 0; i < 64; i++) if (this.cells[i] !== '.') this.piece(i, this.cells[i]);
    }
    // highlights: [{sq, kind}] kind: last | sel | target | path | captured | node
    setHighlights(list) {
      const g = this.gHi;
      g.replaceChildren();
      for (const h of list || []) {
        const i = sqIndex(h.sq);
        const [x, y] = this.xy(i);
        if (h.kind === 'target') el('circle', { cx: x + S / 2, cy: y + S / 2, r: 13, class: 'hi-target' }, g);
        else el('rect', { x: x + 1, y: y + 1, width: S - 2, height: S - 2, class: 'hi-' + h.kind }, g);
      }
    }
    // arrows: [{path:[sq..], tone:'turq'|'coral'|'cobalt'|'ink', dash, width, opacity, label}]
    setArrows(list) {
      const g = this.gArrows;
      g.replaceChildren();
      for (const a of list || []) {
        const pts = a.path.map((s) => this.center(sqIndex(s)));
        if (pts.length < 2) continue;
        const shorten = (p, q, d) => { const dx = q[0] - p[0], dy = q[1] - p[1], L = Math.hypot(dx, dy) || 1; return [q[0] - (dx / L) * d, q[1] - (dy / L) * d]; };
        const last = shorten(pts[pts.length - 2], pts[pts.length - 1], 14);
        const first = shorten(pts[1], pts[0], 18);
        const all = [first, ...pts.slice(1, -1), last];
        const tone = a.tone || 'turq';
        el('path', {
          d: 'M' + all.map((p) => p.join(',')).join('L'),
          class: 'arrow arrow-' + tone, 'marker-end': `url(#ah-${tone})`,
          style: `stroke-width:${a.width || 13};opacity:${a.opacity ?? 0.85}` + (a.dash ? ';stroke-dasharray:22 16' : ''),
        }, g);
        if (a.label) {
          const [lx, ly] = pts[0];
          const t = el('g', { transform: `translate(${lx - 30},${ly - 30})` }, g);
          el('circle', { cx: 0, cy: 0, r: 15, class: 'arrow-badge arrow-badge-' + tone }, t);
          el('text', { x: 0, y: 6, 'text-anchor': 'middle', class: 'arrow-badge-text' }, t).textContent = a.label;
        }
      }
    }
    // small text labels on squares: [{sq, text, tone}]
    setLabels(list) {
      const g = this.gLabels;
      g.replaceChildren();
      for (const l of list || []) {
        const [cx, cy] = this.center(sqIndex(l.sq));
        const w = Math.max(44, l.text.length * 12 + 16); // 19px mono: about 11.4 per character
        el('rect', { x: cx - w / 2, y: cy + 16, width: w, height: 28, rx: 14, class: 'lab lab-' + (l.tone || 'ink') }, g);
        el('text', { x: cx, y: cy + 36, 'text-anchor': 'middle', class: 'lab-text' }, g).textContent = l.text;
      }
    }
    // animate a move {path:[...], captured:[...], promotes}; resolves when done
    animate(mv, ms = 220) {
      return new Promise((resolve) => {
        const from = sqIndex(mv.path[0]);
        const node = [...this.gPieces.children].find((n) => n.getAttribute('data-sq') === mv.path[0]);
        if (!node || typeof requestAnimationFrame === 'undefined') return resolve();
        this.gPieces.appendChild(node); // on top
        const pts = mv.path.map((s) => this.center(sqIndex(s)));
        const capNodes = (mv.captured || []).map((s) => [...this.gPieces.children].find((n) => n.getAttribute('data-sq') === s)).filter(Boolean);
        const hops = pts.length - 1;
        const total = Math.max(ms, hops * ms);
        const t0 = performance.now();
        const step = (now) => { try { frame(now); } catch (e) { resolve(); } };
        const frame = (now) => {
          const t = Math.max(0, Math.min(1, (now - t0) / total));
          const seg = Math.min(hops - 1, Math.floor(t * hops));
          const lt = t * hops - seg;
          const e = lt < 0.5 ? 2 * lt * lt : 1 - Math.pow(-2 * lt + 2, 2) / 2;
          const [x0, y0] = pts[seg], [x1, y1] = pts[seg + 1];
          const lift = Math.sin(Math.PI * lt) * (mv.captured && mv.captured.length ? 18 : 6);
          node.setAttribute('transform', `translate(${x0 + (x1 - x0) * e},${y0 + (y1 - y0) * e - lift}) scale(${1 + lift / 160})`);
          capNodes.forEach((c, k) => { if (t * hops > k + 0.6) c.setAttribute('opacity', String(Math.max(0, 1 - (t * hops - k - 0.6) * 2.5))); });
          if (t < 1) requestAnimationFrame(step); else resolve();
        };
        void from;
        requestAnimationFrame(step);
      });
    }
  }
  root.BoardView = BoardView;
  root.BoardUtil = { sqIndex, sqName, parseBoard };
})(window);
