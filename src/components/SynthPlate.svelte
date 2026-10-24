<script lang="ts">
  import type { SetModel } from '../lib/model.ts';
  import { SYNTH_PAGES, synthKnob, synthPageSet } from '../lib/layout.ts';
  let { set, width, height, engine, onusesynth }: {
    set: SetModel; width: number; height: number; engine: string; onusesynth: () => void;
  } = $props();
  const pageSet = $derived(synthPageSet(width, height));
  let chosen = $state<Record<string, string>>({});
  $effect(() => {
    const kind = pageSet;
    let page = SYNTH_PAGES[kind][0];
    try { const saved = localStorage.getItem(`lpd.synth.page.${kind}`); if (saved && SYNTH_PAGES[kind].includes(saved)) page = saved; } catch { /* pages work without storage */ }
    chosen[kind] = page;
  });
  const page = $derived(chosen[pageSet] ?? SYNTH_PAGES[pageSet][0]);
  function choose(next: string) {
    chosen[pageSet] = next;
    try { localStorage.setItem(`lpd.synth.page.${pageSet}`, next); } catch { /* pages work without storage */ }
  }
</script>
<section class="synth plate" data-label="SYNTH" aria-label="Synth" style:width="{width}px" style:height="{height}px" style:--kn="{synthKnob(height, pageSet)}px">
  <header class="syn-head">
    <div class="syn-head-row">
      <div class="lcd screen syn-patch">{set.keys.patch.name}{set.keys.modified ? ' *' : ''}</div>
      <div class="switch" role="tablist" aria-label="Synth pages">
        {#each SYNTH_PAGES[pageSet] as name}<button class="key small" class:on={name === page} role="tab" aria-selected={name === page} onclick={() => choose(name)}>{name}</button>{/each}
      </div>
    </div>
    <div class="syn-head-row"></div>
  </header>
  <div class="syn-body" role="tabpanel" aria-label={page} inert={engine === 'soundfont'}><span class="lcd-dim">{page}</span></div>
  {#if engine === 'soundfont'}<div class="syn-dim" role="note"><span class="lcd">SOUNDFONT ACTIVE</span><button class="key small" onclick={onusesynth}>USE SYNTH</button></div>{/if}
</section>
<style>
  .synth { flex: none; box-sizing: border-box; display: flex; flex-direction: column; gap: 6px; padding: 10px 10px 8px; position: relative; min-width: 0; }
  .syn-head { flex: none; display: flex; flex-direction: column; gap: 4px; }
  .syn-head-row { display: flex; align-items: center; gap: 6px; height: 26px; }
  .syn-patch { min-width: 120px; max-width: 260px; flex: 1; height: 24px; line-height: 24px; padding: 0 8px; font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; cursor: pointer; }
  .syn-body { flex: 1; min-height: 0; position: relative; }
  .syn-dim { position: absolute; inset: 0; z-index: 2; display: flex; align-items: center; justify-content: center; gap: 12px; background: rgba(0,0,0,.66); }
</style>
