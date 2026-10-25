<script lang="ts">
  import ContextMenu from './ContextMenu.svelte';
  import { meta, getParam, fmtParam } from '../lib/synth.ts';
  import type { SynthParams } from '../lib/synth-types.ts';
  let { id, params, onparam, width }: { id: string; params: SynthParams; onparam: (id: string, value: number) => void; width?: number } = $props();
  const m = $derived(meta(id));
  const value = $derived(getParam(params, id));
  let menu = $state<{ x: number; y: number } | null>(null);
</script>
<button class="lcd screen" style:width={width === undefined ? undefined : `${width}px`} aria-label="{m.section} {m.label}" aria-haspopup="menu" title="{m.section} {m.label} — click to change" onclick={(e) => { const r = e.currentTarget.getBoundingClientRect(); menu = { x: r.left, y: r.bottom + 2 }; }}>{fmtParam(id, value)}</button>
{#if menu}<ContextMenu x={menu.x} y={menu.y} items={m.labels!.map((label, i) => ({ label, checked: i === value, action: () => onparam(id, i) }))} onclose={() => { menu = null; }} />{/if}
<style>
  button { height: 22px; padding: 0 6px; color: var(--lcd); font: 400 11px var(--f-lcd); cursor: pointer; white-space: nowrap; }
</style>
