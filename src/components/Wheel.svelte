<script lang="ts">
  // Keys wheel (mockup 1269–1281): PITCH is spring-loaded (−1…+1, returns to centre in 120 ms),
  // MOD stays where it is put (0…1). The value is sent at most every 16 ms, and the last one always.
  import { getContext, onDestroy } from 'svelte';
  import { isLearned, isLearning, learnable, LEARN, type LearnCtx } from '../lib/learnable.ts';

  let { label, spring = false, value = 0, learnId, onchange }: {
    label: string;
    spring?: boolean;
    /** Position set from outside (the hardware wheel); the thumb follows it. */
    value?: number;
    learnId?: string;
    onchange: (v: number) => void;
  } = $props();

  const learn = getContext<LearnCtx | undefined>(LEARN);
  const lo = $derived(spring ? -1 : 0);
  let v = $state(0);
  $effect(() => { v = value; });
  const clamp = (x: number) => Math.min(1, Math.max(lo, x));

  // Throttle: a leading send, then at most one per 16 ms with the newest value.
  let lastSent = 0, pending: number | null = null, timer = 0;
  function send(x: number) {
    const now = performance.now();
    if (now - lastSent >= 16) { lastSent = now; pending = null; onchange(x); return; }
    pending = x;
    if (!timer) timer = window.setTimeout(() => {
      timer = 0; lastSent = performance.now();
      if (pending !== null) { const p = pending; pending = null; onchange(p); }
    }, 16 - (now - lastSent));
  }
  function set(x: number) { v = clamp(x); send(v); }

  let springRaf = 0;
  function release() {
    if (!spring) return;
    cancelAnimationFrame(springRaf);
    if (matchMedia?.('(prefers-reduced-motion: reduce)').matches || v === 0) { v = 0; send(0); return; }
    const from = v, t0 = performance.now();
    const step = (now: number) => {
      const k = Math.min(1, (now - t0) / 120);
      v = from * (1 - k * k * (3 - 2 * k)); // smoothstep
      send(v);
      if (k < 1) springRaf = requestAnimationFrame(step); else { v = 0; send(0); }
    };
    springRaf = requestAnimationFrame(step);
  }
  onDestroy(() => { cancelAnimationFrame(springRaf); clearTimeout(timer); });

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    cancelAnimationFrame(springRaf);
    const el = e.currentTarget as HTMLElement;
    el.focus();
    el.setPointerCapture(e.pointerId);
    const at = (ev: PointerEvent) => {
      const r = el.getBoundingClientRect();
      const t = Math.min(1, Math.max(0, (ev.clientY - r.top - 6) / (r.height - 12)));
      set(spring ? 1 - 2 * t : 1 - t);
    };
    at(e);
    const up = () => {
      el.removeEventListener('pointermove', at); el.removeEventListener('pointerup', up); el.removeEventListener('pointercancel', up);
      release();
    };
    el.addEventListener('pointermove', at);
    el.addEventListener('pointerup', up);
    el.addEventListener('pointercancel', up);
  }
  function key(e: KeyboardEvent) {
    const d = ({ ArrowUp: 0.1, ArrowDown: -0.1 } as Record<string, number>)[e.key];
    if (d == null) return;
    e.preventDefault(); e.stopPropagation();
    cancelAnimationFrame(springRaf);
    set(v + d);
  }
  const keyup = (e: KeyboardEvent) => { if (spring && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) release(); };
  const text = $derived(spring ? `${v < 0 ? '−' : '+'}${Math.abs(v * 2).toFixed(1)} st` : `MOD ${Math.round(v * 100)} %`);
</script>

<div
  class="wheel screen"
  class:learning={isLearning(learn, learnId)}
  class:learned={isLearned(learn, learnId)}
  data-learn={learnId}
  use:learnable={{ ctx: learn, id: learnId, reset: spring ? undefined : () => set(0) }}
  role="slider"
  tabindex="0"
  aria-label={label}
  aria-valuemin={lo}
  aria-valuemax={1}
  aria-valuenow={Number(v.toFixed(2))}
  aria-valuetext={text}
  title={label}
  style:--v={spring ? (1 - v) / 2 : 1 - v}
  onpointerdown={down}
  onkeydown={key}
  onkeyup={keyup}
>
  <span class="thumb"></span>
</div>

<style>
  .wheel { width: 16px; height: var(--kh); position: relative; cursor: ns-resize; touch-action: none; }
  .thumb { position: absolute; left: 1px; right: 1px; height: 12px; top: calc(var(--v) * (100% - 12px)); border-radius: 3px; background: var(--glass); box-shadow: inset 0 0 0 1px var(--vec); pointer-events: none; }
</style>
