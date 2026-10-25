<script lang="ts">
  import type { ComponentProps } from 'svelte';
  import ParamKnob from './ParamKnob.svelte';
  import ParamStep from './ParamStep.svelte';
  import ParamKey from './ParamKey.svelte';
  import ChoiceLcd from './ChoiceLcd.svelte';
  import { SYNTH_META } from '../lib/synth-meta.ts';
  import { display, tableFrames } from '../lib/synthDisplay.ts';
  import { drawWavetable } from '../lib/synthDraw.ts';
  import { synthFeed, synthLive } from '../lib/synthFeed.ts';
  import type { SynthParams } from '../lib/synth-types.ts';
  let { x, params, live, knob, onparam, mod }: { x: 'A' | 'B'; params: SynthParams; live: number[] | null; knob: number; onparam: (id: string, value: number) => void; mod?: ComponentProps<typeof ParamKnob>['mod'] } = $props();
  const prefix = $derived(`osc${x}`);
  const osc = $derived(x === 'A' ? params.oscA : params.oscB);
  let frames = $state<number[][] | null>(null);
  $effect(() => {
    let active = true;
    frames = null;
    void tableFrames(osc.table).then(value => { if(active)frames=value; });
    return () => { active=false; };
  });
  function draw(g: CanvasRenderingContext2D, w: number, h: number) {
    const pos = synthLive() ? synthFeed.frame?.live[SYNTH_META.dests.findIndex(d=>d.id===`${prefix}.pos`)] ?? osc.pos : osc.pos;
    const selected = Math.round(pos*Math.max(0,(frames?.length??1)-1));
    const preview = frames?.map((frame,index) => index !== selected || !['sync','bend','pwm'].includes(osc.warp) ? frame : frame.map((_,i) => {
      let phase=i/frame.length;
      if(osc.warp==='sync')phase=(phase*16**osc.warpAmt)%1;
      else if(osc.warp==='bend')phase=phase**(1+4*osc.warpAmt);
      else { const width=1-0.98*osc.warpAmt;if(phase>=width)return 0;phase/=width; }
      const at=phase*frame.length,j=Math.floor(at)%frame.length,f=at-Math.floor(at);
      return frame[j]*(1-f)+frame[(j+1)%frame.length]*f;
    })) ?? null;
    drawWavetable(g,w,h,preview,pos,x==='A'?'#7dffb0':'#41c8ff');
  }
</script>
<section class="syn-sec" aria-label="OSC {x}">
  <header class="syn-title"><span class="silk">OSC {x}</span><ParamKey id="{prefix}.on" {params} {onparam}/></header>
  <div class="syn-disp screen" aria-hidden="true"><canvas use:display={{ draw, key: [JSON.stringify(osc), frames] }}></canvas></div>
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
<style>.syn-ctl { row-gap: 0; }</style>
