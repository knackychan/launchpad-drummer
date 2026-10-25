<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import ParamKnob from './ParamKnob.svelte';
  import ParamStep from './ParamStep.svelte';
  import ParamKey from './ParamKey.svelte';
  import ChoiceLcd from './ChoiceLcd.svelte';
  import { SYNTH_META } from '../lib/synth-meta.ts';
  import type { SynthParams } from '../lib/synth-types.ts';
  let { params, live, knob, onparam, mod }: { params: SynthParams; live: number[] | null; knob: number; onparam: (id: string, value: number) => void; mod?: ComponentProps<typeof ParamKnob>['mod'] } = $props();
</script>
<section class="syn-sec" aria-label="SUB · NOISE">
  <header class="syn-title"><span class="silk">SUB · NOISE</span></header>
  <div class="syn-ctl">
    <ParamKey id="sub.on" label="SUB" {params} {onparam}/><ChoiceLcd id="sub.shape" {params} {onparam}/><ParamStep id="sub.oct" {params} {onparam}/>
    <ParamKnob {mod} id="sub.level" {params} {onparam} size={knob} live={live?.[SYNTH_META.dests.findIndex(d => d.id === 'sub.level')] ?? null}/><ParamKey id="sub.toFilter" {params} {onparam}/>
    <ParamKey id="noise.on" label="NOISE" {params} {onparam}/><ChoiceLcd id="noise.color" {params} {onparam}/>
    <ParamKnob {mod} id="noise.level" {params} {onparam} size={knob} live={live?.[SYNTH_META.dests.findIndex(d => d.id === 'noise.level')] ?? null}/><ParamKey id="noise.toFilter" {params} {onparam}/>
  </div>
</section>
