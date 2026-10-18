<script lang="ts">
  import { onMount } from 'svelte';
  import { createHorde, hitCell, ledFrame, noteCell, tickHorde, type HordeState } from '../lib/horde.ts';
  import type { Rgb } from '../lib/led.ts';

  let { onframe }: { onframe: (frame: Rgb[]) => void } = $props();
  let canvas: HTMLCanvasElement;
  let gameState: HordeState = $state(createHorde());
  let summary = $state({ mode: 'title', playMode: 'free', wave: 0, hp: 100, score: 0, combo: 0, bombs: 1, charge: 0, armed: false });
  const W = 640, H = 360;
  const colors = { grunt: '#5fd38a', runner: '#f5c644', brute: '#f08a2b', splitter: '#a77be6', survivor: '#ffffff', boss: '#e63b2e' };

  export function padInput(note: number, pressed: boolean) {
    if (!pressed) return;
    const { column, row } = noteCell(note);
    hitCell(gameState, column, row);
  }

  function draw() {
    const context = canvas?.getContext('2d');
    if (!context) return;
    const g = context;
    g.imageSmoothingEnabled = false;
    g.fillStyle = '#050b16'; g.fillRect(0, 0, W, H);
    g.fillStyle = '#0a2032'; g.fillRect(0, 0, W, 82);
    for (let x = 0; x < W; x += 32) { g.fillStyle = x % 64 ? '#0e2b3b' : '#15384a'; g.fillRect(x, 56 + Math.sin(gameState.time * 2 + x) * 5, 24, 2); }
    g.fillStyle = '#b89d68'; g.fillRect(126, 79, 388, 254);
    g.fillStyle = '#d9c38f'; g.fillRect(126, 80, 388, 10);
    for (let r = 0; r < 8; r++) for (let c = 0; c < 8; c++) {
      g.fillStyle = (r + c) % 2 ? '#806f50' : '#927d59';
      g.fillRect(130 + c * 48, 96 + (7 - r) * 28, 44, 24);
    }
    g.fillStyle = '#454448'; g.fillRect(119, 323, 402, 19); g.fillStyle = '#ffb21a'; g.fillRect(125, 326, 390 * gameState.hp / 100, 4);
    for (const enemy of gameState.enemies) {
      const x = 132 + (enemy.column - 1) * 48, y = 98 + (7 - enemy.row) * 28;
      g.fillStyle = '#201a26'; g.fillRect(x + 4, enemy.type === 'boss' ? y - 24 : y + 6, enemy.type === 'boss' ? 82 : 34, enemy.type === 'boss' ? 51 : 17);
      g.fillStyle = colors[enemy.type]; g.fillRect(x + 8, enemy.type === 'boss' ? y - 27 : y + 1, enemy.type === 'boss' ? 74 : 26, enemy.type === 'boss' ? 46 : 18);
      g.fillStyle = '#101018'; g.fillRect(x + 13, y + 6, 4, 4); g.fillRect(x + 25, y + 6, 4, 4);
      if (enemy.type === 'boss') {
        const weak = enemy.weak ?? 0;
        g.fillStyle = '#fff6dc'; g.fillRect(x + 13 + (weak % 2) * 48, y + 6 - Math.floor(weak / 2) * 28, 10, 10);
      }
      if (enemy.hp > 1) { g.fillStyle = '#fff6dc'; g.font = 'bold 10px monospace'; g.fillText(String(enemy.hp), x + 18, y + 29); }
    }
    g.fillStyle = '#fff6dc'; g.font = 'bold 20px monospace'; g.fillText(`WAVE ${gameState.wave}     ${String(gameState.score).padStart(6, '0')}`, 20, 35);
    g.font = 'bold 13px monospace'; g.fillStyle = '#ffae2b'; g.fillText(`COMBO ×${gameState.combo}   BASE ${gameState.hp}%`, 20, 58);
    if (gameState.mode !== 'play') {
      g.fillStyle = 'rgba(0,0,0,.74)'; g.fillRect(0, 0, W, H);
      g.fillStyle = gameState.mode === 'over' ? '#e63b2e' : '#f5c644'; g.textAlign = 'center';
      g.font = 'bold 36px monospace'; g.fillText(gameState.mode === 'over' ? 'BEACH LOST' : 'HOLD THE BEACH', W / 2, 140);
      g.font = '14px monospace'; g.fillStyle = '#f3e3b8'; g.fillText('HIT THE LIT PADS · MISSES BREAK COMBO', W / 2, 180);
      g.fillText('PRESS ANY PAD TO START', W / 2, 220); g.textAlign = 'left';
    }
  }

  onMount(() => {
    let running = true, last = performance.now(), lastLed = 0;
    const loop = (now: number) => {
      if (!running) return;
      const dt = Math.min(.05, (now - last) / 1000); last = now;
      tickHorde(gameState, dt); draw();
      summary = { mode: gameState.mode, playMode: gameState.playMode, wave: gameState.wave, hp: gameState.hp,
        score: gameState.score, combo: gameState.combo, bombs: gameState.bombs, charge: gameState.charge, armed: gameState.armed };
      if (now - lastLed > 33) { onframe(ledFrame(gameState)); lastLed = now; }
      requestAnimationFrame(loop);
    };
    requestAnimationFrame(loop);
    return () => { running = false; };
  });
