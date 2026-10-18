<script lang="ts">
  // Telemetry column (UI spec 4.8): oscilloscope, spectrum, orbit (channel radar), master L/R meters.
  // Drawn into a low-res buffer, bit-crushed and blown up with a phosphor glow (mockup present()).
  // The orbit radar follows pad-hit activity; scope, spectrum and meters stay in STANDBY until the
  // engine streams master levels to the UI.
  import { onMount } from 'svelte';
  import { energy, isResting, onActivity } from '../lib/activity.ts';
  import { GROUP_COLORS } from '../lib/model.ts';
  import { ledToCss } from '../lib/led.ts';

  let { mini, onfold }: { mini: boolean; onfold: (folded: boolean) => void } = $props();

  type Lo = HTMLCanvasElement & { _labels?: [string, number, number, string][] };
  const canvases: Record<string, HTMLCanvasElement | undefined> = $state({});
  let clock = $state('T+00:00.0');
  const PIX = 2, NB = 28, STEPS = 16;
  const groupCol = (i: number) => ledToCss(GROUP_COLORS[i]);
  const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

  const F3 = (() => {
    const m: Record<string, number[]> = {};
    const d: Record<string, string> = {
      '0': '111101101101111', '1': '010110010010111', '2': '111001111100111', '3': '111001111001111', '4': '101101111001001',
      '5': '111100111001111', '6': '111100111101111', '7': '111001010010010', '8': '111101111101111', '9': '111101111001111',
      A: '010101111101101', B: '110101110101110', C: '011100100100011', D: '110101101101110', E: '111100110100111',
      F: '111100110100100', G: '011100101101011', H: '101101111101101', I: '111010010010111', J: '001001001101010',
      K: '101101110101101', L: '100100100100111', M: '101111111101101', N: '110101101101101', O: '010101101101010',
      P: '110101110100100', Q: '010101101111011', R: '110101110101101', S: '011100010001110', T: '111010010010010',
      U: '101101101101111', V: '101101101101010', W: '101101111111101', X: '101101010101101', Y: '101101010010010',
      Z: '111001010100111', ' ': '000000000000000', '-': '000000111000000', '.': '000000000000010', '·': '000000010000000',
      '/': '001001010100100', ':': '000010000010000', '+': '000010111010000', '%': '101001010100101',
    };
    for (const [k, v] of Object.entries(d)) m[k] = [0, 1, 2, 3, 4].map((r) => parseInt(v.substr(r * 3, 3), 2));
    return m;
  })();

  // 3×5 bitmap labels, queued and stamped after the crush so they stay legible.
  function txt(g: CanvasRenderingContext2D, s: string, x: number, y: number, col: string, al: 'left' | 'center' | 'right' = 'left') {
    s = s.toUpperCase().replace(/−/g, '-');
    const n = s.length * 4 - 1;
    const x0 = Math.round(x / PIX) - (al === 'center' ? Math.floor(n / 2) : al === 'right' ? n : 0), y0 = Math.round(y / PIX) - 5;
    const lo = g.canvas as Lo;
    (lo._labels ??= []).push([s, x0, y0, col]);
  }
  function stamp(g: CanvasRenderingContext2D, [s, x0, y0, col]: [string, number, number, string]) {
    g.fillStyle = col;
    [...s].forEach((ch, i) => {
      const gl = F3[ch];
      if (gl) for (let r = 0; r < 5; r++) for (let b = 0; b < 3; b++) if ((gl[r] >> (2 - b)) & 1) g.fillRect(x0 + i * 4 + b, y0 + r, 1, 1);
    });
  }
  const los = new WeakMap<HTMLCanvasElement, Lo>();
  function fit(c: HTMLCanvasElement | undefined): [CanvasRenderingContext2D, number, number] | null {
    if (!c) return null;
    const d = devicePixelRatio || 1, w = c.clientWidth, h = c.clientHeight;
    if (!w || !h) return null;
    if (c.width !== Math.round(w * d) || c.height !== Math.round(h * d)) { c.width = Math.round(w * d); c.height = Math.round(h * d); }
    let lo = los.get(c);
    if (!lo) { lo = document.createElement('canvas') as Lo; los.set(c, lo); }
    const lw = Math.ceil(w / PIX), lh = Math.ceil(h / PIX);
    if (lo.width !== lw || lo.height !== lh) { lo.width = lw; lo.height = lh; }
    const g = lo.getContext('2d', { willReadFrequently: true })!;
    g.setTransform(1, 0, 0, 1, 0, 0); g.clearRect(0, 0, lw, lh); g.setTransform(1 / PIX, 0, 0, 1 / PIX, 0, 0);
    return [g, w, h];
  }
  function present(c: HTMLCanvasElement | undefined) {
    const lo = c && los.get(c);
    if (!c || !lo || !c.width) return;
    const g = lo.getContext('2d', { willReadFrequently: true })!, im = g.getImageData(0, 0, lo.width, lo.height), a = im.data, W = lo.width;
    for (let i = 0; i < a.length; i += 4) {
      const al = a[i + 3];
      if (al < 20) { a[i + 3] = 0; continue; }
      const k = (((i >> 2) / W) | 0) & 1 ? 0.55 : 1;
      a[i] = Math.round(a[i] / 85) * 85 * k; a[i + 1] = Math.round(a[i + 1] / 85) * 85 * k; a[i + 2] = Math.round(a[i + 2] / 85) * 85 * k;
      a[i + 3] = al < 90 ? 110 : al < 180 ? 190 : 255;
    }
    g.putImageData(im, 0, 0);
    g.setTransform(1, 0, 0, 1, 0, 0);
    (lo._labels ?? []).forEach((l) => stamp(g, l));
    lo._labels = [];
    const o = c.getContext('2d')!, d = devicePixelRatio || 1, dw = lo.width * PIX * d, dh = lo.height * PIX * d;
    o.setTransform(1, 0, 0, 1, 0, 0); o.clearRect(0, 0, c.width, c.height); o.imageSmoothingEnabled = false;
    o.filter = `blur(${(2.2 * d).toFixed(1)}px)`; o.globalAlpha = 0.75; o.drawImage(lo, 0, 0, dw, dh);
    o.filter = 'none'; o.globalAlpha = 1; o.drawImage(lo, 0, 0, dw, dh);
  }

  function scopeGrid(g: CanvasRenderingContext2D, W: number, H: number, nx: number, ny: number) {
    g.lineWidth = 2; g.strokeStyle = 'rgba(65,200,255,.55)'; g.strokeRect(1, 1, W - 2, H - 2);
    g.strokeStyle = 'rgba(65,200,255,.32)'; g.beginPath();
    for (let i = 1; i < nx; i++) { const x = Math.round((i * W) / nx / PIX) * PIX; g.moveTo(x, 0); g.lineTo(x, H); }
    g.stroke(); g.setLineDash([4, 4]); g.beginPath();
    for (let j = 1; j < ny; j++) { const y = Math.round((j * H) / ny / PIX) * PIX; g.moveTo(0, y); g.lineTo(W, y); }
    g.stroke(); g.setLineDash([]);
  }
  function drawScope(g: CanvasRenderingContext2D, W: number, H: number) {
    scopeGrid(g, W, H, 8, 6);
    const cx = Math.round(W / 2 / PIX) * PIX, cy = Math.round(H / 2 / PIX) * PIX;
    g.lineWidth = 2; g.strokeStyle = '#ff3d4f'; g.beginPath(); g.moveTo(cx, 0); g.lineTo(cx, H); g.stroke();
    g.strokeStyle = '#e9fff1'; g.beginPath(); g.moveTo(0, cy); g.lineTo(W, cy); g.stroke();
    txt(g, 'CH1 · MASTER', 6, 14, '#ffb21a');
    txt(g, 'STANDBY · NO LEVEL FEED', W / 2, H * 0.3, '#5cff9d', 'center');
  }
  function drawSpec(g: CanvasRenderingContext2D, W: number, H: number) {
    const lo = 40, hi = 16000, bot = H - 14, sh = 4, sg = 2, nSeg = Math.max(4, Math.floor((bot - 4) / (sh + sg))), bw = (W - 8) / NB;
    for (let b = 0; b < NB; b++) for (let q = 0; q < nSeg; q++) {
      g.fillStyle = 'rgba(65,200,255,.07)'; g.fillRect(4 + b * bw, bot - (q + 1) * (sh + sg), bw - 2, sh);
    }
    for (const [f, l] of [[100, '100'], [1000, '1k'], [10000, '10k']] as const) txt(g, l, 4 + (Math.log(f / lo) / Math.log(hi / lo)) * (W - 8), H - 2, '#7b8796', 'center');
  }
  function drawOrbit(g: CanvasRenderingContext2D, W: number, H: number) {
    const cx = W / 2, cy = H / 2, R = Math.min(W, H) / 2 - 7, r0 = R * 0.64, TAU = Math.PI * 2, ang = (i: number) => -Math.PI / 2 + (i * TAU) / 8;
    g.lineWidth = 1; g.setLineDash([2, 3]); g.strokeStyle = 'rgba(65,200,255,.22)';
    for (const f of [1 / 3, 2 / 3, 1]) { g.beginPath(); g.arc(cx, cy, r0 * f, 0, TAU); g.stroke(); }
    g.setLineDash([]); g.strokeStyle = 'rgba(65,200,255,.16)'; g.beginPath();
    for (let i = 0; i < 8; i++) { g.moveTo(cx, cy); g.lineTo(cx + Math.cos(ang(i)) * r0, cy + Math.sin(ang(i)) * r0); }
    g.stroke();
    const pt = (i: number, rr: number) => [cx + Math.cos(ang(i)) * rr, cy + Math.sin(ang(i)) * rr], rad = (i: number) => r0 * (0.06 + 0.94 * energy[i]);
    g.beginPath();
    for (let i = 0; i < 8; i++) { const [x, y] = pt(i, rad(i)); if (i) g.lineTo(x, y); else g.moveTo(x, y); }
    g.closePath(); g.fillStyle = 'rgba(255,91,31,.16)'; g.fill(); g.strokeStyle = '#ff5b1f'; g.lineWidth = 1.4; g.stroke(); g.lineWidth = 1;
    for (let i = 0; i < 8; i++) {
      const [x, y] = pt(i, rad(i)), c = groupCol(i), hot = energy[i] > 0.05;
      g.fillStyle = c; g.beginPath(); g.arc(x, y, 2 + energy[i] * 2, 0, TAU); g.fill();
      const [lx, ly] = pt(i, r0 + 9);
      txt(g, String(i + 1), lx, ly + 3, hot ? '#ece3cc' : '#7b8796', 'center');
    }
    g.strokeStyle = 'rgba(65,200,255,.35)'; g.beginPath(); g.arc(cx, cy, R, 0, TAU); g.stroke();
    for (let q = 0; q < STEPS; q++) {
      const L = q % 4 === 0 ? 6 : 3, a = -Math.PI / 2 + (q / STEPS) * TAU;
      g.strokeStyle = 'rgba(65,200,255,.4)'; g.beginPath();
      g.moveTo(cx + Math.cos(a) * (R - L), cy + Math.sin(a) * (R - L)); g.lineTo(cx + Math.cos(a) * R, cy + Math.sin(a) * R); g.stroke();
    }
    g.fillStyle = '#7b8796'; g.beginPath(); g.arc(cx, cy - R, 3, 0, TAU); g.fill();
  }
  function drawVu(g: CanvasRenderingContext2D, W: number, H: number) {
    const N = 36, x0 = 12, w = W - x0 - 2, sw = w / N, rh = Math.floor((H - 12) / 2) - 1, pos = (d: number) => Math.min(1, Math.max(0, (d + 48) / 51));
    [0, 1].forEach((ch) => {
      const y = 1 + ch * (rh + 2);
      txt(g, ch ? 'R' : 'L', 2, y + rh - 1, '#f0a23a');
      for (let q = 0; q < N; q++) { g.fillStyle = 'rgba(65,200,255,.07)'; g.fillRect(x0 + q * sw, y, sw - 2, rh); }
    });
    for (const d of [-40, -24, -12, -6, 0]) txt(g, String(d), x0 + pos(d) * w, H - 1, '#7b8796', 'center');
  }
  function drawMini(g: CanvasRenderingContext2D, W: number, H: number) {
    const N = Math.max(8, Math.floor(H / 5)), cw = (W - 6) / 2;
    [0, 1].forEach((ch) => { for (let q = 0; q < N; q++) { g.fillStyle = 'rgba(65,200,255,.07)'; g.fillRect(2 + ch * (cw + 2), H - (q + 1) * (H / N), cw, H / N - 2); } });
  }

  // Nothing here runs on a timer of its own. A panel is drawn when it appears, when its size
  // changes and, for the orbit, while pad activity is decaying. The standby panels (scope, spectrum,
  // meters) are static until the engine streams levels; a live feed will redraw them at the feed
  // rate, not at the display's refresh rate.
  const painters: Record<string, (g: CanvasRenderingContext2D, W: number, H: number) => void> = {
    scope: drawScope, spec: drawSpec, orbit: drawOrbit, vu: drawVu, mini: drawMini,
  };
  const pending = new Set<string>();
  let flushRaf = 0;
  function redraw(id: string) {
    const c = canvases[id];
    const r = fit(c);
    if (r) { painters[id](...r); present(c); }
  }
  function flush() {
    flushRaf = 0;
    const ids = [...pending];
    pending.clear();
    ids.forEach(redraw);
  }
  function schedule(id: string) {
    pending.add(id);
    if (!flushRaf) flushRaf = requestAnimationFrame(flush);
  }
  /** Draws the canvas once it has a size, and again whenever its size changes. */
  function watch(node: HTMLCanvasElement, id: string) {
    const observer = new ResizeObserver(() => schedule(id));
    observer.observe(node);
    return { destroy() { observer.disconnect(); pending.delete(id); } };
  }

  let orbitAt = 0;
  function orbitTick() {
    if (mini) return;
    const now = performance.now();
    // While decaying: at most ~30 fps (4 fps with reduced motion); the final, resting frame always draws.
    if (isResting() || now - orbitAt >= (reduced() ? 250 : 33)) { orbitAt = now; schedule('orbit'); }
  }

  onMount(() => {
    const t0 = performance.now();
    const tick = () => {
      if (mini || document.hidden) return;
      const sec = (performance.now() - t0) / 1000;
      clock = `T+${String(Math.floor(sec / 60)).padStart(2, '0')}:${(sec % 60).toFixed(1).padStart(4, '0')}`;
    };
    const timer = setInterval(tick, reduced() ? 1000 : 100);
    const stopActivity = onActivity(orbitTick);
    return () => { clearInterval(timer); stopActivity(); cancelAnimationFrame(flushRaf); };
  });
