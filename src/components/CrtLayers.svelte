<script lang="ts">
  import { onMount } from 'svelte';
  import type { CrtLevel } from '../lib/display.ts';
  let { level, bent = false }: { level: CrtLevel; bent?: boolean } = $props();
  let mask: HTMLCanvasElement;
  let scan: HTMLDivElement;
  let reduced = $state(true);

  // Every trigger (level change, ResizeObserver, window resize) asks the one scheduler; it draws at most
  // once per frame and skips a draw whose size, pixel ratio and level are the ones already on the canvas.
  let drawn = '';
  let frame = 0;
  let painted = false;
  function requestDraw() {
    if (frame || !mask) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      // The first mask waits one more frame, so the startup view paints before the dense canvas is built.
      if (!painted) { painted = true; requestDraw(); return; }
      drawMask();
    });
  }
  function drawMask() {
    if (!mask) return;
    if (level === 'off') { drawn = ''; return; }
    const ratio = window.devicePixelRatio || 1;
    const rect = mask.getBoundingClientRect();
    const width = Math.round(rect.width * ratio), height = Math.round(rect.height * ratio);
    const key = `${width}x${height}@${ratio}:${level}`;
    if (key === drawn) return;
    drawn = key;
    mask.width = width;
    mask.height = height;
    const ctx = mask.getContext('2d');
    if (!ctx) return;
    const w = mask.width, h = mask.height;
    const cx = w / 2, cy = h / 2, diagonal = Math.hypot(cx, cy);
    const k = level === 'medium' ? .10 : .07;
    // Draw at physical resolution: every cell is exactly three device pixels.
    const bend = (x: number, y: number) => {
      const dx = x - cx, dy = y - cy;
      const r2 = (dx * dx + dy * dy) / (diagonal * diagonal);
      return [cx + dx * (1 - 2 * k * r2), cy + dy * (1 - 2 * k * r2)];
    };
    ctx.fillStyle = '#fff'; ctx.fillRect(0, 0, w, h); ctx.strokeStyle = '#000';
    ctx.lineWidth = 3 * .34; ctx.beginPath();
    for (let y = -h * .25; y < h * 1.25; y += 3) {
      let first = true;
      for (let x = -w * .25; x <= w * 1.25; x += 18) {
        const [bx, by] = bend(x, y);
        if (first) ctx.moveTo(bx, by); else ctx.lineTo(bx, by);
        first = false;
      }
    }
    ctx.stroke(); ctx.lineWidth = 3 * .07; ctx.beginPath();
    for (let x = -w * .25; x < w * 1.25; x += 3) {
      let first = true;
      for (let y = -h * .25; y <= h * 1.25; y += 18) {
        const [bx, by] = bend(x, y);
        if (first) ctx.moveTo(bx, by); else ctx.lineTo(bx, by);
        first = false;
      }
    }
    ctx.stroke();
  }

  // BEND (mockup 557–558, 1921–1923): a radial displacement map, 4.5 px at the middle of the edges and
  // 9 px in the corners (R/G = 0.5 + 0.25·u·r²), sized to the console; SVG filter #crt-bend.
  let bendMap = $state('');
  let bendW = $state(1280), bendH = $state(720);
  function makeBendMap() {
    const N = 512, canvas = document.createElement('canvas');
    canvas.width = canvas.height = N;
    const ctx = canvas.getContext('2d');
    if (!ctx) return '';
    const image = ctx.createImageData(N, N);
    for (let y = 0; y < N; y++) for (let x = 0; x < N; x++) {
      const u = (x / (N - 1)) * 2 - 1, v = (y / (N - 1)) * 2 - 1, r2 = u * u + v * v, i = (y * N + x) * 4;
      image.data[i] = Math.round(255 * (0.5 + 0.25 * u * r2));
      image.data[i + 1] = Math.round(255 * (0.5 + 0.25 * v * r2));
      image.data[i + 2] = 128; image.data[i + 3] = 255;
    }
    ctx.putImageData(image, 0, 0);
    return canvas.toDataURL();
  }
  function sizeBend() {
    const console = document.querySelector<HTMLElement>('.console');
    if (console) { bendW = console.offsetWidth; bendH = console.offsetHeight; }
  }
  $effect(() => { if (bent) { if (!bendMap) bendMap = makeBendMap(); sizeBend(); } });

  onMount(() => {
    const query = matchMedia('(prefers-reduced-motion: reduce)');
    const motionChanged = () => { reduced = query.matches; };
    motionChanged(); query.addEventListener('change', motionChanged);
    const resize = new ResizeObserver(requestDraw);
    resize.observe(mask);
    window.addEventListener('resize', requestDraw);
    window.addEventListener('resize', sizeBend);
    return () => {
      cancelAnimationFrame(frame); frame = 0;
      resize.disconnect(); window.removeEventListener('resize', requestDraw); window.removeEventListener('resize', sizeBend);
      query.removeEventListener('change', motionChanged);
    };
  });
  $effect(() => { level; requestDraw(); });
  $effect(() => {
    if (level === 'off' || reduced || !scan) return;
    let timer: number;
    let animation: Animation | undefined;
    function schedule() { timer = window.setTimeout(sweep, 7000 + Math.random() * 11000); }
    function sweep() {
      const stall = Math.random() < .45;
      const end = scan.parentElement!.clientHeight * (stall ? .3 + Math.random() * .4 : 1) + 40;
      animation = scan.animate([
        { transform: 'translateY(0)', opacity: 0 },
        { opacity: 1, offset: .12 },
        { transform: `translateY(${end}px)`, opacity: stall ? .7 : 1, offset: stall ? .65 : .95 },
        { transform: `translateY(${end}px)`, opacity: 0 },
      ], { duration: stall ? 1400 + Math.random() * 1200 : 1800 + Math.random() * 900, easing: stall ? 'ease-out' : 'linear' });
      schedule();
    }
    schedule();
    return () => { clearTimeout(timer); animation?.cancel(); };
  });
