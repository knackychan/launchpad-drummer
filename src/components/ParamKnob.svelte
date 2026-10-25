<script lang="ts">
  import Knob from './Knob.svelte';
  import type { MenuItem } from './ContextMenu.svelte';
  import type { SynthParams } from '../lib/synth-types.ts';
  import { meta, getParam, toNorm, fromNorm, fmtParam, destOf, arcsFor, lcxlTag, sourceLabel, fmtAmount } from '../lib/synth.ts';
  let { id, params, live = null, size, label, onparam, onalt, selected = null, onselect, menuExtra, mod }: {
    id: string; params: SynthParams; live?: number | null; size: number; label?: string;
    onparam: (id: string, value: number) => void; onalt?: (delta: number) => void;
    selected?: number | null; onselect?: (slot: number) => void; menuExtra?: () => MenuItem[];
    mod?: { selected: Record<string, number>; onselect: (dest: string, slot: number) => void; ondepth: (dest: string, delta: number) => void; onremove: (slot: number) => void };
  } = $props();
  const m = $derived(meta(id));
  const value = $derived(toNorm(m, getParam(params, id)));
  const dest = $derived(destOf(id));
  const mods = $derived(dest ? arcsFor(params, dest, value) : { arcs: [], more: 0 });
  const tagTitle = $derived(lcxlTag(id));
  const step = $derived(m.kind === 'choice' ? 1 / (m.choices!.length - 1) : m.kind === 'int' ? 1 / (m.max! - m.min!) : 0.01);
  function routeMenu(): MenuItem[] {
    return [...(mod && dest ? params.routes.flatMap((r, slot) => r.dest === dest ? [{ label: `Remove ${sourceLabel(r.source)} (${fmtAmount(dest, r.amount)})`, action: () => mod?.onremove(slot) }] : []) : []), ...(menuExtra?.() ?? [])];
  }
</script>
<Knob label={label ?? m.label} ariaLabel="{m.section} {m.label}" {value} min={0} max={1} def={toNorm(m, m.default)} {step} fine={0.001} {size} fmt={(n) => fmtParam(id, fromNorm(m, n))} learnId="synth.{id}" arcs={mods.arcs} more={mods.more} {live} selected={mod && dest ? mod.selected[dest] ?? null : selected} onselect={mod && dest ? (slot) => mod.onselect(dest, slot) : onselect} tag={tagTitle ? 'XL' : undefined} tagTitle={tagTitle ?? undefined} {dest} onalt={mod && dest ? (delta) => mod.ondepth(dest, delta) : onalt} menuExtra={routeMenu} onchange={(n) => onparam(id, fromNorm(m, n))} />