</script>

<div class="horde-screen">
  <div class="game-head lcd"><span>HORDE · HOLD THE BEACH</span><span>WAVE {summary.wave} · BASE {summary.hp}% · SCORE {summary.score}</span></div>
  <canvas bind:this={canvas} width={W} height={H} aria-label="Horde game: hit pads beneath the approaching enemies" onpointerdown={(event) => {
    const bounds = event.currentTarget.getBoundingClientRect();
    const x = (event.clientX - bounds.left) * W / bounds.width, y = (event.clientY - bounds.top) * H / bounds.height;
    if (x >= 130 && x < 514 && y >= 96 && y < 320) padInput(Math.floor((x - 130) / 48) + 11 + (7 - Math.floor((y - 96) / 28)) * 10, true);
    else if (gameState.mode !== 'play') padInput(11, true);
  }}></canvas>
  <div class="game-foot silk">{summary.mode === 'play' ? `COMBO ×${summary.combo} · ON SCREEN OR LAUNCHPAD` : 'PRESS ANY PAD TO DEPLOY'}</div>
  <div class="game-controls lcd" aria-label="Horde controls">
    <button class:active={summary.playMode === 'free'} onclick={() => gameState.playMode = 'free'} disabled={summary.mode === 'play'}>FREE</button>
    <button class:active={summary.playMode === 'beat'} onclick={() => gameState.playMode = 'beat'} disabled={summary.mode === 'play'}>BEAT</button>
    <button onclick={() => padInput(89, true)} disabled={summary.mode !== 'play'} aria-label="Arm bomb">BOMB {summary.bombs}{summary.armed ? ' ARMED' : ''}</button>
    <button onclick={() => padInput(79, true)} disabled={summary.mode !== 'play' || summary.charge < 100} aria-label="Freeze enemies">FREEZE {summary.charge}%</button>
  </div>
</div>

<style>
  .horde-screen { height: 100%; display: grid; grid-template-rows: 28px minmax(0,1fr) 24px 24px; min-height: 0; gap: 6px; }
  .game-head, .game-foot { display:flex; justify-content:space-between; align-items:center; padding:0 8px; }
  canvas { width:100%; height:100%; min-height:0; object-fit:contain; background:#000; image-rendering:pixelated; border:1px solid var(--line); border-radius:3px; box-shadow:inset 0 0 20px #0b1930; }
  .game-controls { display: flex; justify-content: space-between; gap: 6px; margin-top: 4px; }
  .game-controls button { color: inherit; background: var(--glass); border: 1px solid var(--line); font: inherit; cursor: pointer; }
  .game-controls button.active, .game-controls button:focus-visible { color: var(--accent); border-color: var(--accent); }
  .game-controls button:disabled { opacity: .5; cursor: default; }
</style>
