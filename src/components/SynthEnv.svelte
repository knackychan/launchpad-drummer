<script lang="ts">
  import ParamFader from './ParamFader.svelte';
  import type { SynthParams } from '../lib/synth-types.ts';
  let { n, params, live, knob, onparam }: { n: 1 | 2 | 3; params: SynthParams; live: number[] | null; knob: number; onparam: (id: string, value: number) => void } = $props();
</script>
<section class="syn-sec" aria-label="ENV{n}">
  <header class="syn-title"><span class="silk">ENV{n}{n === 1 ? ' · AMP' : n === 2 ? ' · FILTER' : ''}</span></header>
  <div class="syn-disp screen" aria-hidden="true"><canvas></canvas></div>
  <div class="syn-ctl">{#each ['attack','decay','sustain','release'] as field, i}<ParamFader id="env{n}.{field}" label={['A','D','S','R'][i]} {params} {onparam} height={knob}/>{/each}</div>
</section>
