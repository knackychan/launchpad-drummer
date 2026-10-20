<script lang="ts">
  import { onMount } from 'svelte';
  import { armBomb, createHorde, disarmBomb, drainEvents, freezeEnemies, hitCell, ledFrame, noteCell, padPress, resetHorde,
    tickHorde, togglePause, type HordeState, type Layout, type PlayMode } from '../lib/horde.ts';
  import { CODE_TO_CELL, H, W, cellAt, makeLayout } from '../lib/hordeLayout.ts';
  import { DEFAULT_SETTINGS, applyEvents, createFx, updateFx, type HordeSettings } from '../lib/hordeFx.ts';
  import { createRenderer, type HordeRenderer } from '../lib/hordeRender.ts';
  import { createHordeAudio } from '../lib/hordeAudio.ts';
  import type { Rgb } from '../lib/led.ts';

  let { onframe }: { onframe: (frame: Rgb[]) => void } = $props();
  let canvas: HTMLCanvasElement, ambient: HTMLCanvasElement, mirrorCv: HTMLCanvasElement;
  let stageWidth = $state(640), stageHeight = $state(360);
  let VW = $state(640);
  let wingW = $state(0);
  let renderer: HordeRenderer | null = null;

  // The rules object is mutated every frame; the screen only sees `summary`, refreshed when something it shows changes.
  const game: HordeState = createHorde();
  let fx = createFx();
  let layout = makeLayout('pad');
  const audio = createHordeAudio();
  let ui = $state({ layout: 'pad' as Layout, playMode: 'free' as PlayMode, speed: '1', aids: DEFAULT_SETTINGS.aids, quality: DEFAULT_SETTINGS.quality,
    motion: DEFAULT_SETTINGS.motion, crush: String(DEFAULT_SETTINGS.crush), crt: DEFAULT_SETTINGS.crt, sound: DEFAULT_SETTINGS.sound });
  let summary = $state({ mode: 'title', wave: 0, hp: 100, score: 0, combo: 0, bombs: 1, charge: 0, armed: false, paused: false });
  const settings = (): HordeSettings => ({ aids: ui.aids, quality: ui.quality, motion: ui.motion,
    crush: Number(ui.crush) as HordeSettings['crush'], crt: ui.crt, sound: ui.sound });

  $effect(() => {
    const aspect = Math.max(16 / 9, stageWidth / Math.max(1, stageHeight));
    const nextVW = Math.max(640, Math.round(H * aspect));
    const nextWingW = Math.max(0, Math.round((nextVW - W) / 2));
    if (nextVW !== VW || nextWingW !== wingW) {
      VW = nextVW;
      wingW = nextWingW;
      layout = makeLayout(game.layout, wingW, VW);
      if (renderer) renderer.invalidate();
    }
  });

  // Launchpad pads in, as before: the app calls this for every hardware or on-screen pad.
  export function padInput(note: number, pressed: boolean) {
    if (!pressed) return;
    audio.init();
    const { column, row } = noteCell(note);
    padPress(game, column, row);
  }

  function restart(patch: Partial<Pick<HordeState, 'layout' | 'playMode'>>) {
    resetHorde(game, patch);
    fx = createFx();
    layout = makeLayout(game.layout, wingW, VW);
    if (renderer) renderer.invalidate();
    drainEvents(game);
  }
  const changeLayout = () => restart({ layout: ui.layout });
  const changeMode = () => restart({ playMode: ui.playMode });
  $effect(() => { game.speed = Number(ui.speed) || 1; });
  $effect(() => { audio.setMode(ui.sound); });

  function canvasPoint(event: PointerEvent) {
    const bounds = canvas.getBoundingClientRect();
    const vw = layout.VW || W;
    return {
      x: (event.clientX - bounds.left) * vw / bounds.width,
      y: (event.clientY - bounds.top) * H / bounds.height
    };
  }
  function pointerDown(event: PointerEvent) {
    audio.init();
    if (game.mode !== 'play') { hitCell(game, 1, 1); return; }
    const { x, y } = canvasPoint(event);
    const cell = cellAt(layout, x, y);
    if (cell) {
      if ((event.button === 2 || event.shiftKey) && !game.armed) armBomb(game);
      hitCell(game, cell.column, cell.row);
      return;
    }
    if (layout.side) {
      for (const row of [8, 7]) {
        const rect = layout.side(row);
        if (x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h) { hitCell(game, 9, row); return; }
      }
    } else {
      const vw = layout.VW || W;
      if (Math.hypot(x - (vw - 120), y - 318) < 18) armBomb(game);
      if (Math.hypot(x - (vw - 50), y - 318) < 18) freezeEnemies(game);
    }
  }

  function keydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement | null;
    if (event.defaultPrevented || event.ctrlKey || event.altKey || event.metaKey || target?.closest('input, textarea, select')) return;
    if (event.code === 'Space' || event.code.startsWith('Arrow') || event.code === 'Slash' || event.code === 'Quote') event.preventDefault();
    if (event.repeat) return;
    audio.init();
    if (event.code === 'KeyP' && game.layout === 'pad') { togglePause(game); return; }
    if (event.code === 'Backquote') { togglePause(game); return; }
    if (event.code === 'Space' || event.code === 'Enter') {
      if (game.mode === 'play') freezeEnemies(game); else hitCell(game, 1, 1);
      return;
    }
    if (event.code === 'ShiftLeft' || event.code === 'ShiftRight') { if (game.mode === 'play' && !game.armed) armBomb(game); return; }
    const key = CODE_TO_CELL[event.code];
    if (!key) return;
    if (game.layout === 'keys') { event.preventDefault(); hitCell(game, key.column, key.row); }
    else if (game.mode !== 'play') hitCell(game, 1, 1);
  }
  function keyup(event: KeyboardEvent) {
    if ((event.code === 'ShiftLeft' || event.code === 'ShiftRight') && game.armed && game.layout === 'keys') disarmBomb(game);
  }

  function drawMirror(frame: Rgb[]) {
    if (!mirrorCv) return;
    const mctx = mirrorCv.getContext('2d');
    if (!mctx) return;
    mctx.fillStyle = '#06080d';
    mctx.fillRect(0, 0, 72, 72);
    const padSize = 6.8;
    const gap = 1.7;
    const startX = (72 - (8 * padSize + 7 * gap)) / 2;
    const startY = (72 - (8 * padSize + 7 * gap)) / 2;
    for (let r = 1; r <= 8; r++) {
      for (let c = 1; c <= 8; c++) {
        const idx = (r - 1) * 8 + (c - 1);
        const rgb = frame[idx] || [0, 0, 0];
        const x = startX + (c - 1) * (padSize + gap);
        const y = startY + (8 - r) * (padSize + gap);
        const isLit = rgb[0] > 1 || rgb[1] > 1 || rgb[2] > 1;
        mctx.fillStyle = isLit
          ? `rgb(${Math.min(255, Math.round(rgb[0] * 4.05))}, ${Math.min(255, Math.round(rgb[1] * 4.05))}, ${Math.min(255, Math.round(rgb[2] * 4.05))})`
          : '#161924';
        mctx.beginPath();
        mctx.roundRect(x, y, padSize, padSize, 1.2);
        mctx.fill();
      }
    }
  }

  onMount(() => {
    renderer = createRenderer(canvas);
    const ambientCtx = ambient.getContext('2d')!;
    let running = true, last = performance.now(), lastLed = 0, frames = 0;
    const loop = (now: number) => {
      if (!running) return;
      const dt = Math.min(.05, (now - last) / 1000); last = now;
      const cfg = settings();
      tickHorde(game, dt);
      applyEvents(fx, layout, drainEvents(game), audio, cfg);
      updateFx(fx, game, dt);
      renderer!.draw(game, fx, layout, cfg);
      if (++frames % 4 === 0) ambientCtx.drawImage(canvas, 0, 0, ambient.width, ambient.height);
      const next = { mode: game.mode, wave: game.wave, hp: game.hp, score: game.score, combo: game.combo, bombs: game.bombs,
        charge: Math.floor(game.charge), armed: game.armed, paused: game.paused };
      if (Object.keys(next).some(key => next[key as keyof typeof next] !== summary[key as keyof typeof summary])) summary = next;
      if (now - lastLed > 33) {
        const frame = ledFrame(game, game.time, cfg.aids);
        onframe(frame);
        drawMirror(frame);
        lastLed = now;
      }
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
    return () => { running = false; };
  });
</script>

<svelte:window onkeydown={keydown} onkeyup={keyup} />

<div class="horde-screen">
  <div class="game-head lcd">
    <span class="head-title">HORDE · HOLD THE BEACH</span>
    <span class="head-stats">
      <span>WAVE {summary.wave}</span> · <span>BASE {summary.hp}%</span> · <span>SCORE {summary.score}</span> · <span>COMBO ×{summary.combo}</span>
    </span>
  </div>

  <div class="horde-main">
    <aside class="station-console lcd" aria-label="Horde defense console">
      <div class="console-head">
        <span class="station-lamp" class:on={summary.mode === 'play'}></span>
        <span class="station-title">DEFENSE CONSOLE</span>
      </div>

      <div class="settings-fields">
        <label>ARENA <select bind:value={ui.layout} onchange={changeLayout}><option value="pad">Launchpad 8×8</option><option value="keys">Keyboard 10×4</option></select></label>
        <label>MODE <select bind:value={ui.playMode} onchange={changeMode}><option value="free">Free</option><option value="beat">Beat</option></select></label>
        <label>SPEED <select bind:value={ui.speed}><option value="0.8">Easy</option><option value="1">Normal</option><option value="1.25">Hard</option></select></label>
        <label>AIDS <select bind:value={ui.aids}><option value="full">Full</option><option value="medium">Medium</option><option value="low">Low</option></select></label>
        <label>QUALITY <select bind:value={ui.quality}><option value="high">High</option><option value="low">Low</option></select></label>
        <label>MOTION <select bind:value={ui.motion}><option value="full">Full</option><option value="reduce">Reduce</option></select></label>
        <label>CRUSH <select bind:value={ui.crush}><option value="0">Off</option><option value="3">3 bit</option><option value="4">4 bit</option><option value="5">5 bit</option></select></label>
        <label>CRT <select bind:value={ui.crt}><option value="on">On</option><option value="curved">Curved</option><option value="off">Off</option></select></label>
        <label>SOUND <select bind:value={ui.sound}><option value="sfx">SFX</option><option value="synth">Synth</option><option value="off">Off</option></select></label>
      </div>

      <div class="mirror-section">
        <div class="mirror-label">LAUNCHPAD MIRROR</div>
        <div class="mirror-box">
          <canvas bind:this={mirrorCv} width="72" height="72" class="mirror-canvas" aria-label="Launchpad LED mirror"></canvas>
        </div>
      </div>

      <div class="telemetry">
        <div class="tele-line"><span>PATROL</span> <span class="status-ok">ACTIVE</span></div>
        <div class="tele-line"><span>GRID</span> <span>{ui.layout === 'pad' ? '8×8 PAD' : '10×4 KEYS'}</span></div>
        <div class="tele-line"><span>RADAR</span> <span>{summary.mode === 'play' ? 'ENGAGED' : 'STANDBY'}</span></div>
      </div>
    </aside>

    <div class="stage" bind:clientWidth={stageWidth} bind:clientHeight={stageHeight}>
      <canvas class="ambient" bind:this={ambient} width="64" height="36" aria-hidden="true"></canvas>
      <div class="wrap" class:curved={ui.crt === 'curved'} class:nocrt={ui.crt === 'off'}>
        <canvas bind:this={canvas} width={VW} height={H} aria-label="Horde game: hit the pads beneath the approaching enemies"
          onpointerdown={pointerDown} oncontextmenu={event => event.preventDefault()}></canvas>
        <div class="crt scan"></div><div class="crt vig"></div>
      </div>
    </div>
  </div>

  <div class="game-controls lcd" aria-label="Horde controls">
    <button onclick={() => togglePause(game)} disabled={summary.mode !== 'play'} aria-pressed={summary.paused}>{summary.paused ? 'RESUME' : 'PAUSE'}</button>
    <button onclick={() => armBomb(game)} disabled={summary.mode !== 'play'} aria-label="Arm bomb" class:active={summary.armed}>BOMB {summary.bombs}{summary.armed ? ' ARMED' : ''}</button>
    <button onclick={() => freezeEnemies(game)} disabled={summary.mode !== 'play' || summary.charge < 100} aria-label="Freeze enemies">FREEZE {summary.charge}%</button>
    <button onclick={() => restart({})} disabled={summary.mode === 'title'}>TITLE</button>
    <span class="hint silk">{game.layout === 'pad' ? 'CLICK / PAD = SHOOT · SIDE ▶ TOP = BOMB · NEXT = FREEZE · P PAUSE' : 'KEYS = SHOOT · SHIFT+KEY = BOMB · SPACE = FREEZE · ` PAUSE'}</span>
  </div>
</div>

<style>
  .horde-screen {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    gap: 4px;
    background: #080a12;
  }
  .game-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 2px 10px;
    height: 24px;
    flex: none;
    background: rgba(14, 18, 28, 0.7);
    border-bottom: 1px solid var(--line);
    font-size: 11px;
    letter-spacing: .06em;
  }
  .head-title {
    color: var(--lcd);
    font-weight: 600;
  }
  .head-stats {
    font-variant-numeric: tabular-nums;
    color: var(--lcd-dim);
  }
  .head-stats span {
    color: var(--lcd);
  }

  .horde-main {
    flex: 1;
    display: flex;
    min-height: 0;
    min-width: 0;
    gap: 6px;
    overflow: hidden;
  }

  /* Left sidebar station console */
  .station-console {
    width: 184px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding: 7px 9px;
    background: #0d111a;
    border: 1px solid var(--line);
    border-radius: 3px;
    overflow-y: auto;
    font-size: 11px;
  }
  .console-head {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-bottom: 4px;
    border-bottom: 1px solid var(--line);
  }
  .station-lamp {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #333;
    box-shadow: 0 0 0 1px #000 inset;
  }
  .station-lamp.on {
    background: #3ddc84;
    box-shadow: 0 0 6px #3ddc84;
  }
  .station-title {
    font-size: 10px;
    letter-spacing: .1em;
    font-weight: 700;
    color: var(--orange);
  }

  .settings-fields {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .settings-fields label {
    display: flex;
    justify-content: space-between;
    align-items: center;
    color: var(--lcd-dim);
    font-size: 10px;
    letter-spacing: .04em;
  }
  .settings-fields select {
    color: var(--lcd);
    background: var(--glass);
    border: 1px solid var(--line);
    border-radius: 2px;
    font: inherit;
    font-size: 10px;
    padding: 2px 4px;
    max-width: 108px;
  }
  .settings-fields select:focus-visible {
    border-color: var(--focus);
    outline: none;
  }

  .mirror-section {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin-top: 2px;
  }
  .mirror-label {
    font-size: 9px;
    letter-spacing: .08em;
    color: var(--lcd-dim);
    text-transform: uppercase;
    opacity: .85;
  }
  .mirror-box {
    display: flex;
    justify-content: center;
    align-items: center;
    padding: 5px 0;
    background: #06080d;
    border: 1px solid var(--line);
    border-radius: 3px;
  }
  .mirror-canvas {
    width: 72px;
    height: 72px;
    image-rendering: pixelated;
    border-radius: 2px;
    display: block;
  }

  .telemetry {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding-top: 5px;
    border-top: 1px solid var(--line);
    font-size: 9px;
    color: var(--lcd-dim);
  }
  .tele-line {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .tele-line span:last-child {
    color: var(--lcd);
    font-weight: 600;
  }
  .status-ok {
    color: #3ddc84 !important;
  }

  /* Right Stage */
  .stage {
    flex: 1;
    min-height: 0;
    min-width: 0;
    display: flex;
    overflow: hidden;
    background: #000;
    border: 1px solid var(--line);
    border-radius: 3px;
    box-shadow: inset 0 0 20px #0b1930;
    position: relative;
  }
  .ambient {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    filter: blur(14px) saturate(1.2) brightness(.8);
    transform: scale(1.05);
    pointer-events: none;
    cursor: default;
  }
  .wrap {
    position: relative;
    width: 100%;
    height: 100%;
    display: flex;
  }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
    image-rendering: pixelated;
    image-rendering: crisp-edges;
    background: #000;
    cursor: crosshair;
  }
  .crt { position: absolute; inset: 0; pointer-events: none; border-radius: 4px; }
  .crt.scan { background: repeating-linear-gradient(to bottom, rgba(0,0,0,0) 0 2px, rgba(0,0,0,.12) 2px 3px); mix-blend-mode: multiply; }
  .crt.vig { box-shadow: inset 0 0 85px rgba(0,0,0,.46), inset 0 0 16px rgba(0,0,0,.52); }
  .wrap.curved canvas { border-radius: 14px; box-shadow: 0 0 0 3px #181c26, 0 12px 36px rgba(0,0,0,.75); }
  .wrap.curved .crt { border-radius: 14px; }
  .wrap.curved .crt.vig { box-shadow: inset 0 0 110px rgba(0,0,0,.62), inset 0 0 28px rgba(0,0,0,.75), inset 0 2px 6px rgba(255,255,255,.08); }
  .wrap.nocrt .crt { display: none; }

  /* Bottom Controls */
  .game-controls {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    flex: none;
    padding: 0 4px;
  }
  .game-controls button {
    color: inherit;
    background: var(--glass);
    border: 1px solid var(--line);
    font: inherit;
    cursor: pointer;
    padding: 2px 10px;
    border-radius: 2px;
  }
  .game-controls button.active, .game-controls button:focus-visible { color: var(--silk); border-color: var(--silk); }
  .game-controls button:disabled { opacity: .5; cursor: default; }
  .hint { margin-left: auto; opacity: .75; font-size: 10px; }
</style>
