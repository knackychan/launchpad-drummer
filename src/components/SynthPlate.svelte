<script lang="ts">
  import type { SetModel } from '../lib/model.ts';
  import { SYNTH_PAGES, synthKnob, synthPageSet } from '../lib/layout.ts';
  import SynthOsc from './SynthOsc.svelte';
  import SynthSubNoise from './SynthSubNoise.svelte';
  import SynthFilter from './SynthFilter.svelte';
  import SynthAmpVoice from './SynthAmpVoice.svelte';
  import SynthEnv from './SynthEnv.svelte';
  import SynthLfo from './SynthLfo.svelte';
  import SynthFx from './SynthFx.svelte';
  import ModSourceStrip from './ModSourceStrip.svelte';
  import SynthMatrix from './SynthMatrix.svelte';
  import DragGhost from './DragGhost.svelte';
  import { onDestroy, onMount } from 'svelte';
  import { pushEsc } from '../lib/esc.ts';
  import { sourceColor, sourceLabel, throttle } from '../lib/synth.ts';
  import type { Route, SourceId, SynthTelemetry } from '../lib/synth-types.ts';
  import { onSynthFeed, synthFeed, synthLive } from '../lib/synthFeed.ts';
  import { display } from '../lib/synthDisplay.ts';
  import { drawScope } from '../lib/synthDraw.ts';
  let { set, width, height, engine, onusesynth, onparam, selectedRoute, onaddroute, onupdateroute, onremoveroute, onroutedepth, onselectroute, onblankroute }: {
    set: SetModel; width: number; height: number; engine: string; onusesynth: () => void;
    onparam: (id: string, value: number) => void;
    selectedRoute: Record<string, number>; onaddroute: (source: SourceId, dest: string) => void;
    onupdateroute: (slot: number, route: Route) => void; onremoveroute: (slot: number) => void;
    onroutedepth: (dest: string, delta: number) => void; onselectroute: (dest: string, slot: number) => void; onblankroute: () => void;
  } = $props();
  const pageSet = $derived(synthPageSet(width, height));
  let frame = $state<SynthTelemetry | null>(null);
  let reduced = $state(false);
  onMount(() => {
    const query=matchMedia('(prefers-reduced-motion: reduce)');
    const change=()=>{reduced=query.matches;};change();query.addEventListener('change',change);
    return()=>query.removeEventListener('change',change);
  });
  $effect(() => {
    const update=throttle(()=>{if(!document.hidden)frame=synthLive()?synthFeed.frame:null;},reduced?100:1000/60);
    let expiry:ReturnType<typeof setTimeout>;
    const stop=onSynthFeed(()=>{update();clearTimeout(expiry);expiry=setTimeout(update,501);});
    document.addEventListener('visibilitychange',update);
    update();
    return()=>{stop();update.cancel();clearTimeout(expiry);document.removeEventListener('visibilitychange',update);};
  });
  let chosen = $state<Record<string, string>>({});
  $effect(() => {
    const kind = pageSet;
    let page = SYNTH_PAGES[kind][0];
    try { const saved = localStorage.getItem(`lpd.synth.page.${kind}`); if (saved && SYNTH_PAGES[kind].includes(saved)) page = saved; } catch { /* pages work without storage */ }
    chosen[kind] = page;
  });
  const page = $derived(chosen[pageSet] ?? SYNTH_PAGES[pageSet][0]);
  const knob = $derived(Math.min(synthKnob(height, pageSet), page === 'MAIN' || page === 'OSC' || page === 'FILTER' || (page === 'FX' && pageSet === 'compact') ? 24 : 44));
  const sectionProps = $derived({ params: set.keys.patch.params, live: frame?.live??null, knob, onparam, onstart: startMod, mod: { selected: selectedRoute, onselect: onselectroute, ondepth: onroutedepth, onremove: onremoveroute } });
  let modDrag = $state<{ source: SourceId; x: number; y: number; active: boolean } | null>(null);
  let stopMod: (() => void) | undefined;
  function startMod(source: SourceId, event: PointerEvent) {
    if (event.button !== 0 || !(event.currentTarget instanceof HTMLElement)) return;
    stopMod?.();
    event.preventDefault();
    const el = event.currentTarget, x0 = event.clientX, y0 = event.clientY;
    el.setPointerCapture(event.pointerId);
    modDrag = { source, x: x0, y: y0, active: false };
    let target: HTMLElement | null = null;
    const move = (e: PointerEvent) => {
      if (!modDrag) return;
      modDrag.x = e.clientX; modDrag.y = e.clientY;
      modDrag.active ||= Math.hypot(e.clientX-x0, e.clientY-y0) >= 4;
      target?.classList.remove('drop-ok');
      target = modDrag.active ? document.elementFromPoint(e.clientX, e.clientY)?.closest<HTMLElement>('[data-dest]') ?? null : null;
      target?.classList.add('drop-ok');
    };
    const up = (e: PointerEvent) => {
      move(e);
      const dest = target?.dataset.dest;
      const active = modDrag?.active;
      stopMod?.();
      if (active && dest) onaddroute(source, dest);
    };
    const removeEsc = pushEsc(() => stopMod?.());
    stopMod = () => {
      target?.classList.remove('drop-ok');
      el.removeEventListener('pointermove', move); el.removeEventListener('pointerup', up); el.removeEventListener('pointercancel', cancel);
      if (el.hasPointerCapture(event.pointerId)) el.releasePointerCapture(event.pointerId);
      removeEsc(); modDrag = null; stopMod = undefined;
    };
    const cancel = () => stopMod?.();
    el.addEventListener('pointermove', move); el.addEventListener('pointerup', up); el.addEventListener('pointercancel', cancel);
  }
  onDestroy(() => stopMod?.());
  function choose(next: string) {
    stopMod?.();
    chosen[pageSet] = next;
    try { localStorage.setItem(`lpd.synth.page.${pageSet}`, next); } catch { /* pages work without storage */ }
  }
