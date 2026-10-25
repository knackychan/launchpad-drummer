<script lang="ts">
  import type { SetModel } from '../lib/model.ts';
  import { SYNTH_PAGES, synthKnob, synthPageSet } from '../lib/layout.ts';
  import SynthOsc from './SynthOsc.svelte';
  import SynthSubNoise from './SynthSubNoise.svelte';
  import SynthFilter from './SynthFilter.svelte';
  import SynthAmpVoice from './SynthAmpVoice.svelte';
  import SynthEnv from './SynthEnv.svelte';
  import SynthLfo from './SynthLfo.svelte';
  import SynthFx from './SynthFx.svelte';
  let { set, width, height, engine, onusesynth, onparam }: {
    set: SetModel; width: number; height: number; engine: string; onusesynth: () => void;
    onparam: (id: string, value: number) => void;
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
  const knob = $derived(Math.min(synthKnob(height, pageSet), page === 'MAIN' || page === 'OSC' || page === 'FILTER' || (page === 'FX' && pageSet === 'compact') ? 24 : 44));
  const sectionProps = $derived({ params: set.keys.patch.params, live: null, knob, onparam });
  function choose(next: string) {
    chosen[pageSet] = next;
    try { localStorage.setItem(`lpd.synth.page.${pageSet}`, next); } catch { /* pages work without storage */ }
  }
</script>
<section class="synth plate" data-label="SYNTH" aria-label="Synth" style:width="{width}px" style:height="{height}px" style:--kn="{knob}px">
  <header class="syn-head">
    <div class="syn-head-row">
      <div class="lcd screen syn-patch">{set.keys.patch.name}{set.keys.modified ? ' *' : ''}</div>
      <div class="switch" role="tablist" aria-label="Synth pages">
        {#each SYNTH_PAGES[pageSet] as name}<button class="key small" class:on={name === page} role="tab" aria-selected={name === page} onclick={() => choose(name)}>{name}</button>{/each}
      </div>
    </div>
    <div class="syn-head-row"></div>
  </header>
  <div class="syn-body" role="tabpanel" aria-label={page} inert={engine === 'soundfont'}>
    {#if page === 'MAIN'}
      <div class="pg-main">
        <div class="syn-row" style:grid-template-columns="1fr 1fr .6fr"><SynthOsc x="A" {...sectionProps}/><SynthOsc x="B" {...sectionProps}/><SynthSubNoise {...sectionProps}/></div>
        <div class="syn-row" style:grid-template-columns="1.4fr .5fr 1fr"><SynthFilter {...sectionProps}/><SynthAmpVoice {...sectionProps}/></div>
        <div class="syn-row" style:grid-template-columns="repeat(3,.7fr) 1fr 1fr"><SynthEnv n={1} {...sectionProps}/><SynthEnv n={2} {...sectionProps}/><SynthEnv n={3} {...sectionProps}/><SynthLfo n={1} {...sectionProps}/><SynthLfo n={2} {...sectionProps}/></div>
      </div>
    {:else if page === 'OSC'}
      <div class="pg-osc" class:compact={pageSet === 'compact'}>
        <div class="syn-row" style:grid-template-columns="1fr 1fr"><SynthOsc x="A" {...sectionProps}/><SynthOsc x="B" {...sectionProps}/></div>
        {#if pageSet !== 'compact'}<div class="syn-row" style:grid-template-columns=".7fr 1.3fr 1fr"><SynthSubNoise {...sectionProps}/><SynthFilter {...sectionProps}/><SynthAmpVoice {...sectionProps}/></div>{/if}
      </div>
    {:else if page === 'MOD'}
      <div class="pg-mod">
        <div class="syn-row" style:grid-template-columns="1fr 1fr 1fr"><SynthEnv n={1} {...sectionProps}/><SynthEnv n={2} {...sectionProps}/><SynthEnv n={3} {...sectionProps}/></div>
        <div class="syn-row" style:grid-template-columns="1fr 1fr"><SynthLfo n={1} {...sectionProps}/><SynthLfo n={2} {...sectionProps}/></div>
      </div>
    {:else if page === 'FILTER'}
      <div class="syn-row" style:height="100%" style:grid-template-columns=".7fr 1.3fr 1fr"><SynthSubNoise {...sectionProps}/><SynthFilter {...sectionProps}/><SynthAmpVoice {...sectionProps}/></div>
    {:else if page === 'ENV'}
      <div class="syn-row" style:height="100%" style:grid-template-columns="1fr 1fr 1fr"><SynthEnv n={1} {...sectionProps}/><SynthEnv n={2} {...sectionProps}/><SynthEnv n={3} {...sectionProps}/></div>
    {:else if page === 'LFO'}
      <div class="syn-row" style:height="100%" style:grid-template-columns="1fr 1fr"><SynthLfo n={1} {...sectionProps}/><SynthLfo n={2} {...sectionProps}/></div>
    {:else if page === 'FX'}
      <div class="pg-fx" class:wide={pageSet === 'wide'}>{#each ['drive','chorus','delay','reverb'] as unit}<SynthFx unit={unit as 'drive' | 'chorus' | 'delay' | 'reverb'} {...sectionProps}/>{/each}</div>
    {:else}
      <span class="lcd-dim">Routes appear here (Task 16)</span>
    {/if}
  </div>
  {#if engine === 'soundfont'}<div class="syn-dim" role="note"><span class="lcd">SOUNDFONT ACTIVE</span><button class="key small" onclick={onusesynth}>USE SYNTH</button></div>{/if}
</section>
<style>
  .pg-main { display: grid; grid-template-rows: 1.15fr 1fr 1fr; gap: 6px; height: 100%; }
  .pg-main > .syn-row:nth-child(2) > :global(.syn-row) { display: contents; }
  .pg-osc, .pg-mod { display: grid; grid-template-rows: 1fr 1fr; gap: 6px; height: 100%; }
  .pg-osc.compact { grid-template-rows: 1fr; }
  .pg-fx { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); grid-template-rows: 1fr 1fr; gap: 6px; height: 100%; }
  .pg-fx.wide { grid-template-columns: repeat(4,minmax(0,1fr)); grid-template-rows: 1fr; }
  .synth { flex: none; box-sizing: border-box; display: flex; flex-direction: column; gap: 6px; padding: 10px 10px 8px; position: relative; min-width: 0; }
  .syn-head { flex: none; display: flex; flex-direction: column; gap: 4px; }
  .syn-head-row { display: flex; align-items: center; gap: 6px; height: 26px; }
  .syn-patch { min-width: 120px; max-width: 260px; flex: 1; height: 24px; line-height: 24px; padding: 0 8px; font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; cursor: pointer; }
  .syn-body { flex: 1; min-height: 0; position: relative; }
  .syn-dim { position: absolute; inset: 0; z-index: 2; display: flex; align-items: center; justify-content: center; gap: 12px; background: rgba(0,0,0,.66); }
</style>
