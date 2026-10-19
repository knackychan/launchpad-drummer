<script lang="ts">
  import { getContext } from 'svelte';
  import { isLearned, isLearning, learnable, LEARN, type LearnCtx } from '../lib/learnable.ts';
  // Vector-ring knob (UI spec 4.5): vertical drag (150 px = full range, Shift 10× finer), wheel steps,
  // double-click = default, arrows when focused (Shift fine), Home / End = min / max.
  let {
    label = '', ariaLabel, value, min, max, def, step = 1, fine = 0.1, size = 40, fmt, row = false, learnId, onchange,
  }: {
    label?: string; ariaLabel?: string; value: number; min: number; max: number; def: number;
    step?: number; fine?: number; size?: number; fmt: (v: number) => string; row?: boolean;
    /** MIDI-learnable parameter id (right-click → Learn MIDI CC / Reset). */
    learnId?: string;
    onchange: (v: number) => void;
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
      v = clamp(v + ((last - e.clientY) * (max - min)) / (e.shiftKey ? 1500 : 150));
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
    use:learnable={{ ctx: learn, id: learnId, reset: () => apply(def) }}
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
  </div>
  <div class="lcd knob-val">{fmt(value)}</div>
</div>

<style>
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
