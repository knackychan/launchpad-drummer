<script lang="ts">
  import NumLcd from './NumLcd.svelte';
  import { meta, getParam, fmtParam } from '../lib/synth.ts';
  import type { SynthParams } from '../lib/synth-types.ts';
  let { id, params, onparam }: { id: string; params: SynthParams; onparam: (id: string, value: number) => void } = $props();
  const m = $derived(meta(id));
</script>
<NumLcd label="{m.section} {m.label}" value={getParam(params, id)} min={m.min!} max={m.max!} step={1} def={m.default} fmt={(v) => fmtParam(id, v)} onchange={(v) => onparam(id, v)} />
