<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import ParamKnob from './ParamKnob.svelte';
  import ParamStep from './ParamStep.svelte';
  import ParamKey from './ParamKey.svelte';
  import ChoiceLcd from './ChoiceLcd.svelte';
  import { SYNTH_META } from '../lib/synth-meta.ts';
  import type { SynthParams } from '../lib/synth-types.ts';
  let { x, params, live, knob, onparam, mod }: { x: 'A' | 'B'; params: SynthParams; live: number[] | null; knob: number; onparam: (id: string, value: number) => void; mod?: ComponentProps<typeof ParamKnob>['mod'] } = $props();
  const prefix = $derived(`osc${x}`);
</script>
<section class="syn-sec" aria-label="OSC {x}">
  <header class="syn-title"><span class="silk">OSC {x}</span><ParamKey id="{prefix}.on" {params} {onparam}/></header>
  <div class="syn-disp screen" aria-hidden="true"><canvas></canvas></div>
  <div class="syn-ctl">
    <ChoiceLcd id="{prefix}.table" {params} {onparam}/>
    <ParamKnob {mod} id="{prefix}.pos" {params} {onparam} size={knob * 1.2} live={live?.[SYNTH_META.dests.findIndex(d => d.id === `${prefix}.pos`)] ?? null}/>
    <ChoiceLcd id="{prefix}.warp" {params} {onparam}/>
    <ParamKnob {mod} id="{prefix}.warpAmt" {params} {onparam} size={knob} live={live?.[SYNTH_META.dests.findIndex(d => d.id === `${prefix}.warpAmt`)] ?? null}/>
    <ParamStep id="{prefix}.unison" {params} {onparam}/>
    {#each ['detune','blend','width','phaseRand'] as field}<ParamKnob {mod} id="{prefix}.{field}" {params} {onparam} size={knob} live={live?.[SYNTH_META.dests.findIndex(d => d.id === `${prefix}.${field}`)] ?? null}/>{/each}
    <ParamStep id="{prefix}.oct" {params} {onparam}/><ParamStep id="{prefix}.semi" {params} {onparam}/>
    {#each ['fine','level','pan'] as field}<ParamKnob {mod} id="{prefix}.{field}" {params} {onparam} size={knob} live={live?.[SYNTH_META.dests.findIndex(d => d.id === `${prefix}.${field}`)] ?? null}/>{/each}
    <ParamKey id="{prefix}.toFilter" {params} {onparam}/>
  </div>
</section>
