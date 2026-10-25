<script lang="ts">
  import { getContext } from 'svelte';
  import { meta, getParam, toNorm, fromNorm, fmtParam, lcxlTag } from '../lib/synth.ts';
  import { LEARN, learnable, isLearning, isLearned, type LearnCtx } from '../lib/learnable.ts';
  import type { SynthParams } from '../lib/synth-types.ts';
  let { id, params, height, label, onparam }: { id: string; params: SynthParams; height: number; label?: string; onparam: (id: string, value: number) => void } = $props();
  const learn = getContext<LearnCtx | undefined>(LEARN);
  const m = $derived(meta(id));
  const value = $derived(getParam(params, id));
  const pos = $derived(toNorm(m, value));
  const learnId = $derived(`synth.${id}`);
  const tagTitle = $derived(lcxlTag(id));
  const apply = (n: number) => onparam(id, fromNorm(m, n));
  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault(); const el = e.currentTarget as HTMLElement; el.focus(); el.setPointerCapture(e.pointerId);
    let last = e.clientY, n = pos;
    const move = (event: PointerEvent) => { n = Math.min(1, Math.max(0, n + (last - event.clientY) / Math.max(1, height - 22) * (event.shiftKey ? 0.1 : 1))); last = event.clientY; apply(n); };
    const up = () => { el.removeEventListener('pointermove', move); el.removeEventListener('pointerup', up); el.removeEventListener('pointercancel', up); };
    el.addEventListener('pointermove', move); el.addEventListener('pointerup', up); el.addEventListener('pointercancel', up);
  }
  function key(e: KeyboardEvent) {
    const step = e.shiftKey ? 0.001 : 0.01;
    const delta = ({ ArrowUp: step, ArrowRight: step, ArrowDown: -step, ArrowLeft: -step } as Record<string, number>)[e.key];
    if (delta !== undefined || e.key === 'Home' || e.key === 'End') { e.preventDefault(); e.stopPropagation(); apply(e.key === 'Home' ? 0 : e.key === 'End' ? 1 : pos + delta); }
  }
</script>
<div class="param-fader">
  <span class="silk">{label ?? m.label}</span>
  {#if tagTitle}<span class="xl-tag" title={tagTitle}>XL</span>{/if}
  <div class="fader" class:learning={isLearning(learn, learnId)} class:learned={isLearned(learn, learnId)} data-learn={learnId} use:learnable={{ ctx: learn, id: learnId, reset: () => onparam(id, m.default) }} role="slider" tabindex="0" aria-label="{m.section} {m.label}" aria-valuemin={0} aria-valuemax={1} aria-valuenow={pos} aria-valuetext={fmtParam(id, value)} style:height="{height}px" onpointerdown={down} onkeydown={key} onwheel={(e) => { e.preventDefault(); apply(pos + (e.deltaY < 0 ? 1 : -1) * (e.shiftKey ? 0.001 : 0.01)); }} ondblclick={() => onparam(id, m.default)}>
    <div class="f-slot"></div><div class="f-cap" style:top="{(1 - pos) * (height - 22)}px"></div>
  </div>
  <span class="lcd">{fmtParam(id, value)}</span>
</div>
<style>
  .param-fader { position: relative; display: flex; flex-direction: column; align-items: center; gap: 4px; min-width: 48px; }
  .xl-tag { position: absolute; right: 0; top: 0; font: 600 8px var(--f-silk); color: var(--vec); }
  .param-fader > .lcd { font-size: 11px; color: var(--lcd-dim); white-space: nowrap; }
  .fader { position: relative; width: 20px; cursor: ns-resize; touch-action: none; border-radius: 5px; background: linear-gradient(#1d1a18, #121010); box-shadow: inset 0 0 0 2px #070606, inset 0 3px 6px rgba(0,0,0,.7), 0 1px 0 rgba(255,255,255,.08); }
  .f-slot { position: absolute; left: calc(50% - 2px); top: 10px; bottom: 10px; width: 4px; border-radius: 2px; background: linear-gradient(90deg, #9e978b, #d8d2c6 50%, #9e978b); box-shadow: 0 0 0 1px #070606; }
  .f-cap { position: absolute; left: 3px; right: 3px; height: 22px; border-radius: 4px; border: 2px solid #161412; box-sizing: border-box; pointer-events: none; background: linear-gradient(180deg, #c8c1b5 0 34%, #161412 34% calc(34% + 2px), #8b8276 calc(34% + 2px) 72%, #5f574e 100%); box-shadow: inset 0 2px 0 rgba(255,255,255,.4), 3px 5px 0 rgba(0,0,0,.38), 0 9px 12px rgba(0,0,0,.35); }
  .fader:focus-visible .f-cap { outline: 2px solid var(--focus); }
</style>
