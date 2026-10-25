// Turkish draughts (TÜDAF rules) move generator.
// Board: Int8Array(64), index = y*8 + x  (x: file a..h = 0..7, y: rank 1..8 = 0..7)
// 0 empty, 1 white man, 2 white king, -1 black man, -2 black king. White moves up (+y).
// Rules: men move/capture forward or sideways one square; kings fly orthogonally and land
// anywhere beyond the captured piece; capture is mandatory and the longest capture
// (counted in pieces) must be taken; captured pieces are removed immediately; no 180-degree
// turn between two captures; a man reaching the last row during a capture continues as a
// man and is crowned at the end of the move.
const Dama = (() => {
  const DIRS = [[0, 1], [0, -1], [1, 0], [-1, 0]]; // up, down, right, left
  const OPP = [1, 0, 3, 2];                         // index of the opposite direction

  function initial() {
    const b = new Int8Array(64);
    for (let x = 0; x < 8; x++) { b[8 + x] = 1; b[16 + x] = 1; b[40 + x] = -1; b[48 + x] = -1; }
    return b;
  }
  const sqName = (s) => 'abcdefgh'[s & 7] + ((s >> 3) + 1);
  const parseSq = (n) => (n.charCodeAt(0) - 97) + 8 * (parseInt(n.slice(1), 10) - 1);
  const inB = (x, y) => x >= 0 && x < 8 && y >= 0 && y < 8;

  // All legal moves for `side` (1 white, -1 black).
  // Move: {from, to, path:[squares landed on, starting with from], caps:[captured squares], promo}
  function moves(b, side) {
    const caps = captures(b, side);
    if (caps.length) return caps;
    const out = [];
    const fwd = side;
    const last = side === 1 ? 7 : 0;
    for (let s = 0; s < 64; s++) {
      const p = b[s];
      if (p === 0 || Math.sign(p) !== side) continue;
      const x = s & 7, y = s >> 3;
      if (Math.abs(p) === 1) {
        for (const [dx, dy] of [[0, fwd], [1, 0], [-1, 0]]) {
          const nx = x + dx, ny = y + dy;
          if (!inB(nx, ny)) continue;
          const t = ny * 8 + nx;
          if (b[t] === 0) out.push({ from: s, to: t, path: [s, t], caps: [], promo: ny === last });
        }
      } else {
        for (const [dx, dy] of DIRS) {
          let nx = x + dx, ny = y + dy;
          while (inB(nx, ny) && b[ny * 8 + nx] === 0) {
            const t = ny * 8 + nx;
            out.push({ from: s, to: t, path: [s, t], caps: [], promo: false });
            nx += dx; ny += dy;
          }
        }
      }
    }
    return out;
  }

  // Every maximum-length capture sequence for `side`.
  function captures(b, side) {
    let best = 0; let out = [];
    const last = side === 1 ? 7 : 0;
    const w = b.slice();
    const path = [], capsq = [];
    function leaf(isMan) {
      const n = capsq.length;
      if (n === 0) return;
      if (n > best) { best = n; out = []; }
      if (n === best) {
        const to = path[path.length - 1];
        out.push({ from: path[0], to, path: path.slice(), caps: capsq.slice(), promo: isMan && (to >> 3) === last });
      }
    }
    function manDfs(s, lastDir) {
      const x = s & 7, y = s >> 3; let any = false;
      const dirs = side === 1 ? [0, 2, 3] : [1, 2, 3]; // forward + sideways
      for (const di of dirs) {
        if (lastDir >= 0 && di === OPP[lastDir]) continue;
        const [dx, dy] = DIRS[di];
        const mx = x + dx, my = y + dy, lx = x + 2 * dx, ly = y + 2 * dy;
        if (!inB(lx, ly)) continue;
        const m = my * 8 + mx, l = ly * 8 + lx;
        if (w[m] !== 0 && Math.sign(w[m]) === -side && w[l] === 0) {
          const pc = w[s], cap = w[m];
          w[s] = 0; w[m] = 0; w[l] = pc; path.push(l); capsq.push(m);
          manDfs(l, di);
          path.pop(); capsq.pop(); w[l] = 0; w[m] = cap; w[s] = pc;
          any = true;
        }
      }
      if (!any) leaf(true);
    }
    function kingDfs(s, lastDir) {
      const x = s & 7, y = s >> 3; let any = false;
      for (let di = 0; di < 4; di++) {
        if (lastDir >= 0 && di === OPP[lastDir]) continue;
        const [dx, dy] = DIRS[di];
        let cx = x + dx, cy = y + dy;
        while (inB(cx, cy) && w[cy * 8 + cx] === 0) { cx += dx; cy += dy; }
        if (!inB(cx, cy)) continue;
        const m = cy * 8 + cx;
        if (Math.sign(w[m]) !== -side) continue;
        let lx = cx + dx, ly = cy + dy;
        while (inB(lx, ly) && w[ly * 8 + lx] === 0) {
          const l = ly * 8 + lx;
          const pc = w[s], cap = w[m];
          w[s] = 0; w[m] = 0; w[l] = pc; path.push(l); capsq.push(m);
          kingDfs(l, di);
          path.pop(); capsq.pop(); w[l] = 0; w[m] = cap; w[s] = pc;
          any = true;
          lx += dx; ly += dy;
        }
      }
      if (!any) leaf(false);
    }
    for (let s = 0; s < 64; s++) {
      const p = w[s];
      if (p === 0 || Math.sign(p) !== side) continue;
      path.length = 0; capsq.length = 0; path.push(s);
      if (Math.abs(p) === 1) manDfs(s, -1); else kingDfs(s, -1);
    }
    return out;
  }

  // Full capture tree of one piece (for teaching): every branch, not only the longest ones.
  // Node: {sq, cap (captured square or -1), dir, children:[], blocked180:[{cap, dir}]}
  function captureTree(b, from) {
    const side = Math.sign(b[from]);
    const isMan = Math.abs(b[from]) === 1;
    const w = b.slice();
    function build(s, lastDir, cap) {
      const node = { sq: s, cap, dir: lastDir, children: [], blocked180: [] };
      const x = s & 7, y = s >> 3;
      const dirIdx = isMan ? (side === 1 ? [0, 2, 3] : [1, 2, 3]) : [0, 1, 2, 3];
      for (const di of dirIdx) {
        const [dx, dy] = DIRS[di];
        if (isMan) {
          const mx = x + dx, my = y + dy, lx = x + 2 * dx, ly = y + 2 * dy;
          if (!inB(lx, ly)) continue;
          const m = my * 8 + mx, l = ly * 8 + lx;
          if (w[m] !== 0 && Math.sign(w[m]) === -side && w[l] === 0) {
            if (lastDir >= 0 && di === OPP[lastDir]) { node.blocked180.push({ cap: m, dir: di, land: l }); continue; }
            const pc = w[s], c = w[m];
            w[s] = 0; w[m] = 0; w[l] = pc;
            node.children.push(build(l, di, m));
            w[l] = 0; w[m] = c; w[s] = pc;
          }
        } else {
          let cx = x + dx, cy = y + dy;
          while (inB(cx, cy) && w[cy * 8 + cx] === 0) { cx += dx; cy += dy; }
          if (!inB(cx, cy)) continue;
          const m = cy * 8 + cx;
          if (Math.sign(w[m]) !== -side) continue;
          let lx = cx + dx, ly = cy + dy;
          let first = true;
          while (inB(lx, ly) && w[ly * 8 + lx] === 0) {
            const l = ly * 8 + lx;
            if (lastDir >= 0 && di === OPP[lastDir]) { if (first) node.blocked180.push({ cap: m, dir: di, land: l }); first = false; lx += dx; ly += dy; continue; }
            const pc = w[s], c = w[m];
            w[s] = 0; w[m] = 0; w[l] = pc;
            node.children.push(build(l, di, m));
            w[l] = 0; w[m] = c; w[s] = pc;
            lx += dx; ly += dy; first = false;
          }
        }
      }
      return node;
    }
    return build(from, -1, -1);
  }

  function apply(b, mv) {
    const n = b.slice();
    const pc = n[mv.from];
    n[mv.from] = 0;
    for (const c of mv.caps) n[c] = 0;
    n[mv.to] = mv.promo ? Math.sign(pc) * 2 : pc;
    return n;
  }

  function perft(b, side, depth) {
    if (depth === 0) return 1;
    const ms = moves(b, side);
    if (ms.length === 0) return 1;            // terminal node counts as one leaf (as in kish)
    if (depth === 1) return ms.length;
    let n = 0;
    for (const m of ms) n += perft(apply(b, m), -side, depth - 1);
    return n;
  }

  const key = (b, side) => String.fromCharCode(...Array.from(b, (v) => v + 70)) + (side === 1 ? 'w' : 'b');

  function notation(m) {
    if (m.caps.length) return m.path.map(sqName).join('x') + (m.promo ? '=K' : '');
    return sqName(m.from) + '-' + sqName(m.to) + (m.promo ? '=K' : '');
  }

  // Board from piece lists, e.g. pos(['d3'], ['d4','e5'], ['d3'])  (kings listed among their side)
  function pos(white, black, kings) {
    const b = new Int8Array(64);
    for (const s of white) b[parseSq(s)] = 1;
    for (const s of black) b[parseSq(s)] = -1;
    for (const s of kings || []) { const i = parseSq(s); b[i] = Math.sign(b[i]) * 2; }
    return b;
  }
  function play(seq, b0, side0) {
    let b = b0 || initial(), side = side0 || 1;
    for (const n of seq) {
      const m = moves(b, side).find((x) => notation(x) === n);
      if (!m) throw new Error('illegal ' + n);
      b = apply(b, m); side = -side;
    }
    return { b, side };
  }
  return { initial, moves, captures, captureTree, apply, perft, key, sqName, parseSq, notation, pos, play, DIRS, OPP };
})();
if (typeof module !== 'undefined') module.exports = Dama;
