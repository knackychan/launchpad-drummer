<script lang="ts">
  // LCD value (mockup numLcd): drag up/down (4 px per step), wheel, arrows, double-click = default.
  import { getContext } from 'svelte';
  import { isLearned, isLearning, learnable, LEARN, type LearnCtx } from '../lib/learnable.ts';

  let { label, value, min, max, step = 1, def, fmt = String, learnId, onchange }: {
    label: string; value: number; min: number; max: number; step?: number; def: number;
    fmt?: (v: number) => string; learnId?: string; onchange: (v: number) => void;
  } = $props();

  const learn = getContext<LearnCtx | undefined>(LEARN);
  const put = (v: number) => {
    const next = Math.min(max, Math.max(min, Math.round(v / step) * step));
    if (next !== value) onchange(next);
  };

  function down(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    const el = event.currentTarget as HTMLElement;
    el.focus();
    el.setPointerCapture(event.pointerId);
    const y0 = event.clientY, v0 = value;
    const move = (e: PointerEvent) => put(v0 + Math.round((y0 - e.clientY) / 4) * step);
    const up = () => { el.removeEventListener('pointermove', move); el.removeEventListener('pointerup', up); el.removeEventListener('pointercancel', up); };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
  }
  function key(event: KeyboardEvent) {
    const d = ({ ArrowUp: step, ArrowRight: step, ArrowDown: -step, ArrowLeft: -step } as Record<string, number>)[event.key];
    if (d != null) { event.preventDefault(); event.stopPropagation(); put(value + d); }
  }
</script>

<span
  class="lcd screen num"
  class:learning={isLearning(learn, learnId)}
  class:learned={isLearned(learn, learnId)}
  data-learn={learnId}
  use:learnable={{ ctx: learn, id: learnId, reset: () => put(def) }}
  role="slider"
  tabindex="0"
  aria-label={label}
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={value}
  aria-valuetext={fmt(value)}
  title="{label} — drag, wheel or arrows; double-click resets"
  onpointerdown={down}
  onwheel={(e) => { e.preventDefault(); put(value + (e.deltaY < 0 ? step : -step)); }}
  ondblclick={() => put(def)}
  onkeydown={key}
>{fmt(value)}</span>

<style>
  .num { height: 22px; line-height: 22px; padding: 0 5px; font-size: 11px; min-width: 38px; text-align: center; cursor: ns-resize; touch-action: none; user-select: none; }
</style>
