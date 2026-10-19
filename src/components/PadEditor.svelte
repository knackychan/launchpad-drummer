<script lang="ts">
  // Pad editor (UI spec 4.3), shown in the Play side drawer.
  import Knob from './Knob.svelte';
  import { LAUNCHPAD_PALETTE, groupLegend, padLabel } from '../lib/model.ts';
  import type { Group, Pad, PadMode, SamplePeaks } from '../lib/model.ts';
  import { ledToCss } from '../lib/led.ts';
  import { DB_FLOOR, fmtDb, fmtSemis } from '../lib/mix.ts';

  let { bank, x, y, pad, groups, peaks, missing, learning = false, binding = null, onupdate, onlocate, onclear, onlearn, onunbind, ontrigger, onrelease, drop = false }: {
    drop?: boolean;
    /** MIDI learn is armed for this pad. */
    learning?: boolean;
    /** The note bound to this pad (`index` into the set's bindings), if any. */
    binding?: { index: number; number: number; device: string } | null;
    bank: number;
    /** The pad's sample file cannot be found. */
    missing: boolean;
    x: number;
    y: number;
    /** null = empty pad */
    pad: Pad | null;
    groups: Group[];
    peaks: SamplePeaks | null;
    /** `field` names the control being dragged so the backend can fold a drag into one undo step. */
    onupdate: (pad: Pad, field?: string) => void;
    onlocate: () => void;
    onclear: () => void;
    onlearn: () => void;
    onunbind: (index: number) => void;
    ontrigger: (x: number, y: number) => void;
    onrelease: (x: number, y: number) => void;
  } = $props();

  let showRgb = $state(false);
  let canvas = $state<HTMLCanvasElement>();
  let root: HTMLDivElement;
  export function rect() { return root.getBoundingClientRect(); }

  function update<K extends keyof Pad>(key: K, value: Pad[K]) {
    if (pad) onupdate({ ...pad, [key]: value }, key);
  }
  const sameColor = (a: readonly number[], b: readonly number[]) => a[0] === b[0] && a[1] === b[1] && a[2] === b[2];

  // Waveform: pad colour inside the trim, dim outside, white start / end handles.
  function draw() {
    if (!canvas || !pad) return;
    const dpr = devicePixelRatio || 1, W = canvas.clientWidth, H = canvas.clientHeight;
    if (!W) return;
    canvas.width = W * dpr; canvas.height = H * dpr;
    const g = canvas.getContext('2d');
    if (!g) return;
    g.scale(dpr, dpr);
    g.clearRect(0, 0, W, H);
    if (!peaks) {
      g.fillStyle = 'rgba(79,208,198,.35)'; g.fillRect(0, H / 2, W, 1);
      g.fillStyle = '#8fa3bf'; g.font = '11px "Share Tech Mono", monospace'; g.textAlign = 'center';
      g.fillText(pad.sample ? 'SAMPLE NOT LOADED' : 'NO SAMPLE', W / 2, H / 2 - 8);
      return;
    }
    const len = peaks.lengthMs, s = (pad.trimStartMs / len) * W, e = ((pad.trimEndMs ?? len) / len) * W;
    const col = ledToCss(pad.color), n = peaks.peaks.length;
    for (let px = 0; px < W; px += 2) {
      const a = Math.min(1, peaks.peaks[Math.min(n - 1, Math.floor((px / W) * n))]) * (H / 2 - 4);
      g.fillStyle = px < s || px > e ? '#2a2d30' : col;
      g.fillRect(px, H / 2 - a, 1.5, Math.max(1, a * 2));
    }
    g.fillStyle = '#fff';
    for (const hx of [s, e]) { g.fillRect(hx - 1, 0, 2, H); g.fillRect(hx - (hx === s ? 0 : 7), 0, 7, 9); }
  }
  $effect(() => { pad; peaks; canvas; draw(); });

  function waveDown(event: PointerEvent) {
    if (!pad || !canvas) return;
    const r = canvas.getBoundingClientRect(), px = event.clientX - r.left;
    const len = peaks?.lengthMs ?? 0;
    const sx = len ? (pad.trimStartMs / len) * r.width : -99, ex = len ? ((pad.trimEndMs ?? len) / len) * r.width : -99;
    const which = Math.abs(px - sx) < 8 ? 's' : Math.abs(px - ex) < 8 ? 'e' : null;
    if (!which) {
      ontrigger(x, y);
      const up = () => { onrelease(x, y); window.removeEventListener('pointerup', up); };
      window.addEventListener('pointerup', up);
      return;
    }
    canvas.setPointerCapture(event.pointerId);
    const el = canvas;
    const move = (e: PointerEvent) => {
      if (!pad) return;
      const ms = Math.min(len, Math.max(0, ((e.clientX - r.left) / r.width) * len));
      if (which === 's') update('trimStartMs', Math.round(Math.min(ms, (pad.trimEndMs ?? len) - 10)));
      else { const v = Math.max(ms, pad.trimStartMs + 10); update('trimEndMs', v >= len - 1 ? null : Math.round(v)); }
    };
    const up = () => { el.removeEventListener('pointermove', move); el.removeEventListener('pointerup', up); };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
  }
  function waveHover(event: PointerEvent) {
    if (!pad || !canvas || !peaks) return;
    const r = canvas.getBoundingClientRect(), px = event.clientX - r.left, len = peaks.lengthMs;
    const near = [pad.trimStartMs, pad.trimEndMs ?? len].some((v) => Math.abs((v / len) * r.width - px) < 8);
    canvas.style.cursor = near ? 'ew-resize' : 'pointer';
  }
