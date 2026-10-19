<script lang="ts">
  import { getContext } from 'svelte';
  import { isLearned, isLearning, learnable, LEARN, type LearnCtx } from '../lib/learnable.ts';
  // Channel fader: recessed tray, light slot rail, two-face cap; level fill(s) beside the slot.
  import { dbToPos, fmtDb, posToDb } from '../lib/mix.ts';

  let {
    label, value, def = 0, height, color = '#5d6779', meters = [], learnId, onchange,
  }: {
    label: string; value: number; def?: number; height: number; color?: string;
    /** MIDI-learnable parameter id (right-click → Learn MIDI CC / Reset). */
    learnId?: string;
    /** One entry per level fill: a mixer channel index for activity fills, or 'L' / 'R' for master. */
    meters?: (number | 'L' | 'R')[];
    onchange: (db: number) => void;
  } = $props();

  const learn = getContext<LearnCtx | undefined>(LEARN);
  const CAP = 28;
  const pos = $derived(dbToPos(value));
  const apply = (p: number) => {
    const db = posToDb(Math.min(1, Math.max(0, p)));
    if (db !== value) onchange(db);
  };

  function down(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    const el = event.currentTarget as HTMLElement;
    el.focus();
    el.setPointerCapture(event.pointerId);
    let last = event.clientY, p = pos;
    const move = (e: PointerEvent) => {
      p = Math.min(1, Math.max(0, p + ((last - e.clientY) / (height - CAP)) * (e.shiftKey ? 0.2 : 1)));
      last = e.clientY;
      apply(p);
    };
    const up = () => { el.removeEventListener('pointermove', move); el.removeEventListener('pointerup', up); };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerup', up);
  }
  function key(event: KeyboardEvent) {
    const s = event.shiftKey ? 0.004 : 0.02;
    const d = ({ ArrowUp: s, ArrowRight: s, ArrowDown: -s, ArrowLeft: -s } as Record<string, number>)[event.key];
    if (d != null) { event.preventDefault(); event.stopPropagation(); apply(pos + d); }
  }
</script>

<div
  class="fader"
  class:learning={isLearning(learn, learnId)}
  class:learned={isLearned(learn, learnId)}
  data-learn={learnId}
  use:learnable={{ ctx: learn, id: learnId, reset: () => apply(dbToPos(def)) }}
  role="slider"
  tabindex="0"
  aria-label={label}
  aria-valuemin={0}
  aria-valuemax={1}
  aria-valuenow={Number(pos.toFixed(3))}
  aria-valuetext={fmtDb(value)}
  style:height="{height}px"
  style:--fc={color}
  onpointerdown={down}
  onwheel={(e) => { e.preventDefault(); apply(pos + (e.deltaY < 0 ? 0.02 : -0.02)); }}
  ondblclick={() => apply(dbToPos(def))}
  onkeydown={key}
>
  <div class="f-slot"></div>
  {#each meters as m, j}
    <i class="f-fill" class:m0={meters.length > 1 && j === 0} data-group={typeof m === 'number' ? m : undefined} data-ch={typeof m === 'string' ? m : undefined}></i>
  {/each}
  <div class="f-cap" style:top="{(1 - pos) * (height - CAP)}px"></div>
</div>

<style>
  .fader {
    position: relative; width: var(--fw, 36px); cursor: ns-resize; touch-action: none; border-radius: 5px;
    background: linear-gradient(#1d1a18, #121010);
    box-shadow: inset 0 0 0 2px #070606, inset 0 3px 6px rgba(0,0,0,.7), 0 1px 0 rgba(255,255,255,.08);
  }
  .f-slot { position: absolute; left: calc(50% - 2px); top: 12px; bottom: 12px; width: 4px; border-radius: 2px; background: linear-gradient(90deg, #9e978b, #d8d2c6 50%, #9e978b); box-shadow: 0 0 0 1px #070606; }
  .f-cap {
    position: absolute; left: 4px; right: 4px; height: 28px; border-radius: 4px; border: 2px solid #161412; box-sizing: border-box; pointer-events: none;
    background: linear-gradient(180deg, color-mix(in srgb, var(--fc) 50%, #fff) 0 34%, #161412 34% calc(34% + 2px), var(--fc) calc(34% + 2px) 72%, color-mix(in srgb, var(--fc) 78%, #000) 100%);
    box-shadow: inset 0 2px 0 rgba(255,255,255,.4), inset 2px 0 0 rgba(255,255,255,.12), 3px 5px 0 rgba(0,0,0,.38), 0 9px 12px rgba(0,0,0,.35);
  }
  .fader:focus-visible .f-cap { box-shadow: inset 0 2px 0 rgba(255,255,255,.4), 0 0 0 2px var(--focus), 3px 5px 0 rgba(0,0,0,.38); }
  .f-fill { position: absolute; top: 12px; bottom: 12px; left: calc(50% + 5px); width: 3px; pointer-events: none; background: linear-gradient(0deg, var(--ok) 0 72%, var(--warn) 72% 90%, var(--error) 90%); clip-path: inset(calc(100% - var(--m, 0) * 100%) 0 0 0); }
  .f-fill.m0 { left: calc(50% - 8px); }
</style>
