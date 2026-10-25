<script lang="ts">
  import ParamKnob from './ParamKnob.svelte';
  import ParamStep from './ParamStep.svelte';
  import ChoiceLcd from './ChoiceLcd.svelte';
  import { SYNTH_META } from '../lib/synth-meta.ts';
  import type { SynthParams } from '../lib/synth-types.ts';
  let { params, live, knob, onparam }: { params: SynthParams; live: number[] | null; knob: number; onparam: (id: string, value: number) => void } = $props();
</script>
<div class="syn-row" style:grid-template-rows="auto minmax(0,1fr)">
  <section class="syn-sec" aria-label="AMP">
    <header class="syn-title"><span class="silk">AMP</span></header>
    <div class="syn-ctl">{#each ['level','velSens'] as field}<ParamKnob id="amp.{field}" {params} {onparam} size={knob} live={live?.[SYNTH_META.dests.findIndex(d => d.id === `amp.${field}`)] ?? null}/>{/each}</div>
  </section>
  <section class="syn-sec" aria-label="VOICE">
    <header class="syn-title"><span class="silk">VOICE</span></header>
    <div class="syn-ctl">
      <ChoiceLcd id="voice.mode" {params} {onparam}/><ParamKnob id="voice.glide" {params} {onparam} size={knob}/><ChoiceLcd id="voice.glideMode" {params} {onparam}/><ParamStep id="voice.bendRange" {params} {onparam}/>
      <ParamKnob id="voice.drift" {params} {onparam} size={knob}/><ParamKnob id="voice.volumeDb" {params} {onparam} size={knob}/>
    </div>
  </section>
</div>