</script>

<div bind:this={root} class="editor screen" class:drop aria-label="Pad editor, bank {bank + 1}">
  <div class="ed-head">
    <span class="lcd lcd-dim">R{y + 1}·C{x + 1}</span>
    <span class="lcd ed-name">{pad?.sample ? padLabel(pad.sample).toUpperCase() : 'EMPTY'}</span>
    {#if pad && peaks}<span class="lcd lcd-dim small">{(peaks.lengthMs / 1000).toFixed(2)} s · {(peaks.rate / 1000).toFixed(1)} kHz</span>{/if}
    {#if pad}
      <button type="button" class="key small clear" title="Clear pad" onclick={onclear}>CLEAR</button>
    {/if}
  </div>

  {#if !pad}
    <div class="empty-msg">Empty pad. Drop a sample here, drag one from the browser, or press Enter on a file in the browser.</div>
  {:else}
    {#if missing}
      <div class="errbox">
        <span class="errtext">Sample missing: {pad.sample}</span>
        <button type="button" class="key small" onclick={onlocate}>LOCATE…</button>
      </div>
    {/if}
    <div class="ed-sec">
      <div class="silk">SAMPLE · TRIM</div>
      <canvas bind:this={canvas} class="wave" aria-label="Waveform. Drag the handles to trim. Click to audition." onpointerdown={waveDown} onpointermove={waveHover}></canvas>
      <div class="lcd lcd-dim small trim">
        <span>START {Math.round(pad.trimStartMs)} ms</span>
        <span>END {pad.trimEndMs == null ? (peaks ? Math.round(peaks.lengthMs) : '—') : Math.round(pad.trimEndMs)} ms</span>
      </div>
    </div>

    <div class="ed-sec">
      <div class="ed-row between">
        <span class="silk">COLOUR</span>
        <span class="lcd lcd-dim small">{pad.color.join(' · ')}</span>
        <button type="button" class="key small" class:on={showRgb} aria-pressed={showRgb} onclick={() => { showRgb = !showRgb; }}>+RGB</button>
      </div>
      <div class="swatches">
        {#each LAUNCHPAD_PALETTE as c}
          <button
            type="button"
            class="sw"
            class:cur={sameColor(c, pad.color)}
            style:background={ledToCss(c)}
            aria-label="Colour {c.join(' ')}"
            title="R{c[0]} G{c[1]} B{c[2]}"
            onclick={() => update('color', [...c] as [number, number, number])}
          ></button>
        {/each}
      </div>
      {#if showRgb}
        <div class="ed-row rgb">
          {#each ['R', 'G', 'B'] as channel, ci}
            <Knob
              label={channel}
              value={pad.color[ci]}
              min={0}
              max={63}
              def={0}
              step={1}
              fine={1}
              size={30}
              fmt={(v) => String(Math.round(v))}
              onchange={(v) => { if (pad) { const color = [...pad.color] as [number, number, number]; color[ci] = Math.round(v); onupdate({ ...pad, color }, 'colorChannel'); } }}
            />
          {/each}
        </div>
      {/if}
    </div>

    <div class="ed-row knobs">
      <Knob label="VOLUME" value={pad.volumeDb} min={DB_FLOOR} max={6} def={0} step={0.5} fine={0.1} fmt={fmtDb} onchange={(v) => update('volumeDb', v)} />
      <Knob label="PITCH" value={pad.pitchSemitones} min={-24} max={24} def={0} step={1} fine={0.1} fmt={fmtSemis} onchange={(v) => update('pitchSemitones', v)} />
      <div class="ed-sec">
        <span class="silk">MODE</span>
        <div class="switch" role="radiogroup" aria-label="Mode">
          {#each [['oneShot', 'ONE-SHOT'], ['hold', 'HOLD']] as [m, text]}
            <button type="button" class="key" class:on={pad.mode === m} role="radio" aria-checked={pad.mode === m} onclick={() => update('mode', m as PadMode)}>{text}</button>
          {/each}
        </div>
      </div>
    </div>

    <div class="ed-sec">
      <span class="silk">CHOKE</span>
      <div class="ed-row tight" role="radiogroup" aria-label="Choke group">
        {#each [null, 1, 2, 3, 4, 5, 6, 7, 8] as c}
          <button type="button" class="key small" class:on={pad.chokeGroup === c} role="radio" aria-checked={pad.chokeGroup === c} onclick={() => update('chokeGroup', c)}>{c ?? 'OFF'}</button>
        {/each}
      </div>
    </div>

    <div class="ed-sec">
      <div class="ed-row between">
        <span class="silk">MIX GROUP</span>
        <span class="lcd lcd-dim small">{groups[pad.mixGroup] ? groupLegend(pad.mixGroup, groups[pad.mixGroup].name) : ''}</span>
      </div>
      <div class="ed-row tight" role="radiogroup" aria-label="Mix group">
        {#each groups as g, i}
          <button type="button" class="key small" class:on={pad.mixGroup === i} role="radio" aria-checked={pad.mixGroup === i} title={g.name} aria-label="Mix {i + 1} {g.name}" onclick={() => update('mixGroup', i)}>{i + 1}</button>
        {/each}
      </div>
    </div>

    <div class="ed-sec">
      <span class="silk">MIDI</span>
      <div class="ed-row">
        <button type="button" class="key" class:on={learning} class:amber={learning} aria-pressed={learning} title="Bind a note from another MIDI device" onclick={onlearn}>LEARN</button>
        {#if binding}
          <span class="lcd learn-lcd">NOTE {binding.number} · {binding.device}</span>
          <button type="button" class="key small" aria-label="Clear MIDI binding" onclick={() => onunbind(binding.index)}>×</button>
        {:else}
          <span class="tooltip-note">Bind a note from another MIDI device</span>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .editor { flex: 1; min-width: 0; min-height: 0; display: flex; flex-direction: column; gap: 12px; padding: 12px; overflow: auto; }
  .ed-head { display: flex; align-items: baseline; gap: 8px; flex-wrap: nowrap; }
  .ed-head > .lcd:not(.ed-name) { flex-shrink: 0; white-space: nowrap; }
  .ed-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 16px; color: #fff; font-weight: 500; }
  .clear { flex-shrink: 0; margin-left: auto; color: var(--error); }
  .small { font-size: 11px; }
  .learn-lcd { font-size: 11px; color: var(--warn); min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ed-sec { display: flex; flex-direction: column; gap: 6px; }
  .ed-row { display: flex; gap: 6px; flex-wrap: wrap; align-items: center; }
  .ed-row.between { justify-content: space-between; }
  .ed-row.tight { gap: 4px; }
  .ed-row.knobs { gap: 22px; align-items: flex-start; flex-wrap: nowrap; }
  .ed-row.rgb { gap: 14px; }
  .errbox { border: 1px solid rgba(255,59,48,.5); border-radius: 4px; padding: 8px; display: flex; gap: 8px; align-items: center; color: var(--error); font: 400 11px/1.4 var(--f-lcd); }
  .errtext { flex: 1; min-width: 0; overflow-wrap: anywhere; }
  .wave { display: block; width: 100%; height: 72px; border-radius: 2px; background: var(--glass); box-shadow: inset 0 0 0 1px var(--line); cursor: pointer; touch-action: none; }
  .trim { display: flex; justify-content: space-between; }
  .swatches { display: grid; grid-template-columns: repeat(8, 26px); gap: 6px; }
  .sw { width: 26px; height: 22px; border: 0; border-radius: 2px; cursor: pointer; padding: 0; }
  .sw.cur { outline: 2px solid var(--lcd); outline-offset: 2px; }
  .empty-msg, .tooltip-note { color: var(--lcd-dim); font: 400 12px/1.5 var(--f-lcd); }
  .tooltip-note { font-size: 11px; }
</style>
