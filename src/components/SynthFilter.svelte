<script lang="ts">
  import ParamKnob from './ParamKnob.svelte';
  import ParamKey from './ParamKey.svelte';
  import ChoiceLcd from './ChoiceLcd.svelte';
  import { SYNTH_META } from '../lib/synth-meta.ts';
  import type { SynthParams } from '../lib/synth-types.ts';
  let { params, live, knob, onparam }: { params: SynthParams; live: number[] | null; knob: number; onparam: (id: string, value: number) => void } = $props();
</script>
<section class="syn-sec" aria-label="FILTER">
  <header class="syn-title"><span class="silk">FILTER</span><ParamKey id="filter.on" {params} {onparam}/></header>
  <div class="syn-disp screen" aria-hidden="true"><canvas></canvas></div>
  <div class="syn-ctl">
    <ChoiceLcd id="filter.type" {params} {onparam}/>
    {#each ['cutoff','reso','drive','keyTrack','envAmt'] as field}<ParamKnob id="filter.{field}" {params} {onparam} size={knob * (field === 'cutoff' ? 1.2 : 1)} live={live?.[SYNTH_META.dests.findIndex(d => d.id === `filter.${field}`)] ?? null}/>{/each}
  </div>
</section>
