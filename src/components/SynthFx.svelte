<script lang="ts">
  import ParamKnob from './ParamKnob.svelte';
  import ParamKey from './ParamKey.svelte';
  import ChoiceLcd from './ChoiceLcd.svelte';
  import type { SynthParams } from '../lib/synth-types.ts';
  let { unit, params, live, knob, onparam }: { unit: 'drive' | 'chorus' | 'delay' | 'reverb'; params: SynthParams; live: number[] | null; knob: number; onparam: (id: string, value: number) => void } = $props();
</script>
<section class="syn-sec" aria-label={unit.toUpperCase()}>
  <header class="syn-title"><span class="silk">{unit.toUpperCase()}</span><ParamKey id="fx.{unit}.on" {params} {onparam}/></header>
  <div class="syn-ctl">
    {#if unit === 'drive'}
      <ChoiceLcd id="fx.drive.type" {params} {onparam}/>{#each ['amount','tone','mix'] as field}<ParamKnob id="fx.drive.{field}" {params} {onparam} size={knob}/>{/each}
    {:else if unit === 'chorus'}
      {#each ['rate','depth','delay','feedback','mix'] as field}<ParamKnob id="fx.chorus.{field}" {params} {onparam} size={knob}/>{/each}
    {:else if unit === 'delay'}
      <ParamKnob id="fx.delay.{params.fx.delay.sync ? 'syncDiv' : 'time'}" label="TIME" {params} {onparam} size={knob}/><ParamKey id="fx.delay.sync" {params} {onparam}/><ParamKnob id="fx.delay.feedback" {params} {onparam} size={knob}/><ParamKey id="fx.delay.pingPong" {params} {onparam}/><ParamKnob id="fx.delay.tone" {params} {onparam} size={knob}/><ParamKnob id="fx.delay.mix" {params} {onparam} size={knob}/>
    {:else}
      {#each ['size','decay','damp','preDelay','mix'] as field}<ParamKnob id="fx.reverb.{field}" {params} {onparam} size={knob}/>{/each}
    {/if}
  </div>
</section>
