<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import ParamKnob from './ParamKnob.svelte';
  import ParamKey from './ParamKey.svelte';
  import ChoiceLcd from './ChoiceLcd.svelte';
  import { SYNTH_META } from '../lib/synth-meta.ts';
  import { display } from '../lib/synthDisplay.ts';
  import { drawFilter, xToFreq } from '../lib/synthDraw.ts';
  import { synthFeed, synthLive } from '../lib/synthFeed.ts';
  import { throttle } from '../lib/synth.ts';
  import type { SynthParams } from '../lib/synth-types.ts';
  let { params, live, knob, onparam, mod }: { params: SynthParams; live: number[] | null; knob: number; onparam: (id: string, value: number) => void; mod?: ComponentProps<typeof ParamKnob>['mod'] } = $props();
  function drag(node: HTMLCanvasElement) {
    let release = () => {};
    const send = throttle((cutoff:number,reso:number) => { onparam('filter.cutoff',cutoff);onparam('filter.reso',reso); },16);
    const down = (e:PointerEvent) => {
      if(e.button!==0)return;e.preventDefault();release();node.setPointerCapture(e.pointerId);
      const move = (event:PointerEvent) => {const r=node.getBoundingClientRect();send(xToFreq(Math.min(r.width,Math.max(0,event.clientX-r.left)),r.width),Math.min(1,Math.max(0,1-(event.clientY-r.top)/r.height)));};
      const up = () => {send.flush();release();};
      release = () => {node.removeEventListener('pointermove',move);node.removeEventListener('pointerup',up);node.removeEventListener('pointercancel',up);if(node.hasPointerCapture(e.pointerId))node.releasePointerCapture(e.pointerId);};
      node.addEventListener('pointermove',move);node.addEventListener('pointerup',up);node.addEventListener('pointercancel',up);move(e);
    };
    node.addEventListener('pointerdown',down);
    return {destroy(){release();send.cancel();node.removeEventListener('pointerdown',down);}};
  }
</script>
<section class="syn-sec" aria-label="FILTER">
  <header class="syn-title"><span class="silk">FILTER</span><ParamKey id="filter.on" {params} {onparam}/></header>
  <div class="syn-disp screen" aria-hidden="true"><canvas use:drag use:display={{ draw: (g,w,h) => drawFilter(g,w,h,synthLive()?synthFeed.frame?.curve??null:null,synthFeed.spectrum,synthFeed.frame?.cutoffHz??params.filter.cutoff,'#ffb21a'), key: JSON.stringify(params.filter) }}></canvas></div>
  <div class="syn-ctl">
    <ChoiceLcd id="filter.type" {params} {onparam}/>
    {#each ['cutoff','reso','drive','keyTrack','envAmt'] as field}<ParamKnob {mod} id="filter.{field}" {params} {onparam} size={knob * (field === 'cutoff' ? 1.2 : 1)} live={live?.[SYNTH_META.dests.findIndex(d => d.id === `filter.${field}`)] ?? null}/>{/each}
  </div>
</section>
<style>canvas { cursor: crosshair; touch-action: none; }</style>
