// The search tree of one decision drawn as a tree that branches from left to right (docs/web-api.md, Trace).
// js/app.js opens it with the decision's trace and a few helpers. This file lays the tree out, draws it, pans and
// zooms it, and shows the node under the mouse on a small board; the engine is reached only through ctx.position.
(function (root) {
  'use strict';
  const NS = 'http://www.w3.org/2000/svg';
  const ROW = 30;   // vertical distance between two leaves
  const GAP = 46;   // horizontal room for the edges between two columns
  const CH = 22;    // height of a node
  const CW = 7.9;   // width of one character of the 13px monospace font
  const $ = (id) => document.getElementById(id);
  const el = (tag, attrs, parent) => {
    const n = document.createElementNS(NS, tag);
    for (const k in attrs || {}) n.setAttribute(k, attrs[k]);
    if (parent) parent.appendChild(n);
    return n;
  };

  let st = null;       // the tree on screen, null when closed
  let board = null;    // the small board, made on the first opening
  let wired = false;
  let lastFocus = null;

  // ---------- the trace ----------
  // A node's children, one per move: a move searched twice (an empty-window test, then a re-search) keeps the
  // search that counted, the last one.
  function kidsOf(n) {
    const byMove = new Map();
    for (const id of n.children || []) { const k = st.nodes[id]; if (k) byMove.set(k.move, k); }
    return [...byMove.values()];
  }
  // Legal moves that were never searched because the last one tried refuted the node (a cutoff).
  const prunedOf = (n, kids) => (n.cutoff ? Math.max(0, (n.legal || 0) - kids.length) : 0);
  const isMax = (n) => n.ply % 2 === 0; // the root side moves at even plies
  const parentOf = (n) => (n.parent === null || n.parent === undefined ? null : st.nodes[n.parent]);
  // The move that refuted its parent: the parent's last child, when the parent was cut off.
  function causedCut(n) {
    const p = parentOf(n);
    return !!(p && p.cutoff && p.children.length && p.children[p.children.length - 1] === n.id);
  }
  function lineOf(n) {
    const line = [];
    for (let x = n; x && parentOf(x); x = parentOf(x)) line.push(x.move);
    return line.reverse();
  }
  // The nodes of the expected line, from the root, as far as the recorded tree has them.
  function pvIds() {
    const ids = [0];
    let n = st.nodes[0];
    for (const mv of st.ctx.pv || []) {
      const k = kidsOf(n).find((c) => c.move === mv);
      if (!k) break;
      ids.push(k.id);
      n = k;
    }
    return ids;
  }

  // ---------- layout ----------
  function tagOf(n) {
    if (causedCut(n)) return { t: 'cut', c: 'cut' };
    if (n.tt === 'cut') return { t: 'table', c: 'tt' };
    if (n.draw) return { t: n.draw === 'repetition' ? 'rep.' : 'draw', c: 'draw' };
    if (n.extended) return { t: 'forced', c: 'q' };
    return null;
  }
  function chipOf(n) {
    const ctx = st.ctx;
    const label = n.move || 'root';
    const val = n.value === null || n.value === undefined ? '…' : ctx.boundSym(n.bound) + ctx.fmtV(n.value, { short: true });
    const kids = kidsOf(n);
    const tag = tagOf(n);
    const more = kids.length && !st.open.has(n.id) ? `+${kids.length}` : '';
    const w = 26 + label.length * CW + 14 + val.length * CW + (tag ? tag.t.length * 6.6 + 8 : 0) + (more ? more.length * 7 + 16 : 0) + 8;
    return { label, val, tag, more, kids, w: Math.round(w) };
  }
  // Leaves one under the other, a parent halfway between its first and last child, one column per ply.
  function layout() {
    const items = [], colW = [];
    let slot = 0;
    const visit = (n, depth) => {
      const c = chipOf(n);
      const it = { n, depth, c, w: c.w, kids: [] };
      colW[depth] = Math.max(colW[depth] || 0, it.w);
      if (st.open.has(n.id) && c.kids.length) {
        for (const k of c.kids) it.kids.push(visit(k, depth + 1));
        const pr = prunedOf(n, c.kids);
        if (pr) {
          const text = `${pr} not searched`;
          const s = { stub: pr, text, depth: depth + 1, w: Math.round(20 + text.length * 6.9), y: (slot++ + 0.5) * ROW, kids: [] };
          colW[depth + 1] = Math.max(colW[depth + 1] || 0, s.w);
          it.kids.push(s);
          items.push(s);
        }
        it.y = (it.kids[0].y + it.kids[it.kids.length - 1].y) / 2;
      } else {
        it.y = (slot++ + 0.5) * ROW;
      }
      items.push(it);
      return it;
    };
    const top = visit(st.nodes[0], 0);
    const colX = [];
    let x = 0;
    for (let d = 0; d < colW.length; d++) { colX[d] = x; x += (colW[d] || 0) + GAP; }
    for (const it of items) it.x = colX[it.depth];
    return { top, items, w: Math.max(1, x - GAP), h: Math.max(1, slot * ROW) };
  }

  // ---------- drawing ----------
  function draw() {
    const world = $('tmWorld');
    world.replaceChildren();
    const L = layout();
    st.L = L;
    const pv = new Set(st.pv);
    const gE = el('g', {}, world), gN = el('g', {}, world);
    for (const it of L.items) {
      if (it.stub) {
        const g = el('g', { class: 'tm-stub', transform: `translate(${it.x},${it.y})` }, gN);
        el('rect', { x: 0, y: -CH / 2, width: it.w, height: CH, rx: CH / 2 }, g);
        el('text', { x: 10, y: 4.5 }, g).textContent = it.text;
        continue;
      }
      for (const k of it.kids) {
        const x1 = it.x + it.w, y1 = it.y, x2 = k.x, y2 = k.y, mx = x1 + (x2 - x1) / 2;
        const onPv = !k.stub && pv.has(it.n.id) && pv.has(k.n.id);
        el('path', { d: `M${x1},${y1} C${mx},${y1} ${mx},${y2} ${x2},${y2}`, class: 'tm-edge' + (onPv ? ' pv' : '') + (k.stub ? ' stub' : '') }, gE);
      }
      const n = it.n, c = it.c;
      const cls = ['tm-node'];
      if (pv.has(n.id)) cls.push('pv');
      if (n.kind === 'quiescence') cls.push('q');
      if (n.tt === 'cut') cls.push('tt');
      if (st.sel === n.id) cls.push('sel');
      const g = el('g', { class: cls.join(' '), transform: `translate(${it.x},${it.y})`, 'data-id': n.id }, gN);
      el('rect', { x: 0, y: -CH / 2, width: it.w, height: CH, rx: 6, class: 'tm-chip' }, g);
      el('path', { d: isMax(n) ? 'M8,5 L14,-6 L20,5 Z' : 'M8,-5 L20,-5 L14,6 Z', class: isMax(n) ? 'tm-max' : 'tm-min' }, g);
      el('text', { x: 26, y: 4.5, class: 'tm-mv' }, g).textContent = c.label;
      let right = it.w - 8;
      if (c.more) {
        const pw = c.more.length * 7 + 10;
        const p = el('g', { class: 'tm-pill', transform: `translate(${right - pw},0)` }, g);
        el('rect', { x: 0, y: -8, width: pw, height: 16, rx: 8 }, p);
        el('text', { x: pw / 2, y: 4, 'text-anchor': 'middle' }, p).textContent = c.more;
        right -= pw + 6;
      }
      if (c.tag) {
        el('text', { x: right, y: 4, 'text-anchor': 'end', class: 'tm-tag ' + c.tag.c }, g).textContent = c.tag.t;
        right -= c.tag.t.length * 6.6 + 8;
      }
      el('text', { x: right, y: 4.5, 'text-anchor': 'end', class: 'tm-val' }, g).textContent = c.val;
    }
    apply();
  }

  // ---------- view: pan and zoom ----------
  function apply() { $('tmWorld').setAttribute('transform', `translate(${st.v.x},${st.v.y}) scale(${st.v.s})`); }
  // The whole tree in the window. Unless `all`, the scale stays readable (READABLE at least) and a tree too big for
  // it is centred on the node `focusId` instead; the Fit button and the 0 key ask for all of it.
  const READABLE = 0.75;
  function fit(all, focusId) {
    const cv = $('tmCanvas').getBoundingClientRect();
    const L = st.L, pad = 28;
    const whole = Math.min((cv.width - 2 * pad) / L.w, (cv.height - 2 * pad) / L.h, 1.3);
    const s = all ? Math.max(0.2, whole) : Math.max(READABLE, whole);
    const f = L.items.find((it) => it.n && it.n.id === focusId) || L.top;
    st.v.s = s;
    st.v.x = L.w * s <= cv.width - 2 * pad ? (cv.width - L.w * s) / 2 : Math.min(pad, cv.width / 2 - (f.x + f.w / 2) * s);
    st.v.y = L.h * s <= cv.height - 2 * pad ? (cv.height - L.h * s) / 2 : cv.height / 2 - f.y * s;
    apply();
  }
  function zoomAt(px, py, k) {
    const s2 = Math.min(2.5, Math.max(0.2, st.v.s * k));
    st.v.x = px - ((px - st.v.x) * s2) / st.v.s;
    st.v.y = py - ((py - st.v.y) * s2) / st.v.s;
    st.v.s = s2;
    apply();
  }
  // Opens or closes a node, keeping it where it is on the screen.
  function toggle(id) {
    const find = () => st.L.items.find((it) => it.n && it.n.id === id);
    const before = find();
    if (st.open.has(id)) st.open.delete(id); else if (kidsOf(st.nodes[id]).length) st.open.add(id);
    draw();
    const after = find();
    if (before && after) { st.v.x += (before.x - after.x) * st.v.s; st.v.y += (before.y - after.y) * st.v.s; apply(); }
  }

  // ---------- the node under the mouse ----------
  const stat = (k, v) => `<div class="stat"><div class="k">${k}</div><div class="v">${v}</div></div>`;
  function show(n) {
    const ctx = st.ctx;
    const line = lineOf(n);
    let pos = null, m = null;
    try {
      pos = ctx.position(line);
      if (line.length) {
        const prev = ctx.position(line.slice(0, -1));
        m = prev.legal.find((x) => x.move === line[line.length - 1]) || null;
      } else if (ctx.pv && ctx.pv.length) {
        m = pos.legal.find((x) => x.move === ctx.pv[0]) || null;
      }
    } catch (e) { pos = null; }
    if (pos) {
      board.setFlip(!!ctx.flip);
      board.setBoard(pos.board);
      board.setHighlights(m ? ctx.moveMarks(m, line.length ? 'last' : 'node') : []);
      const tone = !line.length || line.length % 2 === 1 ? 'turq' : 'coral';
      board.setArrows(m ? [{ path: m.path, tone, width: 13, opacity: 0.9, dash: !line.length }] : []);
      board.setLabels([]);
    }
    const kids = kidsOf(n), pruned = prunedOf(n, kids), max = isMax(n);
    const mover = max ? ctx.side : ctx.other(ctx.side);
    const end = (x, inf) => (x === null || x === undefined ? inf : ctx.fmtV(x));
    const depth = n.kind === 'quiescence' ? 'past the limit' : n.extended ? `${n.depth} (forced capture)` : String(n.depth);
    const crumbs = ['root'].concat(line).map((x) => `<span>${ctx.esc(x)}</span>`).join('<span class="sep">›</span>');
    const tri = max ? '<path d="M1,11 L7,1 L13,11 Z" class="tm-max"/>' : '<path d="M1,1 L13,1 L7,11 Z" class="tm-min"/>';
    const what = line.length
      ? `The board after ${ctx.esc(line[line.length - 1])}, the move that leads to this node.`
      : `The position where ${ctx.sideWord(ctx.side)} decided; the dashed arrow is the move it played.`;
    $('tmInfo').innerHTML = `<div class="tm-crumbs">${crumbs}</div>
      <div class="tm-role"><svg width="14" height="12" viewBox="0 0 14 12">${tri}</svg> ${max ? 'MAX' : 'MIN'} · ${ctx.sideWord(mover)} to move · ply ${n.ply}</div>
      <p class="tm-what">${what}</p>
      <div class="stats tm-stats">
        ${stat('Value', ctx.boundSym(n.bound) + ctx.fmtV(n.value))}
        ${stat('Window', `${end(n.alpha, '−∞')} … ${end(n.beta, '+∞')}`)}
        ${stat('Depth left', depth)}
        ${stat('Positions below', ctx.fmtInt(n.sub))}
        ${stat('Moves searched', `${kids.length} of ${n.legal ?? kids.length}`)}
      </div>
      <p class="tm-story">${ctx.story(n, kids, pruned, max)}</p>`;
  }

  // ---------- opening, closing, input ----------
  function wire() {
    if (wired) return;
    wired = true;
    const cv = $('tmCanvas'), svg = $('tmSvg');
    $('tmClose').addEventListener('click', close);
    $('tmFit').addEventListener('click', () => fit(true));
    $('tmCollapse').addEventListener('click', () => { st.open = new Set([0]); draw(); fit(false, 0); });
    $('tmPv').addEventListener('click', () => {
      for (const id of st.pv.slice(0, -1)) st.open.add(id);
      draw();
      fit(false, st.pv[Math.max(0, st.pv.length - 2)]);
    });
    $('treeModal').addEventListener('click', (e) => { if (e.target.id === 'treeModal') close(); });
    cv.addEventListener('wheel', (e) => {
      if (!st) return;
      e.preventDefault();
      const r = svg.getBoundingClientRect();
      zoomAt(e.clientX - r.left, e.clientY - r.top, Math.exp(-e.deltaY * 0.0015));
    }, { passive: false });
    let drag = null, dragged = false;
    svg.addEventListener('pointerdown', (e) => {
      if (!st || e.button !== 0) return;
      drag = { id: e.pointerId, x: e.clientX, y: e.clientY, vx: st.v.x, vy: st.v.y, moved: false };
      dragged = false;
    });
    svg.addEventListener('pointermove', (e) => {
      if (!st) return;
      if (drag) {
        const dx = e.clientX - drag.x, dy = e.clientY - drag.y;
        if (!drag.moved && Math.abs(dx) + Math.abs(dy) > 4) {
          drag.moved = true;
          cv.classList.add('dragging');
          try { svg.setPointerCapture(drag.id); } catch (err) { /* the pointer is gone */ }
        }
        if (drag.moved) { st.v.x = drag.vx + dx; st.v.y = drag.vy + dy; apply(); return; }
      }
      const g = e.target.closest ? e.target.closest('.tm-node') : null;
      const id = g ? +g.dataset.id : null;
      if (id !== null && id !== st.hover) { st.hover = id; show(st.nodes[id]); }
    });
    const end = () => {
      if (drag && drag.moved) { dragged = true; try { svg.releasePointerCapture(drag.id); } catch (err) { /* released */ } }
      drag = null;
      cv.classList.remove('dragging');
    };
    svg.addEventListener('pointerup', end);
    svg.addEventListener('pointercancel', end);
    svg.addEventListener('click', (e) => {
      if (!st) return;
      if (dragged) { dragged = false; return; }
      const g = e.target.closest ? e.target.closest('.tm-node') : null;
      if (!g) return;
      const id = +g.dataset.id;
      st.sel = id;
      toggle(id);
      show(st.nodes[id]);
    });
  }
  // Keys while the tree is open; js/app.js passes them on.
  function key(e) {
    if (!st) return;
    const r = $('tmSvg').getBoundingClientRect();
    if (e.key === 'Escape') { e.preventDefault(); close(); }
    else if (e.key === '+' || e.key === '=') { e.preventDefault(); zoomAt(r.width / 2, r.height / 2, 1.25); }
    else if (e.key === '-' || e.key === '_') { e.preventDefault(); zoomAt(r.width / 2, r.height / 2, 0.8); }
    else if (e.key === '0') { e.preventDefault(); fit(true); }
  }
  // ctx: nodes (a trace's), pv (moves), side (the root side), flip, title, sub, and the helpers
  // position(line), story(node, kids, pruned, max), fmtV, boundSym, fmtInt, esc, moveMarks, sideWord, other.
  function open(ctx) {
    wire();
    if (!board) board = new root.BoardView($('tmBoard'));
    st = { ctx, nodes: ctx.nodes, open: new Set([0]), v: { s: 1, x: 0, y: 0 }, sel: 0, hover: null, L: null, pv: [] };
    st.pv = pvIds();
    // The root and the first move of the expected line start open: every root move and every reply to the best.
    if (st.pv.length > 2) st.open.add(st.pv[1]);
    $('tmTitle').textContent = ctx.title || 'Search tree';
    $('tmSub').textContent = ctx.sub || '';
    lastFocus = document.activeElement;
    $('treeModal').hidden = false;
    $('treeModal').focus();
    draw();
    fit(false, st.pv.length > 2 ? st.pv[1] : 0);
    show(st.nodes[0]);
  }
  function close() {
    if (!st) return;
    $('treeModal').hidden = true;
    st = null;
    if (lastFocus && lastFocus.focus) lastFocus.focus();
  }
  root.TreeView = { open, close, key, isOpen: () => !!st };
})(window);