</script>

{#if mini}
  <button type="button" class="tele plate mini" data-label="SIG" title="Telemetry folded: click to open" onclick={() => onfold(false)}>
    <div class="tm-g screen"><canvas bind:this={canvases.mini} use:watch={'mini'}></canvas></div>
  </button>
{:else}
  <aside class="tele plate" data-label="TELEMETRY" aria-hidden="true">
    <div class="tele-head">
      <span class="pill">SIGNAL</span>
      <span class="lcd">{clock}</span>
      <span class="beats"><i></i><i></i><i></i><i></i></span>
      <button type="button" class="key small" title="Fold telemetry" tabindex="-1" onclick={() => onfold(true)}>▸</button>
    </div>
    {#each [['scope', 'OSCILLOSCOPE', '— ms/div'], ['spec', 'SPECTRUM', '—'], ['orbit', 'ORBIT', 'IDLE · NO SEQ'], ['vu', 'MASTER L / R', 'PK −∞']] as [id, label, read]}
      <section class="tm {id}">
        <div class="tm-h"><span class="silk">{label}</span><span class="lcd">{read}</span></div>
        <div class="tm-g screen"><canvas bind:this={canvases[id]} use:watch={id}></canvas></div>
      </section>
    {/each}
  </aside>
{/if}

<style>
  .tele { flex: none; width: var(--tw, 240px); display: flex; flex-direction: column; gap: 8px; padding: 12px 10px 10px; min-height: 0; }
  .tele.mini { padding: 12px 5px 6px; cursor: pointer; border: 0; color: inherit; }
  .tele.mini > .tm-g { flex: 1; width: 100%; }
  .tele-head { flex: none; display: flex; align-items: center; gap: 8px; height: 18px; }
  .pill { height: 16px; padding: 0 10px 0 12px; border-radius: 8px 2px 2px 8px; background: var(--orange); color: #160903; font: 700 10px/16px var(--f-silk); letter-spacing: .16em; }
  .tele-head .key { margin-left: auto; width: 20px; height: 18px; min-width: 0; padding: 0; }
  .beats { display: flex; gap: 3px; }
  .beats i { width: 8px; height: 8px; background: var(--led-off); }
  .tm { display: flex; flex-direction: column; gap: 3px; min-height: 0; flex: 1 1 0; }
  .tm.scope { flex-grow: 1.3; }
  .tm.orbit { flex-grow: 1.4; }
  .tm.vu { flex: 0 0 66px; }
  .tm-h { flex: none; display: flex; align-items: baseline; gap: 6px; white-space: nowrap; }
  .tm-h .lcd { margin-left: auto; }
  .tm-h .silk, .tm-h .lcd, .tele-head .lcd { font: 400 15px/1 'VT323', var(--f-lcd); letter-spacing: .04em; }
  .tm-h .silk { color: var(--warn); text-shadow: 0 0 6px rgba(255,178,26,.55); }
  .tm-h .lcd, .tele-head .lcd { color: #7dffb0; text-shadow: 0 0 6px rgba(125,255,176,.5); font-size: 13px; }
  .tm-g { position: relative; flex: 1; min-height: 0; }
  .tm-g canvas { position: absolute; inset: 0; width: 100%; height: 100%; }
</style>
