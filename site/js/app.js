// Controller: game flow, the user view (what the agent considers and why) and the developer view
// (search, tree, evaluation, what if). All numbers come from the engine through docs/web-api.md.
(function () {
  'use strict';
  const $ = (id) => document.getElementById(id);
  const esc = (s) => String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const WIN = 1000;
  const START = '......../bbbbbbbb/bbbbbbbb/......../......../wwwwwwww/wwwwwwww/........';

  // ---------- opponents: five stages of our engine, each with its ladder Elo ----------
  const LEVELS = [
    { key: 'L1', level: 1, ver: 'V0', title: 'Level 1', what: 'A3 agent, one ply', spec: 'tuned' },
    { key: 'L2', level: 2, ver: 'V2', title: 'Level 2', what: 'alpha-beta, 5 plies', spec: 'ab5' },
    { key: 'L3', level: 3, ver: 'V6', title: 'Level 3', what: 'repetition-aware search', spec: 'idqr:n20000' },
    { key: 'L4', level: 4, ver: 'V8', title: 'Level 4', what: 'Texel-tuned evaluation', spec: 'idqr:n20000:w=texel1' },
    { key: 'L5', level: 5, ver: 'V10', title: 'Level 5', what: 'full engine', spec: 'idqrot:n20000:w=texel1' },
  ];
  // Search budget of the iterative-deepening levels (3 to 5): positions, time or depth. The ladder measured
  // every level at 20,000 positions a move. A depth budget also stops at DEPTH_CAP positions, since the page
  // waits for the engine while it searches.
  const LADDER_BUDGET = 'n20000';
  const DEPTH_CAP = 1000000;
  const BUDGETS = [
    { code: 'n2000', group: 'Positions', label: '2,000 positions' },
    { code: 'n20000', group: 'Positions', label: '20,000 positions (ladder)' },
    { code: 'n200000', group: 'Positions', label: '200,000 positions' },
    { code: 't100', group: 'Time', label: '0.1 s a move' },
    { code: 't500', group: 'Time', label: '0.5 s a move' },
    { code: 't1000', group: 'Time', label: '1 s a move' },
    { code: 't3000', group: 'Time', label: '3 s a move' },
    { code: 'd4', group: 'Depth', label: '4 plies' },
    { code: 'd6', group: 'Depth', label: '6 plies' },
    { code: 'd8', group: 'Depth', label: '8 plies' },
  ];
  const budgetPart = (code) => (code[0] === 'd' ? `:${code}:n${DEPTH_CAP}` : `:${code}`);
  const withBudget = (spec, code) => (/^id/.test(spec) ? spec.replace(/:n\d+/, budgetPart(code)) : spec);
  const budgetOptions = (current) => ['Positions', 'Time', 'Depth'].map((g) => {
    const off = g === 'Time' && !app.timeOk;
    return `<optgroup label="${g}${off ? ' (needs the engine update)' : ''}">` + BUDGETS.filter((b) => b.group === g)
      .map((b) => `<option value="${b.code}"${b.code === current ? ' selected' : ''}${off ? ' disabled' : ''}>${b.label}</option>`).join('') + '</optgroup>';
  }).join('');
  const LADDER = window.LADDER || { versions: [], meta: {} };
  const VER = {};
  for (const v of LADDER.versions || []) VER[v.id] = v;
  const levelOf = (key) => LEVELS.find((l) => l.key === key) || null;

  // ---------- formatting ----------
  const isWin = (v) => Math.abs(v) >= WIN - 200;
  function fmtV(v, opt = {}) {
    if (v === null || v === undefined || Number.isNaN(v)) return '—';
    if (isWin(v)) { const n = Math.max(1, Math.round(WIN - Math.abs(v))); return (v > 0 ? 'win' : 'loss') + (opt.short ? '' : ` in ${n}`); }
    const s = Math.abs(v) < 0.005 ? '0.00' : Math.abs(v).toFixed(2);
    return (v > 0.004 ? '+' : v < -0.004 ? '−' : '') + s;
  }
  const fmtElo = (e) => (e === null || e === undefined ? '—' : (e >= 0.5 ? '+' : e <= -0.5 ? '−' : '') + Math.round(Math.abs(e)));
  const fmtD = (d) => (d === null || d === undefined || Number.isNaN(d) ? '—' : Math.abs(d).toFixed(2)); // a difference: no sign
  const boundSym = (b) => (b === 'upper' ? '≤ ' : b === 'lower' ? '≥ ' : '');
  const fmtInt = (n) => (n === null || n === undefined ? '—' : Math.round(n).toLocaleString('en-US'));
  const pct = (a, b) => (b ? `${((100 * a) / b).toFixed(a / b < 0.1 ? 1 : 0)}%` : '—');
  const sideWord = (s) => (s === 'white' ? 'White' : 'Black');
  const other = (s) => (s === 'white' ? 'black' : 'white');
  const weightsOf = (spec) => { const m = /:w=([a-z0-9_]+)/.exec(spec || ''); return m ? m[1] : /^(tuned|first)$/.test(spec) ? spec : 'tuned'; };
  const lettersOf = (spec) => { const m = /^id(q?)(r?)(o?)(t?)/.exec(spec || ''); return m ? { q: !!m[1], r: !!m[2], o: !!m[3], t: !!m[4] } : null; };
  const nodesOf = (spec) => { const m = /:n(\d+)/.exec(spec || ''); return m ? +m[1] : null; };

  // ---------- engine ----------
  class Engine {
    constructor(session, info, kind) { this.s = session; this.info = info; this.kind = kind; }
    call(name, ...args) {
      const out = JSON.parse(this.s[name](...args));
      if (out && out.error) throw new Error(out.error);
      return out;
    }
    async slow(name, ...args) { await sleep(25); return this.call(name, ...args); } // lets the page paint first
  }
  // The Rust engine, compiled to WebAssembly (site/pkg/, docs/web-api.md). pkg/dama_web.js declares
  // `wasm_bindgen` with a top-level `let`, which is global to every script but not a property of window.
  async function createEngine() {
    const W = typeof wasm_bindgen !== 'undefined' ? wasm_bindgen : null; // eslint-disable-line no-undef
    if (!W) throw new Error('The engine is missing: pkg/dama_web.js did not load. Build it with scripts/build-web.sh.');
    const b64 = window.DAMA_WASM_B64; // set by tools/bundle.js in the one-file version
    const src = b64 ? Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)) : 'pkg/dama_web_bg.wasm';
    try {
      await W({ module_or_path: src });
    } catch (e) {
      if (location.protocol === 'file:') throw new Error('A page opened from disk cannot load pkg/dama_web_bg.wasm. Open dist/dama.html instead, or serve this folder: python -m http.server 8000');
      throw e;
    }
    return new Engine(new W.Session(), JSON.parse(W.version()), 'wasm');
  }

  // ---------- state ----------
  const app = {
    eng: null, board: null, st: null, legal: [],
    players: { white: 'L5', black: 'L2' }, seed: 1,
    auto: false, busy: false, speed: 1000, mode: 'user', tab: 'search',
    thinkingSide: null, decisions: [], sel: null, routes: null,
    showAll: false, tree: { path: [0], legalCache: new Map() }, heat: false,
    budget: LADDER_BUDGET, timeOk: true,
    wi: { q: true, r: true, o: true, t: true, w: 'texel1', budget: LADDER_BUDGET, result: null, busy: false, feat: { mob: true, adv: true, back: true, cen: true, edge: true, qmob: true } },
    evalWeights: null, previewNode: null, scene: null, linePreview: null,
  };
  const WI_DEFAULT = JSON.stringify(app.wi);
  const last = () => app.decisions[app.decisions.length - 1] || null;
  const isAgent = (side) => app.players[side] !== 'human';
  const over = () => app.st && app.st.status !== 'playing';

  // ---------- setup ----------
  function fillSelect(sel, side) {
    sel.replaceChildren();
    const h = document.createElement('option'); h.value = 'human'; h.textContent = 'Human'; sel.appendChild(h);
    for (const l of LEVELS) {
      const o = document.createElement('option');
      const v = VER[l.ver];
      o.value = l.key;
      o.textContent = `${l.title} · ${l.ver} ${l.what}` + (v ? ` · ${fmtElo(v.elo)} Elo` : '');
      sel.appendChild(o);
    }
    sel.value = app.players[side];
    sel.addEventListener('change', () => {
      app.players[side] = sel.value;
      if (isAgent(side)) setAgent(side);
      app.sel = null;
      render();
      kick();
    });
  }
  function setAgent(side) {
    const l = levelOf(app.players[side]);
    if (!l) return;
    app.eng.call('set_agent', side, withBudget(l.spec, app.budget), app.seed * 2 + (side === 'black' ? 1 : 0));
  }
  function fillBudget() {
    const sel = $('budgetSel');
    sel.innerHTML = budgetOptions(app.budget);
    sel.addEventListener('change', () => {
      app.budget = sel.value;
      for (const s of ['white', 'black']) if (isAgent(s)) setAgent(s);
      sel.blur();
      render();
    });
  }
  function clearGameState() {
    app.decisions = []; app.sel = null; app.routes = null; app.previewNode = null; app.linePreview = null; app.tree.path = [0];
    app.wi.result = null;
  }
  function newGame() {
    app.auto = false; app.seed++; app.scene = null;
    app.st = app.eng.call('reset', START, 'white');
    clearGameState();
    for (const s of ['white', 'black']) if (isAgent(s)) setAgent(s);
    refresh();
    render();
    kick();
  }

  // ---------- scenes: positions from our own games, prepared for the live demo (data/scenes.js) ----------
  const SCENES = window.SCENES || [];
  function loadScene(sc) {
    if (!sc || app.busy) return;
    app.auto = false; app.scene = sc;
    app.budget = LADDER_BUDGET; $('budgetSel').value = LADDER_BUDGET;
    app.players.white = sc.white; app.players.black = sc.black;
    $('whiteSel').value = sc.white; $('blackSel').value = sc.black;
    app.st = app.eng.call('reset', START, 'white');
    for (const m of sc.moves) app.eng.call('play', m);
    clearGameState();
    // Agents join after the moves, as in the scene check: they see the game so far, with fresh tables.
    for (const s of ['white', 'black']) {
      const l = levelOf(app.players[s]);
      if (l) app.eng.call('set_agent', s, withBudget(l.spec, LADDER_BUDGET), sc.seeds[s]);
    }
    app.wi = JSON.parse(WI_DEFAULT);
    if (sc.whatif) Object.assign(app.wi, sc.whatif);
    if (sc.mode) app.mode = sc.mode;
    if (sc.tab) app.tab = sc.tab;
    if (sc.speed) { app.speed = sc.speed; $('speedSel').value = String(sc.speed); }
    $('sceneSel').value = sc.id;
    refresh();
    render();
  }
  function fillScenes() {
    const sel = $('sceneSel');
    for (const sc of SCENES) {
      const o = document.createElement('option');
      o.value = sc.id; o.textContent = `${sc.key} · ${sc.title}`;
      sel.appendChild(o);
    }
    sel.hidden = !SCENES.length;
    sel.addEventListener('change', () => { const sc = SCENES.find((x) => x.id === sel.value); if (sc) loadScene(sc); sel.blur(); });
  }
  function renderScene() {
    const card = $('sceneCard');
    const sc = app.scene;
    card.hidden = !sc;
    document.querySelector('.board-col').classList.toggle('has-scene', !!sc);
    if (!sc) { $('sceneSel').value = ''; return; }
    card.innerHTML = `<div class="scene-head"><span class="badge">Scene ${esc(sc.key)}</span><b>${esc(sc.title)}</b><button class="scene-x" id="sceneX" title="Leave the scene" aria-label="Leave the scene">×</button></div>
      <div class="scene-note">${esc(sc.note)}</div><div class="scene-src">${esc(sc.source)}</div>`;
    $('sceneX').addEventListener('click', () => { app.scene = null; render(); });
  }
  function refresh() { app.st = app.eng.call('state'); app.legal = app.st.status === 'playing' ? app.eng.call('legal') : []; }

  // ---------- game flow ----------
  async function kick() {
    if (app.busy || over()) { if (over()) { app.auto = false; render(); } return; }
    const side = app.st.turn;
    if (!isAgent(side)) return;
    const humanInGame = !isAgent(other(side));
    if (!(app.auto || humanInGame)) return;
    await agentMove(side);
  }
  async function agentMove(side) {
    if (app.busy || over() || !isAgent(side) || app.st.turn !== side) return;
    app.busy = true; app.thinkingSide = side; app.sel = null; renderThinking(); renderControls();
    const lvl = levelOf(app.players[side]);
    let d;
    try {
      const opts = { trace: app.mode === 'dev' ? { iteration: -1, max_nodes: 80000 } : null };
      const report = await app.eng.slow('think', side, JSON.stringify(opts));
      const line = report.pv && report.pv.length ? report.pv : report.best ? [report.best] : [];
      const w = weightsOf(lvl.spec);
      const endEval = line.length ? app.eng.call('eval', w, JSON.stringify({ line })) : null;
      const nowEval = app.eng.call('eval', w, JSON.stringify({ line: [] }));
      const mv = app.legal.find((m) => m.move === report.best);
      d = { side, level: lvl, budget: /^id/.test(lvl.spec) ? app.budget : null, report, endEval, nowEval, move: mv, ply: app.st.ply, legal: app.legal.slice() };
      app.decisions.push(d);
      app.thinkingSide = null;
      app.tree.path = [0]; app.previewNode = null; app.wi.result = null;
      render();
      await sleep(Math.min(900, app.speed * 0.6));
      if (mv) await app.board.animate(mv, app.speed <= 100 ? 70 : 200);
      app.eng.call('play', report.best);
      refresh();
    } catch (e) {
      app.thinkingSide = null;
      showError(e);
    }
    app.busy = false;
    if (over()) app.auto = false;
    render();
    if (!over()) {
      if (app.auto && isAgent(app.st.turn) && isAgent(other(app.st.turn))) { await sleep(app.speed); kick(); }
      else kick();
    }
  }
  async function humanPlay(mv) {
    app.sel = null; app.routes = null;
    render();
    await app.board.animate(mv, 180);
    app.eng.call('play', mv.move);
    app.decisions.push({ side: app.st.turn, human: true, move: mv, ply: app.st.ply });
    refresh(); render();
    kick();
  }
  function undo() {
    if (app.busy) return;
    app.auto = false;
    const humans = ['white', 'black'].filter((s) => !isAgent(s));
    let n = 1;
    if (humans.length === 1) {
      // go back to the human's turn
      n = app.st.turn === humans[0] ? 2 : 1;
    }
    for (let i = 0; i < n && app.st.ply > 0; i++) {
      app.eng.call('undo');
      while (app.decisions.length && last().ply >= app.st.ply - 1 && last().ply >= app.eng.call('state').ply) app.decisions.pop();
      refresh();
    }
    app.decisions = app.decisions.filter((d) => d.ply < app.st.ply);
    app.sel = null; app.routes = null; app.previewNode = null; app.tree.path = [0];
    render();
  }

  // ---------- human input ----------
  function onSquare(sq) {
    if (app.busy || over() || isAgent(app.st.turn) || app.previewNode) return;
    const mine = app.legal.filter((m) => m.from === sq);
    if (!app.sel) {
      if (mine.length) { app.sel = { from: sq, moves: mine }; render(); }
      return;
    }
    if (sq === app.sel.from) { app.sel = null; app.routes = null; render(); return; }
    const hits = app.sel.moves.filter((m) => m.to === sq);
    if (hits.length === 1) return void humanPlay(hits[0]);
    if (hits.length > 1) { app.routes = hits; renderRoutes(); return; }
    if (mine.length) { app.sel = { from: sq, moves: mine }; app.routes = null; render(); return; }
    app.sel = null; app.routes = null; render();
  }
  function renderRoutes() {
    const wrap = $('boardWrap');
    wrap.querySelector('.routes')?.remove();
    if (!app.routes) return;
    const box = document.createElement('div');
    box.className = 'routes';
    box.innerHTML = '<div class="hint">More than one capture route ends there. Pick one:</div>';
    for (const m of app.routes) {
      const b = document.createElement('button');
      b.className = 'btn mono'; b.textContent = m.move;
      b.addEventListener('click', () => humanPlay(m));
      box.appendChild(b);
    }
    wrap.appendChild(box);
  }

  // ---------- rendering ----------
  function render() {
    renderControls();
    renderBoard();
    renderStatus();
    renderThinking();
    if (app.mode === 'user') renderUser(); else renderDev();
    renderRecord();
    renderScene();
  }
  function renderControls() {
    const both = isAgent('white') && isAgent('black');
    const ab = $('autoBtn');
    ab.textContent = app.auto ? 'Pause' : 'Play';
    ab.classList.toggle('on', app.auto);
    ab.disabled = !both || over();
    $('stepBtn').disabled = app.busy || over() || !isAgent(app.st.turn);
    $('undoBtn').disabled = app.busy || app.st.ply === 0;
    $('modeUser').setAttribute('aria-pressed', String(app.mode === 'user'));
    $('modeDev').setAttribute('aria-pressed', String(app.mode === 'dev'));
    $('userView').hidden = app.mode !== 'user';
    $('devView').hidden = app.mode !== 'dev';
  }
  function moveMarks(mv, kind) {
    if (!mv) return [];
    const out = mv.path.map((s, i) => ({ sq: s, kind: i === 0 || i === mv.path.length - 1 ? kind : 'path' }));
    for (const c of mv.captured || []) out.push({ sq: c, kind: 'captured' });
    return out;
  }
  function pvArrows(d, maxLen = 3) {
    if (!d || d.human || !d.report || !d.report.pv) return [];
    const pv = d.report.pv.slice(0, maxLen);
    const out = [];
    let line = [];
    for (let i = 0; i < pv.length; i++) {
      let pos;
      try { pos = app.eng.call('position', JSON.stringify({ back: app.st.ply - d.ply, line })); } catch (e) { break; }
      const m = pos.legal.find((x) => x.move === pv[i]);
      if (!m) break;
      const mover = i % 2 === 0 ? d.side : other(d.side);
      out.push({ path: m.path, tone: mover === d.side ? 'turq' : 'coral', dash: i % 2 === 1, width: i === 0 ? 14 : 10, opacity: i === 0 ? 0.9 : 0.6, label: String(i + 1) });
      line = line.concat(pv[i]);
    }
    return out;
  }
  function renderBoard() {
    const b = app.board;
    const banner = $('previewBanner');
    if (app.previewNode) {
      const p = app.previewNode;
      b.setBoard(p.board);
      b.setHighlights(p.marks || []);
      b.setArrows(p.arrows || []);
      b.setLabels([]);
      banner.hidden = false;
      banner.textContent = p.title;
      $('status').hidden = true;
      return;
    }
    banner.hidden = true;
    $('status').hidden = false;
    b.setBoard(app.st.board);
    const d = last();
    let marks = [];
    if (d && d.move && d.ply === app.st.ply - 1) marks = moveMarks(d.move, 'last');
    if (app.sel) {
      marks.push({ sq: app.sel.from, kind: 'sel' });
      for (const m of app.sel.moves) marks.push({ sq: m.to, kind: 'target' });
    } else if (!isAgent(app.st.turn) && !over() && !app.busy) {
      const froms = [...new Set(app.legal.map((m) => m.from))];
      for (const f of froms) marks.push({ sq: f, kind: 'path' });
    }
    b.setHighlights(marks);
    // arrows: the line the agent expects, drawn from the position where it decided
    const pending = d && !d.human && d.ply === app.st.ply && app.busy;
    if (d && !d.human && (pending || d.ply === app.st.ply - 1) && app.mode === 'user') {
      b.setArrows(pending ? pvArrows(d) : []);
    } else b.setArrows([]);
    if (app.heat && app.mode === 'dev' && app.tab === 'eval') {
      try {
        const w = app.evalWeights || 'texel1';
        const e = app.eng.call('eval', w, JSON.stringify({ line: [] }));
        const flip = e.side === 'white' ? 1 : -1;
        b.setLabels(e.squares.map((s) => ({ sq: s.square, text: fmtV(flip * s.contribution), tone: flip * s.contribution >= 0 ? 'turq' : 'coral' })));
      } catch (e) { b.setLabels([]); }
    } else b.setLabels([]);
  }
  function renderStatus() {
    const st = app.st;
    const el = $('status');
    const reasons = { no_moves: 'no legal move left', repetition: 'third repetition', fifty: '50 plies without a capture', one_each: 'one piece each', ply_cap: 'ply 400 reached' };
    if (st.status !== 'playing') {
      const res = st.status === 'draw' ? 'Draw' : `${sideWord(st.status)} wins`;
      el.innerHTML = `<span class="result">${res}</span><span>${esc(reasons[st.reason] || st.reason || '')} · ${st.ply} plies</span>`;
      return;
    }
    const who = isAgent(st.turn) ? `${levelOf(app.players[st.turn]).title} (${levelOf(app.players[st.turn]).ver})` : 'Human';
    const forced = app.legal.length && app.legal[0].captured.length ? ` · capture is compulsory (${app.legal[0].captured.length})` : '';
    el.innerHTML = `<span class="turn"><span class="dot ${st.turn === 'white' ? 'w' : 'b'}" style="width:14px;height:14px;border-radius:50%;border:2px solid var(--ink);display:inline-block;background:${st.turn === 'white' ? 'var(--piece-w)' : 'var(--piece-b)'}"></span>${sideWord(st.turn)} to move</span><span>${esc(who)}</span><span class="muted">ply ${st.ply} · ${st.quiet_plies} quiet · ${app.legal.length} legal${forced}</span>`;
  }
  function renderThinking() {
    const t = app.thinkingSide;
    for (const id of ['thinking', 'devThinking']) {
      $(id).hidden = !t;
      if (t) $(id).textContent = `${sideWord(t)} is thinking`;
    }
  }
  function renderRecord() {
    const mv = app.st.moves;
    const html = [];
    for (let i = 0; i < mv.length; i += 2) {
      const n = i / 2 + 1;
      const cls = (k) => 'm' + (k === mv.length - 1 ? ' last' : '');
      html.push(`<span class="n">${n}.</span><span class="${cls(i)}">${esc(mv[i])}</span><span class="${cls(i + 1)}">${mv[i + 1] ? esc(mv[i + 1]) : ''}</span>`);
    }
    for (const id of ['record', 'devRecord']) {
      const el = $(id);
      el.innerHTML = html.join('') || '<span class="muted" style="grid-column:1/-1">No moves yet</span>';
      el.scrollTop = el.scrollHeight;
    }
  }
  function agentTitle(d) {
    const l = d.level, v = VER[l.ver];
    return { name: `${sideWord(d.side)} · ${l.title} · ${v ? v.label : l.ver}`, meta: `${d.report.spec || l.spec}` };
  }

  // ---------- user view ----------
  function renderUser() {
    const d = [...app.decisions].reverse().find((x) => !x.human);
    const dot = $('whoDot');
    if (!d) {
      $('whoName').textContent = 'Waiting for an agent move';
      $('whoMeta').textContent = 'Choose the players above, then press Play or Step.';
      dot.className = 'dot';
      $('eloBadge').hidden = true;
      $('cands').innerHTML = '';
      $('candsNote').textContent = '';
      $('why').innerHTML = '<p class="muted">The explanation appears after an agent moves.</p>';
      $('evalW').style.width = '50%';
      $('evalText').textContent = '';
      return;
    }
    const t = agentTitle(d);
    dot.className = 'dot ' + (d.side === 'white' ? 'w' : 'b');
    $('whoName').textContent = t.name;
    const r = d.report;
    const wantDepth = d.budget && d.budget[0] === 'd' ? +d.budget.slice(1) : null;
    const short = wantDepth && r.depth < wantDepth ? ` (stopped at ${fmtInt(DEPTH_CAP)} positions, before ${wantDepth})` : '';
    $('whoMeta').textContent = r.depth > 1 ? `searched ${r.depth} plies${short} · ${fmtInt(r.nodes)} positions · ${Math.round(r.ms)} ms` : `looked one move ahead · ${r.root.length} moves`;
    const v = VER[d.level.ver];
    const eb = $('eloBadge');
    eb.hidden = !v;
    if (v) {
      const atLadder = !d.budget || d.budget === LADDER_BUDGET;
      eb.textContent = atLadder ? `${fmtElo(v.elo)} Elo vs V1 [${Math.round(v.ci[0])}, ${Math.round(v.ci[1])}]` : `${fmtElo(v.elo)} Elo at 20,000 positions`;
      eb.title = 'From our Elo ladder: 25 pairings, 6,800 games, every level at 20,000 positions a move (results/ladder/ladder.json)' + (atLadder ? '' : '. This search had another budget, so its strength differs.');
    }
    // evaluation bar from White's side
    const wv = d.side === 'white' ? r.value : -r.value;
    const p = isWin(wv) ? (wv > 0 ? 0.98 : 0.02) : 1 / (1 + Math.exp(-0.9 * wv));
    $('evalW').style.width = `${(p * 100).toFixed(1)}%`;
    $('evalText').textContent = isWin(wv) ? `${sideWord(wv > 0 ? 'white' : 'black')} has a forced win in its search.` : Math.abs(wv) < 0.05 ? 'About even, in its own judgement.' : `${sideWord(wv > 0 ? 'white' : 'black')} is ahead by ${Math.abs(wv).toFixed(2)} men, in its own judgement.`;
    renderCands(d);
    $('why').innerHTML = explain(d);
  }
  function renderCands(d) {
    const r = d.report;
    let rows = r.root.map((x) => ({ ...x }));
    rows.sort((a, b) => (b.move === r.best) - (a.move === r.best) || (b.value ?? -1e9) - (a.value ?? -1e9));
    const shown = app.showAll ? rows : rows.slice(0, 8);
    const ties = tiesOf(r);
    const vals = shown.filter((x) => x.value !== null && !isWin(x.value)).map((x) => x.value);
    const top = vals.length ? Math.max(...vals) : 0;
    const lo = Math.min(...vals, top - 0.5), hi = top + 0.02;
    const width = (v) => (v === null ? 0 : isWin(v) ? (v > 0 ? 100 : 3) : 6 + (94 * (v - lo)) / (hi - lo || 1));
    $('cands').innerHTML = shown.map((x) => {
      const chosen = x.move === r.best;
      const tie = !chosen && ties.has(x.move);
      const bound = x.bound === 'upper' || x.bound === 'lower';
      return `<div class="cand${chosen ? ' chosen' : ''}${bound ? ' bound' : ''}" data-move="${esc(x.move)}">
        <span class="mv">${esc(x.move)}${chosen ? '<span class="tag">played</span>' : tie ? '<span class="tag tie" title="Same value as the move played; the agent picked among the tied moves at random">tied</span>' : ''}</span>
        <span class="bar"><span class="fill" style="width:${width(x.value).toFixed(1)}%"></span></span>
        <span class="val">${x.value === null ? 'not searched' : boundSym(x.bound) + fmtV(x.value)}</span></div>`;
    }).join('') + (rows.length > 8 ? `<button class="more" id="moreBtn">${app.showAll ? 'Show fewer' : `Show all ${rows.length}`}</button>` : '');
    const mb = $('moreBtn');
    if (mb) mb.addEventListener('click', () => { app.showAll = !app.showAll; renderUser(); });
    const anyBound = rows.some((x) => x.bound === 'upper');
    $('candsNote').innerHTML = r.depth > 1
      ? `Values are in men, from ${sideWord(d.side)}'s side, after a ${r.depth}-ply search.` + (anyBound ? ' Striped bars are upper bounds: alpha-beta proved those moves worse than the best one without working out their exact value.' : '')
      : `Values are U scores of the board after each move (men, kings and mobility), the Assignment 1 utility.`;
    for (const row of $('cands').querySelectorAll('.cand')) {
      row.addEventListener('mouseenter', () => {
        const m = d.legal.find((x) => x.move === row.dataset.move);
        if (m && d.ply === app.st.ply) app.board.setArrows([{ path: m.path, tone: 'cobalt', width: 12, opacity: 0.85 }]);
      });
      row.addEventListener('mouseleave', () => renderBoard());
    }
  }
  function topFeatures(e, side, n = 3) {
    if (!e) return [];
    const s = e.side === side ? 1 : -1;
    return e.features.map((f) => ({ name: f.name, c: s * f.contribution })).filter((f) => Math.abs(f.c) >= 0.005).sort((a, b) => Math.abs(b.c) - Math.abs(a.c)).slice(0, n);
  }
  const tiesOf = (r) => new Set(r.ties && r.ties.length ? r.ties : [r.best]);
  function tieText(r) {
    if (!r.ties || r.ties.length < 2) return '';
    const list = r.ties.map((m) => `<span class="line">${esc(m)}</span>`).join(' ');
    return `<p>${r.ties.length} moves tie for the top value: ${list}. The agent picks one of them at random, from its seed for this game, exactly as in our arena and ladder games.</p>`;
  }
  function explain(d) {
    const r = d.report, side = d.side, who = sideWord(side);
    const out = [];
    const ties = tiesOf(r);
    const caps = d.legal.length && d.legal[0].captured.length;
    if (d.legal.length === 1) out.push(`<p>Only one legal move: <span class="line">${esc(r.best)}</span>.</p>`);
    else if (caps) out.push(`<p>Capture is compulsory here: all ${d.legal.length} legal moves take ${caps} piece${caps > 1 ? 's' : ''}, the most any capture can.</p>`);
    if (r.depth <= 1) {
      const chosen = r.root.find((x) => x.move === r.best);
      const rest = r.root.filter((x) => !ties.has(x.move)).sort((a, b) => b.value - a.value);
      out.push(`<p>This level looks one move ahead. It scores the board after each move with U = men + ${weightsOf(d.level.spec) === 'tuned' ? '2.5' : '3'} × kings − 0.05 × mobility and plays the highest score: <span class="line">${esc(r.best)}</span> at ${fmtV(chosen ? chosen.value : r.value)}.</p>`);
      out.push(tieText(r));
      if (rest.length) out.push(`<p>Next best: <span class="line">${esc(rest[0].move)}</span> at ${fmtV(rest[0].value)}. It cannot see the reply, so a capture that comes back one move later is invisible to it.</p>`);
      return out.join('');
    }
    const line = r.pv.map((m, i) => `<span class="line">${esc(m)}</span>`).join(' ');
    const qShare = r.nodes ? (100 * r.qnodes) / r.nodes : 0;
    out.push(`<p>${who} searched <b>${r.depth} plies</b> ahead, ${fmtInt(r.nodes)} positions, ${qShare.toFixed(0)}% of them in capture sequences past the depth limit. It expects ${line}.</p>`);
    if (isWin(r.value)) out.push(`<p>That line ${r.value > 0 ? 'wins' : 'loses'} by force: the opponent ${r.value > 0 ? 'runs out of moves' : 'cannot be stopped'} within the search.</p>`);
    else {
      const feats = topFeatures(d.endEval, side);
      const chips = feats.map((f) => `<span class="chip ${f.c >= 0 ? 'pos' : 'neg'}">${esc(f.name)} <b>${fmtV(f.c)}</b></span>`).join('');
      out.push(`<p>At the end of that line it judges the position <b>${fmtV(r.value)}</b> for ${who}. The biggest parts of that judgement:</p><div class="feat-chips">${chips || '<span class="chip">all features balanced</span>'}</div>`);
    }
    out.push(tieText(r));
    const rest = r.root.filter((x) => !ties.has(x.move) && x.value !== null).sort((a, b) => b.value - a.value);
    if (rest.length) {
      const x = rest[0];
      if (x.bound === 'upper') out.push(`<p style="margin-top:10px">The next candidate, <span class="line">${esc(x.move)}</span>, is at most ${fmtV(x.value)}: once the search proved it could not beat ${esc(r.best)}, it stopped looking for its exact value.</p>`);
      else out.push(`<p style="margin-top:10px">The next candidate, <span class="line">${esc(x.move)}</span>, scores ${fmtV(x.value)}, ${fmtD(r.value - x.value)} below the move it played.</p>`);
    }
    return out.join('');
  }

  // ---------- developer view ----------
  function renderDev() {
    const d = [...app.decisions].reverse().find((x) => !x.human);
    if (d) {
      const t = agentTitle(d);
      $('devDot').className = 'dot ' + (d.side === 'white' ? 'w' : 'b');
      $('devName').textContent = t.name;
      const back = app.st.ply - d.ply;
      $('devMeta').textContent = `${t.meta} · decision at ply ${d.ply}` + (back > 0 ? ` (${back} ${back === 1 ? 'ply' : 'plies'} ago)` : '');
    } else {
      $('devDot').className = 'dot'; $('devName').textContent = 'No agent decision yet'; $('devMeta').textContent = 'Let an agent move; its search appears here.';
    }
    for (const b of document.querySelectorAll('.tabs button')) b.setAttribute('aria-selected', String(b.dataset.tab === app.tab));
    for (const p of document.querySelectorAll('[data-panel]')) p.hidden = p.dataset.panel !== app.tab;
    const panel = document.querySelector(`[data-panel="${app.tab}"]`);
    if (app.tab === 'search') panel.innerHTML = searchPanel(d);
    if (app.tab === 'tree') treePanel(panel, d);
    if (app.tab === 'eval') evalPanel(panel, d);
    if (app.tab === 'whatif') whatIfPanel(panel, d);
  }
  const stat = (k, v, title) => `<div class="stat"${title ? ` title="${esc(title)}"` : ''}><div class="k">${esc(k)}</div><div class="v">${v}</div></div>`;
  function searchPanel(d) {
    if (!d) return '<p class="hint">No search yet.</p>';
    const r = d.report, s = r.stats || {};
    const head = `<div class="stats">
      ${stat('Depth', r.depth)}
      ${stat('Positions', fmtInt(r.nodes))}
      ${stat('In captures', pct(r.qnodes, r.nodes), 'Quiescence: capture sequences searched past the depth limit')}
      ${stat('Table hits', s.tt_probes ? pct(s.tt_hits, s.tt_probes) : 'off', 'Transposition table: positions met before')}
      ${stat('First-move cuts', s.beta_cutoffs ? pct(s.first_move_cutoffs, s.beta_cutoffs) : '—', 'Share of cutoffs made by the first move tried: how good the move ordering is')}
      ${stat('Time', `${Math.round(r.ms)} ms`)}
    </div>`;
    const it = r.iterations.map((x) => `<tr${x.depth === r.depth ? ' class="best"' : ''}><td class="r">${x.depth}</td><td class="mv">${esc(x.best || '')}</td><td class="r mono">${fmtV(x.value)}</td><td class="r num">${fmtInt(x.nodes)}</td><td class="pv">${esc((x.pv || []).join(' '))}</td></tr>`).join('');
    const roots = r.root.slice().sort((a, b) => a.order - b.order).map((x) => `<tr${x.move === r.best ? ' class="best"' : ''}><td class="r">${x.order}</td><td class="mv">${esc(x.move)}</td><td><span class="reason ${esc(x.reason)}">${esc(reasonText(x.reason))}</span></td><td class="r mono">${x.value === null ? '—' : boundSym(x.bound) + fmtV(x.value)}</td><td class="r num">${fmtInt(x.nodes)}</td></tr>`).join('');
    const ties = r.ties && r.ties.length > 1
      ? `<p class="hint">${r.ties.length} moves tie at the top of the deciding iteration (${r.ties.map(esc).join(', ')}). The table shows the first of them; the agent picked <b class="mono">${esc(r.best)}</b> among them at random, from its seed.</p>`
      : '';
    const unfinished = r.nodes > (r.iterations.length ? r.iterations[r.iterations.length - 1].nodes : 0)
      ? `<p class="hint">The next iteration ran out of budget after ${fmtInt(r.nodes - r.iterations[r.iterations.length - 1].nodes)} more positions and was thrown away: only complete iterations count.</p>`
      : '';
    return head + `<div class="section-h"><h3>Iterative deepening</h3><span class="hint">each row is one complete search, one ply deeper; the last complete one decides</span></div>
      <div class="tbl-wrap"><table class="tbl"><thead><tr><th class="r">Depth</th><th>Best</th><th class="r">Value</th><th class="r">Positions so far</th><th>Expected line</th></tr></thead><tbody>${it}</tbody></table></div>${unfinished}${ties}
      <div class="section-h"><h3>Root moves in the order tried</h3><span class="hint">≤ marks an upper bound</span></div>
      <div class="tbl-wrap"><table class="tbl"><thead><tr><th class="r">#</th><th>Move</th><th>Why tried there</th><th class="r">Value</th><th class="r">Positions</th></tr></thead><tbody>${roots}</tbody></table></div>`;
  }
  const reasonText = (r) => ({ tt: 'table move', pv: 'last best', killer: 'killer', history: 'history', promotion: 'promotion', capture: 'capture', none: 'plain order' }[r] || r || '—');

  // tree explorer
  function treePanel(panel, d) {
    if (!d) { panel.innerHTML = '<p class="hint">No search yet.</p>'; return; }
    const tr = d.report.trace;
    if (!tr || !tr.nodes || !tr.nodes.length) {
      panel.innerHTML = `<p class="hint">This decision was made before the developer view was open, so its search tree was not recorded.</p>
        <button class="btn" id="recBtn">Replay this decision with a fresh agent and record the tree</button>
        <p class="hint" style="margin-top:8px">The replay uses the same settings but an empty transposition table, so it can differ slightly from the search that chose the move. Decisions made while this view is open are recorded exactly.</p>`;
      $('recBtn').addEventListener('click', async () => {
        $('recBtn').disabled = true;
        try {
          const rep = await app.eng.slow('analyze', d.level.spec, JSON.stringify({ back: app.st.ply - d.ply, seed: 1, trace: { iteration: -1, max_nodes: 80000 } }));
          d.report.trace = rep.trace; d.report.traceReplay = true;
        } catch (e) { showError(e); }
        renderDev();
      });
      return;
    }
    const nodes = tr.nodes;
    const path = app.tree.path.filter((id) => nodes[id]);
    if (!path.length) path.push(0);
    app.tree.path = path;
    const cur = nodes[path[path.length - 1]];
    const line = path.slice(1).map((id) => nodes[id].move);
    const back = app.st.ply - d.ply;
    let legal = [];
    try { legal = app.eng.call('position', JSON.stringify({ back, line })).legal; } catch (e) { legal = []; }
    const crumbs = [`<button data-depth="0"${path.length === 1 ? ' aria-current="true"' : ''}>root</button>`]
      .concat(path.slice(1).map((id, i) => `<span class="sep">›</span><button data-depth="${i + 1}"${i + 2 === path.length ? ' aria-current="true"' : ''}>${esc(nodes[id].move)}</button>`)).join('');
    const maxNode = cur.ply % 2 === 0;
    const roleWord = maxNode ? `MAX (${sideWord(d.side)} to move)` : `MIN (${sideWord(other(d.side))} to move)`;
    const win = (x) => (x === null ? (x === cur.alpha ? '−∞' : '+∞') : fmtV(x));
    const card = `<div class="nodecard">
      ${stat('Node', `${cur.ply === 0 ? 'root' : 'ply ' + cur.ply}`)}
      ${stat('Turn', maxNode ? 'MAX' : 'MIN', roleWord)}
      ${stat('Depth left', cur.depth > 0 ? cur.depth : 'capture ext.', cur.kind)}
      ${stat('Window', `${cur.alpha === null ? '−∞' : fmtV(cur.alpha)} … ${cur.beta === null ? '+∞' : fmtV(cur.beta)}`, 'alpha … beta, from the root side')}
      ${stat('Value', `${boundSym(cur.bound)}${fmtV(cur.value)}`)}
      ${stat('Positions below', fmtInt(cur.sub))}
    </div>`;
    void win;
    const kids = cur.children.map((id) => nodes[id]);
    const cutKid = cur.cutoff ? kids[kids.length - 1] : null;
    const searched = new Set(kids.map((k) => k.move));
    const pruned = legal.filter((m) => !searched.has(m.move));
    const rows = kids.map((k, i) => `<tr class="click${cutKid === k ? '' : ''}" data-id="${k.id}"><td class="r">${i + 1}</td><td class="mv">${esc(k.move)}</td><td><span class="reason ${esc(k.order || 'none')}">${esc(reasonText(k.order))}</span></td><td class="r mono">${boundSym(k.bound)}${fmtV(k.value)}</td><td class="r num">${fmtInt(k.sub)}</td><td>${cutKid === k ? '<span class="cutmark">cutoff</span>' : k.draw ? esc(k.draw) : k.tt === 'cut' ? 'from table' : ''}</td></tr>`).join('')
      + pruned.map((m) => `<tr class="dim"><td class="r">—</td><td class="mv">${esc(m.move)}</td><td colspan="4">not searched: cut off</td></tr>`).join('');
    panel.innerHTML = `<p class="hint">${d.report.traceReplay ? 'Replayed with a fresh agent (empty table). ' : 'The exact search that chose the move. '}Iteration ${tr.iteration}, ${fmtInt(tr.nodes_total)} positions${tr.truncated ? ', cut short for display' : ''}. Values are from ${sideWord(d.side)}'s side: MAX picks the highest, MIN the lowest.</p>
      <div class="crumbs">${crumbs}</div>${card}
      <div class="why-node">${nodeStory(cur, kids, pruned.length, maxNode)}</div>
      <div class="tbl-wrap"><table class="tbl"><thead><tr><th class="r">#</th><th>Move</th><th>Why tried there</th><th class="r">Value</th><th class="r">Positions</th><th></th></tr></thead><tbody>${rows || '<tr><td colspan="6" class="muted">Leaf: no moves searched below this node.</td></tr>'}</tbody></table></div>`;
    for (const b of panel.querySelectorAll('.crumbs button')) b.addEventListener('click', () => { app.tree.path = app.tree.path.slice(0, +b.dataset.depth + 1); renderTreePreview(d); renderDev(); });
    for (const tr2 of panel.querySelectorAll('tr.click')) tr2.addEventListener('click', () => { app.tree.path = app.tree.path.concat(+tr2.dataset.id); renderTreePreview(d); renderDev(); });
    renderTreePreview(d, true);
  }
  function nodeStory(n, kids, nPruned, maxNode) {
    if (n.draw === 'repetition') return 'This position already occurred on the way here, so it counts as a draw (0) without searching further.';
    if (n.draw === 'one_each') return 'One piece each: the game would end in a draw here, so the value is 0.';
    if (n.tt === 'cut') return 'Answered from the transposition table: this position was searched before, at least as deep, so its stored value is used.';
    if (!kids.length) {
      if (n.static !== null && n.static !== undefined) return `The depth limit is reached and no capture is pending, so the position gets its static evaluation: <b>${fmtV(n.static)}</b>.`;
      if (n.value !== null && isWin(n.value)) return 'No legal move for the side to move: the game is over here.';
      return 'A leaf of the search.';
    }
    const best = kids.reduce((a, b) => (maxNode ? (b.value > a.value ? b : a) : (b.value < a.value ? b : a)));
    let s = `${maxNode ? 'MAX' : 'MIN'} tried ${kids.length} of ${kids.length + nPruned} moves`;
    if (n.kind === 'quiescence') s += ' (a capture sequence past the depth limit)';
    s += `. ${maxNode ? 'Highest' : 'Lowest'}: <b>${esc(best.move)}</b> at ${boundSym(best.bound)}${fmtV(best.value)}.`;
    if (n.cutoff && nPruned > 0) {
      const k = kids[kids.length - 1];
      s += maxNode
        ? ` After ${esc(k.move)} reached ${fmtV(k.value)}, at least the window's top (${n.beta === null ? '+∞' : fmtV(n.beta)}), MIN above would never allow this position, so the other ${nPruned} move${nPruned > 1 ? 's were' : ' was'} skipped.`
        : ` After ${esc(k.move)} fell to ${fmtV(k.value)}, at most the window's bottom (${n.alpha === null ? '−∞' : fmtV(n.alpha)}), MAX above would never choose this line, so the other ${nPruned} move${nPruned > 1 ? 's were' : ' was'} skipped.`;
    }
    return s;
  }
  function renderTreePreview(d, keepIfSame) {
    if (app.mode !== 'dev' || app.tab !== 'tree' || !d || !d.report.trace) { if (app.previewNode) { app.previewNode = null; renderBoard(); } return; }
    const nodes = d.report.trace.nodes;
    const path = app.tree.path;
    const line = path.slice(1).map((id) => nodes[id].move);
    const key = line.join(' ');
    if (keepIfSame && app.previewNode && app.previewNode.key === key) return;
    const back = app.st.ply - d.ply;
    try {
      const pos = app.eng.call('position', JSON.stringify({ back, line }));
      let marks = [], arrows = [];
      if (line.length) {
        const prev = app.eng.call('position', JSON.stringify({ back, line: line.slice(0, -1) }));
        const m = prev.legal.find((x) => x.move === line[line.length - 1]);
        if (m) marks = moveMarks(m, 'last');
      }
      const cur = nodes[path[path.length - 1]];
      const kids = cur.children.map((id) => nodes[id]);
      if (kids.length) {
        const maxNode = cur.ply % 2 === 0;
        const best = kids.reduce((a, b) => (maxNode ? (b.value > a.value ? b : a) : (b.value < a.value ? b : a)));
        const m = pos.legal.find((x) => x.move === best.move);
        if (m) arrows.push({ path: m.path, tone: pos.turn === d.side ? 'turq' : 'coral', width: 12, opacity: 0.8 });
      }
      app.previewNode = { key, board: pos.board, marks, arrows, title: line.length ? `Search tree preview · ${line.length} ${line.length === 1 ? 'ply' : 'plies'} into the search` : 'Search tree preview · the decision position' };
    } catch (e) { app.previewNode = null; }
    renderBoard();
  }

  function evalPanel(panel, d) {
    const w = app.evalWeights || (d ? weightsOf(d.level.spec) : 'texel1');
    app.evalWeights = w;
    let now, end = null;
    try {
      now = app.eng.call('eval', w, JSON.stringify({ line: [] }));
      if (d && d.report.pv && d.report.pv.length) end = app.eng.call('eval', w, JSON.stringify({ back: app.st.ply - d.ply, line: d.report.pv }));
    } catch (e) { panel.innerHTML = `<p class="hint">${esc(e.message)}</p>`; return; }
    const table = (e, title) => {
      if (!e) return '';
      const s = e.side === 'white' ? 1 : -1;
      const rows = e.features.map((f) => `<tr><td>${esc(f.name)}</td><td class="r num">${fmtNum(f.white)}</td><td class="r num">${fmtNum(f.black)}</td><td class="r mono">${f.weight.toFixed(3)}</td><td class="r mono">${fmtV(s * f.contribution)}</td></tr>`).join('');
      return `<div><div class="section-h"><h3>${esc(title)}</h3><span class="mono">${fmtV(s * e.total)}</span></div>
        <div class="tbl-wrap"><table class="tbl"><thead><tr><th>Feature</th><th class="r">White</th><th class="r">Black</th><th class="r">Weight</th><th class="r">For White</th></tr></thead><tbody>${rows}</tbody></table></div></div>`;
    };
    panel.innerHTML = `<div class="wi-controls">
        <label>Weights <select id="evW">${['texel1', 'tuned', 'first'].map((x) => `<option${x === w ? ' selected' : ''}>${x}</option>`).join('')}</select></label>
        <label class="toggle"><input type="checkbox" id="heatT"${app.heat ? ' checked' : ''}> Show each piece's value on the board</label>
      </div>
      <p class="hint">The evaluation is a weighted sum: each feature counts White minus Black, times its weight. Positive numbers favour White.</p>
      <div class="eval-grid">${table(now, 'Board now')}${table(end, 'End of the expected line')}</div>`;
    $('evW').addEventListener('change', (ev) => { app.evalWeights = ev.target.value; renderDev(); renderBoard(); });
    $('heatT').addEventListener('change', (ev) => { app.heat = ev.target.checked; renderBoard(); });
  }
  const fmtNum = (x) => (Number.isInteger(x) ? String(x) : x.toFixed(2));

  function whatIfPanel(panel, d) {
    const wi = app.wi;
    const base = d && lettersOf(d.level.spec) ? d.level : levelOf('L5');
    const FEATS = wi.w === 'texel1' ? [['mob', 'mobility'], ['adv', 'advancement'], ['back', 'back row'], ['cen', 'centre'], ['edge', 'edge'], ['qmob', 'quiet mobility']] : [['mob', 'mobility']];
    const off = FEATS.filter(([k]) => !wi.feat[k]).map(([k]) => `:${k}=0`).join('');
    const spec = `id${wi.q ? 'q' : ''}${wi.r ? 'r' : ''}${wi.o ? 'o' : ''}${wi.t ? 't' : ''}${budgetPart(wi.budget)}:w=${wi.w}${off}`;
    const back = d ? app.st.ply - d.ply : 0;
    const box = (k, label, title) => `<label class="toggle" title="${esc(title)}"><input type="checkbox" data-k="${k}"${wi[k] ? ' checked' : ''}> ${label} ${ablBadge(k)}</label>`;
    let res = '';
    if (wi.result) {
      const R = wi.result;
      const same = R.alt.best === R.full.best;
      res = `<div class="compare">
        <div class="cmp${same ? '' : ' diff'}"><div class="hint" style="margin:0">With these settings</div><div class="big">${esc(R.alt.best)}</div><div class="muted">${fmtV(R.alt.value)} in its own eyes · depth ${R.alt.depth} · ${fmtInt(R.alt.nodes)} positions</div></div>
        <div class="cmp"><div class="hint" style="margin:0">Full engine (${esc(R.fullSpec)})</div><div class="big">${esc(R.full.best)}</div><div class="muted">${fmtV(R.full.value)} · depth ${R.full.depth} · ${fmtInt(R.full.nodes)} positions</div></div>
      </div>` + (same
        ? `<div class="verdict same">Both choose the same move here: switching those parts off costs nothing in this position. The badges above give the average cost over 400 games.</div>`
        : verdict(R));
    }
    panel.innerHTML = `<p class="hint">Switch parts of the engine off and search the ${d ? `decision position (${back ? back + ' ' + (back === 1 ? 'ply' : 'plies') + ' ago' : 'now'})` : 'current position'} again: our ablation, one position at a time. Each badge is what switching that part off cost the full engine over 400 games.</p>
      <div class="wi-controls">
        ${box('q', 'Quiescence', 'Keep searching capture sequences past the depth limit')}
        ${box('r', 'Repetition', 'A repeated position counts as a draw inside the search')}
        ${box('o', 'Move ordering', 'Previous best move, promotions, killers and history first')}
        ${box('t', 'Table', 'Transposition table')}
        <label>Weights <select id="wiW">${['texel1', 'tuned'].map((x) => `<option${x === wi.w ? ' selected' : ''}>${x}</option>`).join('')}</select> ${ablBadge('w')}</label>
        <label>Budget <select id="wiN">${budgetOptions(wi.budget)}</select></label>
      </div>
      <div class="wi-controls"><span class="hint" style="margin:0">Evaluation features:</span>${FEATS.map(([k, label]) => `<label class="toggle"><input type="checkbox" data-f="${k}"${wi.feat[k] ? ' checked' : ''}> ${label}</label>`).join('')}</div>
      <div class="wi-controls"><span class="wi-spec">${esc(spec)}</span><button class="btn primary" id="wiGo"${wi.busy ? ' disabled' : ''}>${wi.busy ? 'Searching' : 'Search this position'}</button></div>
      ${res}`;
    for (const c of panel.querySelectorAll('input[data-k]')) c.addEventListener('change', () => { wi[c.dataset.k] = c.checked; wi.result = null; renderDev(); });
    for (const c of panel.querySelectorAll('input[data-f]')) c.addEventListener('change', () => { wi.feat[c.dataset.f] = c.checked; wi.result = null; renderDev(); });
    $('wiW').addEventListener('change', (e) => { wi.w = e.target.value; wi.result = null; renderDev(); });
    $('wiN').addEventListener('change', (e) => { wi.budget = e.target.value; wi.result = null; renderDev(); });
    $('wiGo').addEventListener('click', async () => {
      wi.busy = true; renderDev();
      try {
        const fullSpec = withBudget(base.spec, wi.budget);
        const alt = await app.eng.slow('analyze', spec, JSON.stringify({ back, seed: 1 }));
        const full = await app.eng.slow('analyze', fullSpec, JSON.stringify({ back, seed: 1 }));
        let pair = null, line = null;
        if (alt.best !== full.best) {
          // Both moves in one search, so their values come from the same depth; then the
          // alternative alone, for the line the full engine expects after it.
          pair = await app.eng.slow('analyze', fullSpec, JSON.stringify({ back, seed: 1, only: [alt.best, full.best] }));
          line = await app.eng.slow('analyze', fullSpec, JSON.stringify({ back, seed: 1, only: [alt.best] }));
        }
        wi.result = { alt, full, pair, line, fullSpec, ply: app.st.ply - back };
        wi.lineK = 0; app.previewNode = null;
      } catch (e) { showError(e); }
      wi.busy = false; render();
    });
    const lp = $('lpNext');
    if (lp) {
      lp.addEventListener('click', () => stepLine(1));
      $('lpPrev').addEventListener('click', () => stepLine(-1));
    }
  }

  // Walks the line the full engine expects after the alternative, on the board.
  function stepLine(delta) {
    const R = app.wi.result;
    if (!R || !R.line || !R.line.pv) return;
    app.wi.lineK = Math.max(0, Math.min(R.line.pv.length, (app.wi.lineK || 0) + delta));
    showLine();
    renderDev();
  }
  function showLine() {
    const R = app.wi.result, k = app.wi.lineK || 0;
    if (!R || !R.line || !k) { app.previewNode = null; renderBoard(); return; }
    const back = app.st.ply - R.ply, line = R.line.pv.slice(0, k);
    try {
      const pos = app.eng.call('position', JSON.stringify({ back, line }));
      const prev = app.eng.call('position', JSON.stringify({ back, line: line.slice(0, -1) }));
      const m = prev.legal.find((x) => x.move === line[k - 1]);
      const tone = prev.turn === R.alt.side ? 'turq' : 'coral';
      app.previewNode = {
        key: 'line:' + k, board: pos.board, marks: m ? moveMarks(m, 'last') : [],
        arrows: m ? [{ path: m.path, tone, width: 12, opacity: 0.85 }] : [],
        title: `Line after ${R.alt.best} · move ${k} of ${R.line.pv.length}: ${line[k - 1]}`,
      };
    } catch (e) { app.previewNode = null; }
    renderBoard();
  }

  // What switching each part off cost V10 over 400 games (results/ablation/ablation.json).
  const ABL = window.ABLATION || { versions: [], pairings: [] };
  const ABL_ID = { q: 'V10-q', r: 'V10-r', o: 'V10-o', t: 'V9', w: 'V10-w' };
  function ablBadge(key) {
    const v = (ABL.versions || []).find((x) => x.id === ABL_ID[key]);
    if (!v) return '';
    const p = (ABL.pairings || []).find((x) => x.b === v.id);
    const sig = v.ci[1] < 0 || v.ci[0] > 0;
    const title = `V10 against V10 without ${v.removed}` + (p ? `: ${p.games} games, ${p.w} wins, ${p.d} draws, ${p.l} losses` : '')
      + `. 95% interval [${Math.round(v.ci[0])}, ${Math.round(v.ci[1])}] Elo. Source: results/ablation/ablation.json`;
    return `<span class="badge${sig ? ' coral' : ''}" title="${esc(title)}">${fmtElo(v.elo)} Elo${sig ? '' : ' (not significant)'}</span>`;
  }
  function verdict(R) {
    const a = R.pair && R.pair.root.find((x) => x.move === R.alt.best);
    const f = R.pair && R.pair.root.find((x) => x.move === R.full.best);
    const k = app.wi.lineK || 0;
    const punish = R.line && R.line.pv && R.line.pv.length > 1
      ? ` The line it expects after it:<div class="lineplay"><button class="btn" id="lpPrev" aria-label="Previous move of the line"${k ? '' : ' disabled'}>◀</button><button class="btn" id="lpNext" aria-label="Next move of the line"${k < R.line.pv.length ? '' : ' disabled'}>▶</button><span class="lp-moves">${R.line.pv.map((m, i) => `<span class="${i < k ? 'done' : ''}${i === k - 1 ? ' cur' : ''}">${esc(m)}</span>`).join(' ')}</span></div>`
      : '';
    if (!a || !f || a.value === null || f.value === null) return `<div class="verdict bad">The settings above would play <b class="mono">${esc(R.alt.best)}</b> instead.${punish}</div>`;
    const depth = `searching only these two moves, to depth ${R.pair.depth}`;
    if (f.value > a.value + 1e-9) {
      const atMost = a.bound === 'upper';
      return `<div class="verdict bad">The settings above would play <b class="mono">${esc(R.alt.best)}</b>. The full engine, ${depth}, scores it <b class="mono">${boundSym(a.bound)}${fmtV(a.value)}</b> against <b class="mono">${fmtV(f.value)}</b> for its own choice: ${atMost ? 'at least ' : ''}<b class="mono">${fmtD(f.value - a.value)}</b> worse by its judgement.${punish}</div>`;
    }
    return `<div class="verdict same">The settings above would play <b class="mono">${esc(R.alt.best)}</b>. The full engine, ${depth}, rates it <b class="mono">${fmtV(a.value)}</b> against <b class="mono">${fmtV(f.value)}</b> for its own choice: the choice changes, the value hardly does.</div>`;
  }

  function showError(e) {
    console.error(e);
    const el = $('status');
    el.innerHTML = `<span class="badge coral">Error</span><span>${esc(e.message || e)}</span>`;
  }

  // ---------- wiring ----------
  async function main() {
    app.board = new window.BoardView($('boardWrap'));
    app.board.onSquare = onSquare;
    try { app.eng = await createEngine(); }
    catch (e) { $('engineChip').textContent = 'No engine'; showError(e); return; }
    const chip = $('engineChip');
    chip.textContent = `Rust engine · ${app.eng.info.engine || 'unknown'}`;
    chip.title = 'Our Rust engine compiled to WebAssembly, running in this page. Every move, value and tree on this page comes from it. The version names the engine source it was built from.';
    fillSelect($('whiteSel'), 'white');
    fillSelect($('blackSel'), 'black');
    try { app.eng.call('analyze', 'idqrot:t5:w=texel1', '{}'); app.timeOk = true; } catch (e) { app.timeOk = false; }
    fillBudget();
    fillScenes();
    $('newBtn').addEventListener('click', newGame);
    $('undoBtn').addEventListener('click', undo);
    $('autoBtn').addEventListener('click', () => { app.auto = !app.auto; renderControls(); if (app.auto) kick(); });
    $('stepBtn').addEventListener('click', () => { if (!app.busy && isAgent(app.st.turn)) agentMove(app.st.turn); });
    $('speedSel').addEventListener('change', (e) => { app.speed = +e.target.value; });
    $('flipBtn').addEventListener('click', () => { app.board.setFlip(!app.board.flip); renderBoard(); });
    $('modeUser').addEventListener('click', () => { app.mode = 'user'; app.previewNode = null; render(); });
    $('modeDev').addEventListener('click', () => { app.mode = 'dev'; render(); });
    for (const b of document.querySelectorAll('.tabs button')) b.addEventListener('click', () => { app.tab = b.dataset.tab; app.previewNode = null; app.wi.lineK = 0; render(); });
    document.addEventListener('keydown', (e) => {
      if (e.target.closest('select, input, textarea')) return;
      if (e.key === ' ') { e.preventDefault(); $('autoBtn').click(); }
      else if (e.key === 'ArrowRight') { e.preventDefault(); $('stepBtn').click(); }
      else if (e.key === 'd' || e.key === 'D') { app.mode = app.mode === 'user' ? 'dev' : 'user'; app.previewNode = null; render(); }
      else if (e.key === 'u' || e.key === 'U') undo();
      else if (e.key === 'f' || e.key === 'F') $('flipBtn').click();
      else if (e.key === 'n' || e.key === 'N') newGame();
      else if (e.key === 'Escape') { app.sel = null; app.routes = null; app.previewNode = null; app.wi.lineK = 0; renderRoutes(); render(); }
      else if (/^[1-9]$/.test(e.key)) loadScene(SCENES.find((x) => x.key === e.key));
    });
    newGame();
  }
  window.DamaApp = app;
  main();
})();
