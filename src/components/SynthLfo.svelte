<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import ParamKnob from './ParamKnob.svelte';
  import ParamKey from './ParamKey.svelte';
  import ChoiceLcd from './ChoiceLcd.svelte';
  import { SYNTH_META } from '../lib/synth-meta.ts';
  import type { SynthParams, SourceId } from '../lib/synth-types.ts';
  let { n, params, live, knob, onparam, mod, onstart }: { n: 1 | 2; params: SynthParams; live: number[] | null; knob: number; onparam: (id: string, value: number) => void; mod?: ComponentProps<typeof ParamKnob>['mod']; onstart: (source: SourceId, e: PointerEvent) => void } = $props();
  const prefix = $derived(`lfo${n}`);
</script>
<section class="syn-sec" aria-label="LFO{n}">
  <header class="syn-title"><button class="silk" onpointerdown={(e) => onstart(`lfo${n}` as SourceId, e)}>LFO{n}</button></header>
  <div class="syn-disp screen" aria-hidden="true"><canvas></canvas></div>
  <div class="syn-ctl">
    <ChoiceLcd id="{prefix}.shape" {params} {onparam}/>
    <ParamKnob {mod} id="{prefix}.{params.lfo[n-1].sync ? 'syncDiv' : 'rate'}" label="RATE" {params} {onparam} size={knob} live={params.lfo[n-1].sync ? null : live?.[SYNTH_META.dests.findIndex(d => d.id === `${prefix}.rate`)] ?? null}/>
    <ParamKey id="{prefix}.sync" {params} {onparam}/><ChoiceLcd id="{prefix}.mode" {params} {onparam}/>
    <ParamKnob {mod} id="{prefix}.delay" {params} {onparam} size={knob}/><ParamKnob {mod} id="{prefix}.phase" {params} {onparam} size={knob}/><ParamKey id="{prefix}.polarity" {params} {onparam}/>
  </div>
</section>
