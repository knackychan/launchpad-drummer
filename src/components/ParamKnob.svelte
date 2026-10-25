<script lang="ts">
  import Knob from './Knob.svelte';
  import type { MenuItem } from './ContextMenu.svelte';
  import type { SynthParams } from '../lib/synth-types.ts';
  import { meta, getParam, toNorm, fromNorm, fmtParam, destOf, arcsFor, lcxlTag } from '../lib/synth.ts';
  let { id, params, live = null, size, label, onparam, onalt, selected = null, onselect, menuExtra }: {
    id: string; params: SynthParams; live?: number | null; size: number; label?: string;
    onparam: (id: string, value: number) => void; onalt?: (delta: number) => void;
    selected?: number | null; onselect?: (slot: number) => void; menuExtra?: () => MenuItem[];
  } = $props();
  const m = $derived(meta(id));
  const value = $derived(toNorm(m, getParam(params, id)));
  const dest = $derived(destOf(id));
  const mods = $derived(dest ? arcsFor(params, dest, value) : { arcs: [], more: 0 });
  const tagTitle = $derived(lcxlTag(id));
  const step = $derived(m.kind === 'choice' ? 1 / (m.choices!.length - 1) : m.kind === 'int' ? 1 / (m.max! - m.min!) : 0.01);
</script>
<Knob label={label ?? m.label} ariaLabel="{m.section} {m.label}" {value} min={0} max={1} def={toNorm(m, m.default)} {step} fine={0.001} {size} fmt={(n) => fmtParam(id, fromNorm(m, n))} learnId="synth.{id}" arcs={mods.arcs} more={mods.more} {live} {selected} {onselect} tag={tagTitle ? 'XL' : undefined} tagTitle={tagTitle ?? undefined} {dest} {onalt} {menuExtra} onchange={(n) => onparam(id, fromNorm(m, n))} />
