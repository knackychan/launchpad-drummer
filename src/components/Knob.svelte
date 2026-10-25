<script lang="ts">
  import { getContext } from 'svelte';
  import { isLearned, isLearning, learnable, LEARN, type LearnCtx } from '../lib/learnable.ts';
  import { arcPath, type Arc } from '../lib/synth.ts';
  import type { MenuItem } from './ContextMenu.svelte';
  // Vector-ring knob (UI spec 4.5): vertical drag (150 px = full range, Shift 10× finer), wheel steps,
  // double-click = default, arrows when focused (Shift fine), Home / End = min / max.
  let {
    label = '', ariaLabel, value, min, max, def, step = 1, fine = 0.1, size = 40, fmt, row = false, learnId, onchange,
    arcs = [], more = 0, live = null, selected = null, onselect, tag, tagTitle, dest, onalt, menuExtra,
  }: {
    label?: string; ariaLabel?: string; value: number; min: number; max: number; def: number;
    step?: number; fine?: number; size?: number; fmt: (v: number) => string; row?: boolean;
    /** MIDI-learnable parameter id (right-click → Learn MIDI CC / Reset). */
    learnId?: string;
    onchange: (v: number) => void;
    arcs?: Arc[]; more?: number; live?: number | null; selected?: number | null;
    onselect?: (slot: number) => void; tag?: string; tagTitle?: string; dest?: string;
    onalt?: (delta: number) => void; menuExtra?: () => MenuItem[];
  } = $props();

  const learn = getContext<LearnCtx | undefined>(LEARN);
  const t = $derived((Math.min(max, Math.max(min, value)) - min) / (max - min));
  const clamp = (v: number) => Math.min(max, Math.max(min, v));
  const quant = (v: number, s: number) => Math.round(v / s) * s;
  function apply(v: number) {
    const next = clamp(v);
    if (next !== value) onchange(next);
  }

  function down(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    const el = event.currentTarget as HTMLElement;
    el.focus();
    el.setPointerCapture(event.pointerId);
    let last = event.clientY, v = value;
    const move = (e: PointerEvent) => {
      const delta = ((last - e.clientY) * (max - min)) / (e.shiftKey ? 1500 : 150);
      if (e.altKey && onalt) { last = e.clientY; onalt(delta); return; }
      v = clamp(v + delta);
      last = e.clientY;
      apply(quant(v, e.shiftKey ? fine : step));
    };
    const up = () => { el.removeEventListener('pointermove', move); el.removeEventListener('pointerup', up); };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
  }
  function wheel(event: WheelEvent) {
    event.preventDefault();
    const s = event.shiftKey ? fine : step;
    apply(quant(value + (event.deltaY < 0 ? s : -s), s));
  }
  function key(event: KeyboardEvent) {
    const s = event.shiftKey ? fine : step;
    const d = ({ ArrowUp: s, ArrowRight: s, ArrowDown: -s, ArrowLeft: -s } as Record<string, number>)[event.key];
    if (d != null) { event.preventDefault(); event.stopPropagation(); apply(quant(value + d, s)); }
    if (event.key === 'Home') { event.preventDefault(); apply(min); }
    if (event.key === 'End') { event.preventDefault(); apply(max); }
  }
</script>

<div class="knob-wrap" class:row>
  {#if label}<div class="silk">{label}</div>{/if}
  <div
    class="knob"
    class:learning={isLearning(learn, learnId)}
    class:learned={isLearned(learn, learnId)}
    data-learn={learnId}
    use:learnable={{ ctx: learn, id: learnId, reset: () => apply(def), extra: menuExtra }}
    data-dest={dest}
    role="slider"
    tabindex="0"
    aria-label={ariaLabel ?? label}
    aria-valuemin={min}
    aria-valuemax={max}
    aria-valuenow={value}
    aria-valuetext={fmt(value)}
    style:--size="{size}px"
    style:--t={t.toFixed(3)}
    onpointerdown={down}
    onwheel={wheel}
    ondblclick={() => apply(def)}
    onkeydown={key}
  >
    <div class="cap" style:transform="rotate({-135 + 270 * t}deg)"></div>
    {#if arcs.length || live !== null}
      <svg class="mods" width={size} height={size} viewBox="0 0 {size} {size}" aria-hidden="true">
        {#each arcs as arc, i}<path d={arcPath(size/2, size/2, size/2 - 3 - 3*i, arc.from, arc.to)} stroke={arc.color} stroke-width="2" fill="none" />{/each}
        {#if live !== null}<circle r="2.5" fill="var(--lcd)" cx={size/2 + (size/2 - 3) * Math.cos((-225 + 270 * live) * Math.PI/180)} cy={size/2 + (size/2 - 3) * Math.sin((-225 + 270 * live) * Math.PI/180)} />{/if}
      </svg>
    {/if}
    {#if more > 0}<span class="mod-more">+{more}</span>{/if}
  </div>
  {#if tag}<span class="xl-tag" title={tagTitle}>{tag}</span>{/if}
  {#if arcs.length}<div class="mod-chips">{#each arcs as arc}<button class:selected={selected === arc.slot} style:background={arc.color} aria-label="Select modulation {arc.slot + 1}" aria-pressed={selected === arc.slot} onclick={() => onselect?.(arc.slot)}></button>{/each}</div>{/if}
  <div class="lcd knob-val">{fmt(value)}</div>
</div>

<style>
  .mods { position: absolute; inset: 0; pointer-events: none; }
  .mod-more { position: absolute; right: -8px; bottom: -4px; font: 400 9px var(--f-lcd); color: var(--lcd); }
  .xl-tag { position: absolute; right: 0; top: 0; font: 600 8px var(--f-silk); color: var(--vec); }
  .mod-chips { display: flex; gap: 4px; }
  .mod-chips button { width: 6px; height: 6px; border: 0; padding: 0; border-radius: 50%; cursor: pointer; }
  .mod-chips button.selected { outline: 1px solid var(--focus); outline-offset: 2px; }
  .knob-wrap { display: flex; flex-direction: column; align-items: center; gap: 4px; position: relative; }
  .knob-wrap.row { flex-direction: row; gap: 8px; }
  .knob {
    width: var(--size); height: var(--size); border-radius: 50%; position: relative; cursor: ns-resize; touch-action: none; flex: none;
    background: radial-gradient(circle, var(--tag) 0 53%, transparent 54%),
      conic-gradient(from 225deg, var(--warn) 0 calc(var(--t) * 270deg), var(--knob-track) 0 270deg, transparent 0);
    box-shadow: 0 5px 0 rgba(0,0,0,.3), 0 8px 14px rgba(0,0,0,.65);
  }
  .cap { position: absolute; inset: 0; border-radius: 50%; pointer-events: none; }
  .cap::after { content: ''; position: absolute; left: calc(50% - 1px); top: 13%; width: 2px; height: 28%; border-radius: 1px; background: var(--lcd); box-shadow: 0 0 4px rgba(236,227,204,.6); }
  .knob-val { font-size: 11px; min-width: 56px; text-align: center; color: var(--lcd-dim); white-space: nowrap; }
  .row .knob-val { min-width: 0; text-align: left; }
</style>
