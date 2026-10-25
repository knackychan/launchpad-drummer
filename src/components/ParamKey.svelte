<script lang="ts">
  import { getContext } from 'svelte';
  import { meta, getParam, fmtParam } from '../lib/synth.ts';
  import { LEARN, learnable, isLearning, isLearned, type LearnCtx } from '../lib/learnable.ts';
  import type { SynthParams } from '../lib/synth-types.ts';
  let { id, params, label, onparam }: { id: string; params: SynthParams; label?: string; onparam: (id: string, value: number) => void } = $props();
  const learn = getContext<LearnCtx | undefined>(LEARN);
  const m = $derived(meta(id));
  const value = $derived(getParam(params, id));
  const learnId = $derived(m.kind === 'bool' ? `synth.${id}` : undefined);
</script>
<button class="key small" class:on={!!value} class:learning={isLearning(learn, learnId)} class:learned={isLearned(learn, learnId)} data-learn={learnId} use:learnable={{ ctx: learn, id: learnId }} aria-label="{m.section} {m.label}" aria-pressed={!!value} onclick={() => onparam(id, value ? 0 : 1)}>{m.kind === 'bool' ? label ?? m.label : fmtParam(id, value)}</button>
