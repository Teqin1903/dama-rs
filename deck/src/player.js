// ---------- player: scaling, navigation, transitions, notes ----------
(() => {
  let S = 1, DPR = 1;
  const st = { scene: 0, beat: 0, t0: performance.now(), final: false, auto: false };
  const snap = document.createElement('canvas'); const sctx = snap.getContext('2d');
  let trans = null;
  const mq = window.matchMedia('(prefers-reduced-motion: reduce)');
  let reduced = mq.matches;
  const notesEl = document.getElementById('notes');
  const hintEl = document.getElementById('hint');
  const barEl = document.getElementById('bar');
  let clockStart = null;

  function resize() {
    const vw = window.innerWidth, vh = window.innerHeight;
    S = Math.min(vw / W, vh / H); DPR = window.devicePixelRatio || 1;
    cv.width = Math.round(W * S * DPR); cv.height = Math.round(H * S * DPR);
    cv.style.width = `${W * S}px`; cv.style.height = `${H * S}px`;
    cv.style.left = `${(vw - W * S) / 2}px`; cv.style.top = `${(vh - H * S) / 2}px`;
    snap.width = cv.width; snap.height = cv.height;
  }
  window.addEventListener('resize', resize);

  const totalBeats = () => SCENES.reduce((a, s) => a + s.beats.length, 0);
  const beatIndex = () => SCENES.slice(0, st.scene).reduce((a, s) => a + s.beats.length, 0) + st.beat;

  function go(scene, beat, dir) {
    if (scene !== st.scene) {
      sctx.setTransform(1, 0, 0, 1, 0, 0); sctx.clearRect(0, 0, snap.width, snap.height); sctx.drawImage(cv, 0, 0);
      trans = { start: performance.now() };
    }
    st.scene = scene; st.beat = beat; st.t0 = performance.now(); st.final = dir < 0;
    if (dir > 0 && clockStart === null && (scene > 0 || beat > 0)) clockStart = performance.now();
    try { history.replaceState(null, '', `#${scene + 1}.${beat}`); } catch (e) { /* file:// may refuse */ }
    ui();
  }
  function next() {
    const sc = SCENES[st.scene];
    if (st.beat < sc.beats.length - 1) go(st.scene, st.beat + 1, 1);
    else if (st.scene < SCENES.length - 1) go(st.scene + 1, 0, 1);
  }
  function prev() {
    if (st.beat > 0) go(st.scene, st.beat - 1, -1);
    else if (st.scene > 0) go(st.scene - 1, SCENES[st.scene - 1].beats.length - 1, -1);
  }
  function replay() { st.t0 = performance.now(); st.final = false; }

  function ui() {
    barEl.style.width = `${(100 * (beatIndex() + 1)) / totalBeats()}%`;
    const sc = SCENES[st.scene];
    if (notesEl.classList.contains('on')) {
      const main = Math.min(SCENES.length, SLIDE_COUNT);
      const until = SCENES.slice(0, Math.min(st.scene + 1, main)).reduce((acc, x) => acc + x.beats.reduce((u, v) => u + v, 0), 0);
      const mmss = (x) => `${Math.floor(x / 60)}:${String(Math.round(x % 60)).padStart(2, '0')}`;
      notesEl.innerHTML = `<div class="top"><span>Slide ${st.scene + 1} of ${SCENES.length}, step ${st.beat + 1} of ${sc.beats.length}. Aim to finish this slide by ${mmss(until)}. Keys: → next, ← back, R replay, A autoplay, F full screen, N notes</span><span class="clock" id="clk">0:00</span></div>` +
        sc.notes.map((n, i) => `<p class="${i === st.beat ? 'now' : ''}"><b>${i + 1}</b>${n} <span class="secs">~${Math.round(sc.beats[i])} s</span></p>`).join('');
      tickClock();
    }
  }
  function tickClock() {
    const el = document.getElementById('clk'); if (!el) return;
    const s = clockStart === null ? 0 : Math.floor((performance.now() - clockStart) / 1000);
    el.textContent = `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
  }
  setInterval(tickClock, 500);

  function printScript() {
    const titles = ['Our agent on one page', 'Since Assignment 3: ten versions on one scale', 'Ablation: switch one part off, then the demo', 'Why alpha–beta, and not Monte Carlo tree search?', 'Learning the weights from our own games', 'After the ladder: the 50-ply rule, found and fixed', 'What we left out of our C++ agent, and why', 'Next steps'];
    let cum = 0; const mmss = (x) => `${Math.floor(x / 60)}:${String(Math.round(x % 60)).padStart(2, '0')}`;
    const rows = SCENES.slice(0, SLIDE_COUNT).map((sc, i) => {
      const items = sc.notes.map((n, j) => { cum += sc.beats[j]; return `<li>${n} <small>(${Math.round(sc.beats[j])} s, by ${mmss(cum)})</small></li>`; }).join('');
      return `<h2>${i + 1}. ${titles[i]}</h2><ol>${items}</ol>`;
    }).join('');
    const w = window.open('', '_blank');
    if (!w) return;
    w.document.write(`<!doctype html><meta charset="utf-8"><title>Speaker script</title><style>body{font:16px/1.5 system-ui,sans-serif;max-width:760px;margin:32px auto;padding:0 16px;color:#18223D}h1{font-size:24px}h2{font-size:18px;margin:22px 0 6px}li{margin:4px 0}small{color:#7E879B}</style><h1>Speaker script: Turkish draughts, search measured (Team Eight Bits, pair 42)</h1><p>One numbered line = one press of the right arrow. Target: about 9 and a half minutes with the demo, leaving 1 and a half for questions (11 minutes in all).</p>${rows}`);
    w.document.close();
  }
  function hideHint() { hintEl.style.opacity = '0'; }
  setTimeout(hideHint, 6000);

  window.addEventListener('keydown', (e) => {
    if (e.ctrlKey || e.metaKey || e.altKey) return;
    const k = e.key;
    if (['ArrowRight', 'ArrowDown', 'PageDown', ' ', 'Enter'].includes(k)) { e.preventDefault(); next(); }
    else if (['ArrowLeft', 'ArrowUp', 'PageUp', 'Backspace'].includes(k)) { e.preventDefault(); prev(); }
    else if (k === 'Home') go(0, 0, 1);
    else if (k === 'End') go(SCENES.length - 1, 0, 1);
    else if (k === 'f' || k === 'F') { if (!document.fullscreenElement) document.documentElement.requestFullscreen?.(); else document.exitFullscreen?.(); }
    else if (k === 'n' || k === 'N') { notesEl.classList.toggle('on'); ui(); }
    else if (k === 'r' || k === 'R') replay();
    else if (k === 'a' || k === 'A') st.auto = !st.auto;
    else if (k === 'm' || k === 'M') { reduced = !reduced; replay(); }
    else if (k === 't' || k === 'T') clockStart = performance.now();
    else if (k === 'p' || k === 'P') printScript();
    else if (/^[1-9]$/.test(k) && +k <= SCENES.length) go(+k - 1, 0, 1);
    hideHint();
  });
  cv.addEventListener('click', (e) => {
    const r = cv.getBoundingClientRect();
    if (e.clientX - r.left < r.width * 0.2) prev(); else next();
    hideHint();
  });

  // debug/screenshot hook: window.__freeze = {scene, beat, t}
  function frame(now) {
    let sc = SCENES[st.scene], beat = st.beat;
    let t = (now - st.t0) / 1000;
    if (st.final || reduced) t = 1e6;
    if (window.__freeze) { sc = SCENES[window.__freeze.scene]; beat = window.__freeze.beat; t = window.__freeze.t; }
    ctx.setTransform(S * DPR, 0, 0, S * DPR, 0, 0);
    ctx.globalAlpha = 1;
    ctx.fillStyle = C.ground; ctx.fillRect(0, 0, W, H);
    try { sc.render(beat, t); } catch (err) { console.error(err); }
    if (reduced && st.scene === 0 && !window.__freeze) T('Reduced motion is on, so animations are skipped. Press M to play them.', 120, 1000, { size: 18, c: C.ink3 });
    if (trans && !window.__freeze) {
      const p = (now - trans.start) / 420;
      if (p >= 1) trans = null;
      else { ctx.setTransform(1, 0, 0, 1, 0, 0); ctx.globalAlpha = 1 - E.io(p); ctx.drawImage(snap, 0, 0); ctx.globalAlpha = 1; }
    }
    if (st.auto && !window.__freeze && t > sc.beats[beat]) next();
    requestAnimationFrame(frame);
  }

  async function start() {
    resize();
    const faces = ['400 40px "Young Serif"', '400 20px "Instrument Sans"', '500 20px "Instrument Sans"', '600 20px "Instrument Sans"', '400 20px "STIX Two Text"', 'italic 400 20px "STIX Two Text"', '400 20px "JetBrains Mono"', '600 20px "JetBrains Mono"'];
    try { await Promise.all(faces.map((f) => document.fonts.load(f, 'AaÜÇış0123'))); } catch (e) { /* fall back to system fonts */ }
    try { if (LOGO.src && LOGO.decode) await LOGO.decode(); } catch (e) { /* the title slide simply shows no logo */ }
    for (const s of SCENES) if (s.init) s.init();
    const m = /^#(\d+)\.(\d+)$/.exec(location.hash);
    if (m) { const sc = Math.min(SCENES.length - 1, +m[1] - 1); st.scene = sc; st.beat = Math.min(SCENES[sc].beats.length - 1, +m[2]); st.final = true; }
    ui();
    cv.focus();
    window.__ready = true;
    requestAnimationFrame(frame);
  }
  start();
})();
