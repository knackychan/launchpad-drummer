<script lang="ts">
  import RackUnit from './RackUnit.svelte';
  import { targetLabel } from '../lib/learn.ts';
  import type { SetModel } from '../lib/model.ts';
  let { set, onunbind }: { set: SetModel; onunbind: (index: number) => void } = $props();
  const bindings = $derived(set.learn ?? []);
</script>

<RackUnit title="MIDI LEARN">
  {#if bindings.length === 0}
    <div class="screen empty"><span class="lcd lcd-dim">No bindings. Right-click any knob, fader or key → Learn MIDI CC. Pads: LEARN in the pad editor.</span></div>
  {:else}
    <table class="tbl screen">
      <tbody>
        {#each bindings as b, i}
          <tr>
            <td class="lcd">{b.device} · {b.kind === 'note' ? 'NOTE' : 'CC'} {b.number}</td>
            <td class="lcd lcd-dim arrow" aria-hidden="true">→</td>
            <td class="lcd">{targetLabel(b.target, set)}</td>
            <td class="rm"><button type="button" class="key small" aria-label="Remove binding {b.device} {b.kind === 'note' ? 'NOTE' : 'CC'} {b.number}" onclick={() => onunbind(i)}>×</button></td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</RackUnit>

<style>
  .empty { padding: 12px; }
  .tbl { width: 100%; border-collapse: collapse; font-size: 12px; }
  .tbl td { padding: 7px 10px; text-align: left; border-bottom: 1px solid rgba(255,255,255,.05); }
  .arrow { width: 24px; }
  .rm { width: 40px; }
</style>