</script>
{#if modDrag?.active}<DragGhost x={modDrag.x} y={modDrag.y} color={sourceColor(modDrag.source)} label={sourceLabel(modDrag.source)}/>{/if}
<section class="synth plate" data-label="SYNTH" aria-label="Synth" style:width="{width}px" style:height="{height}px" style:--kn="{knob}px">
  <header class="syn-head">
    <div class="syn-head-row">
      <div class="lcd screen syn-patch">{set.keys.patch.name}{set.keys.modified ? ' *' : ''}</div>
      <div class="switch" role="tablist" aria-label="Synth pages">
        {#each SYNTH_PAGES[pageSet] as name}<button class="key small" class:on={name === page} role="tab" aria-selected={name === page} onclick={() => choose(name)}>{name}</button>{/each}
      </div>
    </div>
    <div class="syn-head-row"><ModSourceStrip meters={frame?.sources??null} onstart={startMod}/><canvas class="screen syn-scope" aria-hidden="true" use:display={{ draw: (g,w,h) => drawScope(g,w,h,synthLive()?synthFeed.frame?.scope??null:null) }}></canvas><span class="lcd-dim syn-stats">VOICES {frame?`${frame.voices}/16`:'—'}<br/>CPU {frame?`${Math.round(frame.cpu*100)} %`:'—'}</span><span class="lamp" class:ok={engine==='synth'} title={engine==='soundfont'?'SOUNDFONT':'SYNTH'}></span></div>
  </header>
  <div class="syn-body" role="tabpanel" aria-label={page} inert={engine === 'soundfont'}>
    {#if page === 'MAIN'}
      <div class="pg-main">
        <div class="syn-row" style:grid-template-columns="1fr 1fr .6fr"><SynthOsc x="A" {...sectionProps}/><SynthOsc x="B" {...sectionProps}/><SynthSubNoise {...sectionProps}/></div>
        <div class="syn-row" style:grid-template-columns="1.4fr .5fr 1fr"><SynthFilter {...sectionProps}/><SynthAmpVoice {...sectionProps}/></div>
        <div class="syn-row" style:grid-template-columns="repeat(3,.7fr) 1fr 1fr"><SynthEnv n={1} {...sectionProps}/><SynthEnv n={2} {...sectionProps}/><SynthEnv n={3} {...sectionProps}/><SynthLfo n={1} {...sectionProps}/><SynthLfo n={2} {...sectionProps}/></div>
      </div>
    {:else if page === 'OSC'}
      <div class="pg-osc" class:compact={pageSet === 'compact'}>
        <div class="syn-row" style:grid-template-columns="1fr 1fr"><SynthOsc x="A" {...sectionProps}/><SynthOsc x="B" {...sectionProps}/></div>
        {#if pageSet !== 'compact'}<div class="syn-row" style:grid-template-columns=".7fr 1.3fr 1fr"><SynthSubNoise {...sectionProps}/><SynthFilter {...sectionProps}/><SynthAmpVoice {...sectionProps}/></div>{/if}
      </div>
    {:else if page === 'MOD'}
      <div class="pg-mod">
        <div class="syn-row" style:grid-template-columns="1fr 1fr 1fr"><SynthEnv n={1} {...sectionProps}/><SynthEnv n={2} {...sectionProps}/><SynthEnv n={3} {...sectionProps}/></div>
        <div class="syn-row" style:grid-template-columns="1fr 1fr"><SynthLfo n={1} {...sectionProps}/><SynthLfo n={2} {...sectionProps}/></div>
      </div>
    {:else if page === 'FILTER'}
      <div class="syn-row" style:height="100%" style:grid-template-columns=".7fr 1.3fr 1fr"><SynthSubNoise {...sectionProps}/><SynthFilter {...sectionProps}/><SynthAmpVoice {...sectionProps}/></div>
    {:else if page === 'ENV'}
      <div class="syn-row" style:height="100%" style:grid-template-columns="1fr 1fr 1fr"><SynthEnv n={1} {...sectionProps}/><SynthEnv n={2} {...sectionProps}/><SynthEnv n={3} {...sectionProps}/></div>
    {:else if page === 'LFO'}
      <div class="syn-row" style:height="100%" style:grid-template-columns="1fr 1fr"><SynthLfo n={1} {...sectionProps}/><SynthLfo n={2} {...sectionProps}/></div>
    {:else if page === 'FX'}
      <div class="pg-fx" class:wide={pageSet === 'wide'}>{#each ['drive','chorus','delay','reverb'] as unit}<SynthFx unit={unit as 'drive' | 'chorus' | 'delay' | 'reverb'} {...sectionProps}/>{/each}</div>
    {:else}
      <SynthMatrix params={set.keys.patch.params} meters={frame?.sources??null} onupdate={onupdateroute} onremove={onremoveroute} onadd={onblankroute}/>
    {/if}
  </div>
  {#if engine === 'soundfont'}<div class="syn-dim" role="note"><span class="lcd">SOUNDFONT ACTIVE</span><button class="key small" onclick={onusesynth}>USE SYNTH</button></div>{/if}
</section>
<style>
  .pg-main { display: grid; grid-template-rows: 1.15fr 1fr 1fr; gap: 6px; height: 100%; }
  .pg-main > .syn-row:nth-child(2) > :global(.syn-row) { display: contents; }
  .pg-osc, .pg-mod { display: grid; grid-template-rows: 1fr 1fr; gap: 6px; height: 100%; }
  .pg-osc.compact { grid-template-rows: 1fr; }
  .pg-fx { display: grid; grid-template-columns: repeat(2,minmax(0,1fr)); grid-template-rows: 1fr 1fr; gap: 6px; height: 100%; }
  .pg-fx.wide { grid-template-columns: repeat(4,minmax(0,1fr)); grid-template-rows: 1fr; }
  .synth { flex: none; box-sizing: border-box; display: flex; flex-direction: column; gap: 6px; padding: 10px 10px 8px; position: relative; min-width: 0; }
  .syn-head { flex: none; display: flex; flex-direction: column; gap: 4px; }
  .syn-head-row { display: flex; align-items: center; gap: 6px; height: 26px; }
  .syn-head-row:nth-child(2) { height: auto; min-height: 26px; }
  .syn-scope { width: 120px; height: 24px; flex: none; }
  .syn-stats { font-size: 10px; white-space: nowrap; flex: none; }
  .syn-patch { min-width: 120px; max-width: 260px; flex: 1; height: 24px; line-height: 24px; padding: 0 8px; font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; cursor: pointer; }
  .syn-body { flex: 1; min-height: 0; position: relative; }
  .syn-dim { position: absolute; inset: 0; z-index: 2; display: flex; align-items: center; justify-content: center; gap: 12px; background: rgba(0,0,0,.66); }
</style>