</script>

<svg width="0" height="0" style="position:absolute" aria-hidden="true" focusable="false">
  <filter id="crt-bend" x="0" y="0" width="100%" height="100%" color-interpolation-filters="sRGB">
    <feImage href={bendMap} x="0" y="0" width={bendW} height={bendH} preserveAspectRatio="none" result="map" />
    <feDisplacementMap in="SourceGraphic" in2="map" scale="18" xChannelSelector="R" yChannelSelector="G" result="d" />
    <feGaussianBlur in="d" stdDeviation=".4" />
  </filter>
</svg>
<div class="crt-bloom" aria-hidden="true"></div>
<canvas bind:this={mask} class="crt-mask" aria-hidden="true"></canvas>
<div class="crt-tube" aria-hidden="true">
  <div class="grain"></div>
  <div class="scan" bind:this={scan}></div>
  <div class="crt-frame"></div>
</div>

<style>
  .crt-bloom, .crt-mask, .crt-tube { position: absolute; inset: var(--frame); pointer-events: none; border-radius: 44px / 38px; }
  .crt-bloom { z-index: 63; backdrop-filter: brightness(.75) contrast(4) saturate(1.8) blur(5px); mix-blend-mode: screen; opacity: min(calc(.32 * var(--crt)), .45); }
  .crt-mask { z-index: 64; width: calc(100% - 2 * var(--frame)); height: calc(100% - 2 * var(--frame)); mix-blend-mode: multiply; opacity: min(calc(.18 * var(--crt)), .26); }
  .crt-tube { z-index: 65; overflow: hidden; background: radial-gradient(ellipse 75% 70% at 50% 50%, transparent 66%, rgba(0,0,0,calc(.2 * var(--crt)))); animation: flicker 6s steps(1) infinite; }
  .crt-tube::after { content: ''; position: absolute; inset: 0; background: radial-gradient(ellipse 60% 45% at 28% 18%, rgba(220,235,255,calc(.035 * var(--crt))), transparent 70%); }
  .grain { position: absolute; inset: -60px; mix-blend-mode: overlay; opacity: calc(.1 * var(--crt)); animation: grain 1s steps(10) infinite; background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='240' height='240'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='.8' numOctaves='3' stitchTiles='stitch'/%3E%3CfeColorMatrix type='saturate' values='0'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='.7'/%3E%3C/svg%3E"); }
  .scan { position: absolute; left: 0; right: 0; top: -40px; height: 40px; opacity: 0; background: linear-gradient(transparent,rgba(160,210,255,calc(.035 * var(--crt))) 70%,rgba(225,240,255,calc(.16 * var(--crt))) 96%,rgba(255,255,255,calc(.28 * var(--crt)))); }
  .crt-frame { position: absolute; inset: 0; border-radius: inherit; box-shadow: inset 0 0 calc(34px * var(--crt)) rgba(0,0,0,.45), inset 0 0 0 3px #15120f, inset 0 0 0 4px rgba(255,255,255,.22); }
  .crt-frame::after { content: ''; position: fixed; right: 64px; bottom: 3px; width: 6px; height: 6px; border-radius: 50%; background: var(--ok); box-shadow: 0 0 6px var(--ok), 0 0 0 1px rgba(0,0,0,.35); }
  :global(.instrument:not([data-crt='off'])) { box-shadow: inset 0 2px 0 rgba(255,255,255,.55), inset 0 -2px 0 rgba(0,0,0,.25), inset 0 0 22px rgba(120,100,60,.25); }
  :global(.instrument[data-crt='off']) .crt-bloom, :global(.instrument[data-crt='off']) .crt-mask, :global(.instrument[data-crt='off']) .crt-tube { display: none; }
  @keyframes grain { from { transform: translate(0,0); } 20% { transform: translate(-22px,14px); } 40% { transform: translate(18px,-26px); } 60% { transform: translate(-30px,-8px); } 80% { transform: translate(10px,30px); } }
  @keyframes flicker { 0%,100% { opacity: 1; } 31% { opacity: .93; } 32% { opacity: 1; } 74% { opacity: .96; } 75% { opacity: 1; } }
  @media (prefers-reduced-motion: reduce) { .crt-tube, .grain { animation: none; } .scan { display: none; } }
</style>
