<script lang="ts">
  import ContextMenu, { type MenuItem } from './ContextMenu.svelte';
  import NumLcd from './NumLcd.svelte';
  import { SYNTH_META } from '../lib/synth-meta.ts';
  import { sourceColor, sourceLabel, destLabel, fmtAmount } from '../lib/synth.ts';
  import type { SynthParams, Route, SourceId } from '../lib/synth-types.ts';
  let { params, meters, onupdate, onremove, onadd }: { params: SynthParams; meters: number[] | null; onupdate: (slot: number, route: Route) => void; onremove: (slot: number) => void; onadd: () => void } = $props();
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  function open(e: MouseEvent, items: MenuItem[]) { const r = e.currentTarget instanceof HTMLElement ? e.currentTarget.getBoundingClientRect() : null; if (r) menu = { x: r.left, y: r.bottom+2, items }; }
  function destinations(slot: number, route: Route): MenuItem[] {
    return ['oscA','oscB','osc','sub','noise','filter','amp','lfo1','lfo2'].flatMap((section, i) => [
      { label: ['OSC A','OSC B','OSC','SUB','NOISE','FILTER','AMP','LFO1','LFO2'][i], info: true },
      ...SYNTH_META.dests.filter(d => d.id.split('.')[0] === section).map(d => ({ label: destLabel(d.id), checked: route.dest === d.id, action: () => onupdate(slot, { ...route, dest: d.id }) })),
    ]);
  }
  function meter(source: SourceId): number { const v = meters?.[SYNTH_META.sources.findIndex(s => s.id === source)]; return v === undefined ? 0 : Math.min(1, Math.max(0, ['lfo1','lfo2','note','rand','bend'].includes(source) ? (v+1)/2 : v)); }
</script>
<div class="matrix" role="table" aria-label="Modulation matrix">
  <div class="mx-row silk" role="row">{#each ['SOURCE','DESTINATION','AMOUNT','AUX','BYP','·','×'] as label}<span role="columnheader">{label}</span>{/each}</div>
  {#each params.routes as route, slot}
    <div class="mx-row" role="row">
      <div role="cell"><button class="lcd screen" aria-label="Route {slot+1} source" aria-haspopup="menu" onclick={(e) => open(e, SYNTH_META.sources.map(s => ({ label: s.label, checked: s.id === route.source, action: () => onupdate(slot, { ...route, source: s.id }) })))}>{sourceLabel(route.source)}</button></div>
      <div role="cell"><button class="lcd screen" aria-label="Route {slot+1} destination" aria-haspopup="menu" title={route.dest ? destLabel(route.dest) : '—'} onclick={(e) => open(e, destinations(slot, route))}>{route.dest ? destLabel(route.dest) : '—'}</button></div>
      <div role="cell"><NumLcd label="Route {slot+1} amount" value={route.amount} min={-1} max={1} step={0.01} def={0.25} fmt={(v) => fmtAmount(route.dest, v)} onchange={(amount) => onupdate(slot, { ...route, amount })}/></div>
      <div role="cell"><button class="lcd screen" aria-label="Route {slot+1} aux" aria-haspopup="menu" onclick={(e) => open(e, [{ label: '—', checked: route.aux === null, action: () => onupdate(slot, { ...route, aux: null }) }, ...SYNTH_META.sources.map(s => ({ label: s.label, checked: s.id === route.aux, action: () => onupdate(slot, { ...route, aux: s.id }) }))])}>{route.aux ? sourceLabel(route.aux) : '—'}</button></div>
      <div role="cell"><button class="key small" aria-label="Route {slot+1} bypass" aria-pressed={route.bypass} class:on={route.bypass} onclick={() => onupdate(slot, { ...route, bypass: !route.bypass })}>BYP</button></div>
      <div role="cell" aria-hidden="true"><i class="mx-meter" style:background={sourceColor(route.source)} style:transform="scaleY({meter(route.source)})"></i></div>
      <div role="cell"><button class="key small" aria-label="Delete route {slot+1}" onclick={() => onremove(slot)}>×</button></div>
    </div>
  {/each}
  <button class="key small" aria-disabled={params.routes.length >= 16} onclick={onadd}>+ ADD ROUTE</button>
</div>
{#if menu}<ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => { menu = null; }}/>{/if}
<style>
  .matrix { height: 100%; overflow: auto; }
  .mx-row { display: grid; grid-template-columns: minmax(54px,1fr) minmax(60px,1.5fr) 82px minmax(40px,1fr) 34px 8px 24px; gap: 4px; align-items: center; margin-bottom: 5px; }
  .mx-row > div { min-width: 0; }
  .mx-row button.lcd { width: 100%; height: 24px; padding: 0 4px; font: 400 11px var(--f-lcd); color: var(--lcd); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; cursor: pointer; }
  .mx-row .key { min-width: 0; padding: 0 3px; width: 100%; }
  .mx-row.silk { font-size: 9px; }
  .mx-meter { display: block; width: 4px; height: 18px; transform-origin: bottom; }
</style>
