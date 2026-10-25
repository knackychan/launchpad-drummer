<script lang="ts">
  import ParamFader from './ParamFader.svelte';
  import { display } from '../lib/synthDisplay.ts';
  import { drawEnv } from '../lib/synthDraw.ts';
  import { synthFeed, synthLive } from '../lib/synthFeed.ts';
  import { sourceColor } from '../lib/synth.ts';
  import type { SynthParams, SourceId } from '../lib/synth-types.ts';
  let { n, params, live, knob, onparam, onstart }: { n: 1 | 2 | 3; params: SynthParams; live: number[] | null; knob: number; onparam: (id: string, value: number) => void; onstart: (source: SourceId, e: PointerEvent) => void } = $props();
</script>
<section class="syn-sec" aria-label="ENV{n}">
  <header class="syn-title"><button class="silk" onpointerdown={(e) => onstart(`env${n}` as SourceId, e)}>ENV{n}{n === 1 ? ' · AMP' : n === 2 ? ' · FILTER' : ''}</button></header>
  <div class="syn-disp screen" aria-hidden="true"><canvas use:display={{ draw: (g,w,h) => drawEnv(g,w,h,params.env[n-1],synthLive()?synthFeed.frame?.env[n-1]??null:null,sourceColor(`env${n}` as SourceId)), key: JSON.stringify(params.env[n-1]) }}></canvas></div>
  <div class="syn-ctl">{#each ['attack','decay','sustain','release'] as field, i}<ParamFader id="env{n}.{field}" label={['A','D','S','R'][i]} {params} {onparam} height={knob}/>{/each}</div>
</section>
