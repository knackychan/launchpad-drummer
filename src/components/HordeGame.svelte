<script lang="ts">
  import { onMount } from 'svelte';
  import { armBomb, createHorde, disarmBomb, drainEvents, freezeEnemies, hitCell, ledFrame, noteCell, padPress, resetHorde,
    tickHorde, togglePause, type HordeState, type Layout, type PlayMode } from '../lib/horde.ts';
  import { CODE_TO_CELL, H, W, cellAt, makeLayout } from '../lib/hordeLayout.ts';
  import { DEFAULT_SETTINGS, applyEvents, createFx, updateFx, type HordeSettings } from '../lib/hordeFx.ts';
  import { createRenderer } from '../lib/hordeRender.ts';
  import { createHordeAudio } from '../lib/hordeAudio.ts';
  import type { Rgb } from '../lib/led.ts';

  let { onframe }: { onframe: (frame: Rgb[]) => void } = $props();
  let canvas: HTMLCanvasElement;
  let stageWidth = $state(640), stageHeight = $state(360);

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
  const fit = $derived(Math.max(.25, Math.min((stageWidth - 4) / W, (stageHeight - 4) / H)));

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
    layout = makeLayout(game.layout);
    drainEvents(game);
  }
  const changeLayout = () => restart({ layout: ui.layout });
  const changeMode = () => restart({ playMode: ui.playMode });
  $effect(() => { game.speed = Number(ui.speed) || 1; });
  $effect(() => { audio.setMode(ui.sound); });

  function canvasPoint(event: PointerEvent) {
    const bounds = canvas.getBoundingClientRect();
    return { x: (event.clientX - bounds.left) * W / bounds.width, y: (event.clientY - bounds.top) * H / bounds.height };
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
      if (Math.hypot(x - 520, y - 318) < 18) armBomb(game);
      if (Math.hypot(x - 590, y - 318) < 18) freezeEnemies(game);
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

  onMount(() => {
    const renderer = createRenderer(canvas);
    let running = true, last = performance.now(), lastLed = 0;
    const loop = (now: number) => {
      if (!running) return;
      const dt = Math.min(.05, (now - last) / 1000); last = now;
      const cfg = settings();
      tickHorde(game, dt);
      applyEvents(fx, layout, drainEvents(game), audio, cfg);
      updateFx(fx, game, dt);
      renderer.draw(game, fx, layout, cfg);
      const next = { mode: game.mode, wave: game.wave, hp: game.hp, score: game.score, combo: game.combo, bombs: game.bombs,
        charge: Math.floor(game.charge), armed: game.armed, paused: game.paused };
      if (Object.keys(next).some(key => next[key as keyof typeof next] !== summary[key as keyof typeof summary])) summary = next;
      if (now - lastLed > 33) { onframe(ledFrame(game, game.time, cfg.aids)); lastLed = now; }
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
    return () => { running = false; };
  });
</script>

<svelte:window onkeydown={keydown} onkeyup={keyup} />

<div class="horde-screen">
  <div class="game-head lcd"><span>HORDE · HOLD THE BEACH</span><span>WAVE {summary.wave} · BASE {summary.hp}% · SCORE {summary.score}</span></div>
  <div class="settings lcd" aria-label="Horde settings">
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
  <div class="stage" bind:clientWidth={stageWidth} bind:clientHeight={stageHeight}>
    <div class="wrap" class:curved={ui.crt === 'curved'} class:nocrt={ui.crt === 'off'} style:width="{W * fit}px" style:height="{H * fit}px">
      <canvas bind:this={canvas} width={W} height={H} aria-label="Horde game: hit the pads beneath the approaching enemies"
        onpointerdown={pointerDown} oncontextmenu={event => event.preventDefault()}></canvas>
      <div class="crt scan"></div><div class="crt vig"></div>
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
  .horde-screen { height: 100%; display: grid; grid-template-rows: 24px auto minmax(0,1fr) auto; min-height: 0; gap: 6px; }
  .game-head { display:flex; justify-content:space-between; align-items:center; padding:0 8px; }
  .settings { display: flex; flex-wrap: wrap; gap: 4px 10px; padding: 0 4px; font-size: 11px; }
  .settings label { display: flex; align-items: center; gap: 4px; white-space: nowrap; color: var(--lcd-dim); }
  .settings select { color: var(--lcd); background: var(--glass); border: 1px solid var(--line); font: inherit; font-size: 11px; padding: 1px 2px; }
  .settings select:focus-visible { border-color: var(--focus); outline: none; }
  .stage { min-height: 0; display: grid; place-items: center; overflow: hidden; background: #000; border: 1px solid var(--line); border-radius: 3px; box-shadow: inset 0 0 20px #0b1930; }
  .wrap { position: relative; }
  canvas { display: block; width: 100%; height: 100%; image-rendering: pixelated; image-rendering: crisp-edges; background: #000; cursor: crosshair; }
  .crt { position: absolute; inset: 0; pointer-events: none; border-radius: 6px; }
  .crt.scan { background: repeating-linear-gradient(to bottom, rgba(0,0,0,0) 0 2px, rgba(0,0,0,.12) 2px 3px); mix-blend-mode: multiply; }
  .crt.vig { box-shadow: inset 0 0 85px rgba(0,0,0,.46), inset 0 0 16px rgba(0,0,0,.52); }
  .wrap.curved canvas { border-radius: 18px; box-shadow: 0 0 0 3px #181c26, 0 12px 36px rgba(0,0,0,.75); }
  .wrap.curved .crt { border-radius: 18px; }
  .wrap.curved .crt.vig { box-shadow: inset 0 0 110px rgba(0,0,0,.62), inset 0 0 28px rgba(0,0,0,.75), inset 0 2px 6px rgba(255,255,255,.08); }
  .wrap.nocrt .crt { display: none; }
  .game-controls { display: flex; align-items: center; gap: 6px; }
  .game-controls button { color: inherit; background: var(--glass); border: 1px solid var(--line); font: inherit; cursor: pointer; padding: 2px 10px; }
  .game-controls button.active, .game-controls button:focus-visible { color: var(--silk); border-color: var(--silk); }
  .game-controls button:disabled { opacity: .5; cursor: default; }
  .hint { margin-left: auto; opacity: .75; }
</style>
