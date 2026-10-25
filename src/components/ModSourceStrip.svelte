<script lang="ts">
  import { SYNTH_META } from '../lib/synth-meta.ts';
  import type { SourceId } from '../lib/synth-types.ts';
  let { meters, onstart }: { meters: number[] | null; onstart: (source: SourceId, e: PointerEvent) => void } = $props();
  function meter(id: SourceId, i: number) {
    const v = meters?.[i];
    return v === undefined ? 0 : Math.min(1, Math.max(0, ['lfo1','lfo2','note','rand','bend'].includes(id) ? (v+1)/2 : v));
  }
</script>
<div class="mod-strip" role="toolbar" aria-label="Modulation sources">
  {#each SYNTH_META.sources as source, i}<button class="mod-badge" style:--c={source.color} title="{source.label} — drag onto a knob to modulate it" onpointerdown={(e) => { if (e.button === 0) onstart(source.id, e); }}><span class="grip" aria-hidden="true">≡</span>{source.label}<i class="mod-meter" style:--m={meter(source.id, i)}></i></button>{/each}
</div>
<style>
  .mod-strip { display: flex; gap: 4px; flex-wrap: wrap; }
  .mod-badge { display: inline-flex; align-items: center; gap: 4px; height: 20px; padding: 0 6px; border: 0; border-radius: 3px; background: var(--glass); box-shadow: inset 0 0 0 1px var(--c); color: var(--c); font: 600 9.5px/1 var(--f-silk); letter-spacing: .1em; cursor: grab; position: relative; touch-action: none; }
  .mod-meter { position: absolute; left: 3px; right: 3px; bottom: 2px; height: 2px; background: var(--c); transform-origin: left; transform: scaleX(var(--m, 0)); opacity: .8; pointer-events: none; }
</style>
