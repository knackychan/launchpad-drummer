<script lang="ts">
  import ParamKnob from './ParamKnob.svelte';
  import ParamKey from './ParamKey.svelte';
  import ChoiceLcd from './ChoiceLcd.svelte';
  import { SYNTH_META } from '../lib/synth-meta.ts';
  import type { SynthParams } from '../lib/synth-types.ts';
  let { n, params, live, knob, onparam }: { n: 1 | 2; params: SynthParams; live: number[] | null; knob: number; onparam: (id: string, value: number) => void } = $props();
  const prefix = $derived(`lfo${n}`);
</script>
<section class="syn-sec" aria-label="LFO{n}">
  <header class="syn-title"><span class="silk">LFO{n}</span></header>
  <div class="syn-disp screen" aria-hidden="true"><canvas></canvas></div>
  <div class="syn-ctl">
    <ChoiceLcd id="{prefix}.shape" {params} {onparam}/>
    <ParamKnob id="{prefix}.{params.lfo[n-1].sync ? 'syncDiv' : 'rate'}" label="RATE" {params} {onparam} size={knob} live={params.lfo[n-1].sync ? null : live?.[SYNTH_META.dests.findIndex(d => d.id === `${prefix}.rate`)] ?? null}/>
    <ParamKey id="{prefix}.sync" {params} {onparam}/><ChoiceLcd id="{prefix}.mode" {params} {onparam}/>
    <ParamKnob id="{prefix}.delay" {params} {onparam} size={knob}/><ParamKnob id="{prefix}.phase" {params} {onparam} size={knob}/><ParamKey id="{prefix}.polarity" {params} {onparam}/>
  </div>
</section>
