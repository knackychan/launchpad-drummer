<script lang="ts">
  import RackUnit from './RackUnit.svelte';
  import type { Bend, CrtLevel } from '../lib/display.ts';
  let { level, bend, onchange, onbend }: {
    level: CrtLevel; bend: Bend;
    onchange: (level: CrtLevel) => void; onbend: (bend: Bend) => void;
  } = $props();
  const levels: CrtLevel[] = ['off', 'low', 'medium'];
  const bends: [Bend, string][] = [['flat', 'FLAT'], ['bent', 'BENT']];
  const bendOff = $derived(level === 'off');
</script>

<RackUnit title="DISPLAY">
  <fieldset>
    <legend class="silk">CRT EFFECTS</legend>
    <div class="switch">
      {#each levels as value}
        <label class:active={level === value}>
          <input type="radio" name="crt" {value} checked={level === value} onchange={() => onchange(value)} />
          {value.toUpperCase()}
        </label>
      {/each}
    </div>
  </fieldset>
  <fieldset class:dim={bendOff}>
    <legend class="silk">BEND</legend>
    <div class="switch" aria-disabled={bendOff}>
      {#each bends as [value, text]}
        <label class:active={bend === value}>
          <input type="radio" name="bend" {value} checked={bend === value} aria-disabled={bendOff} onclick={(e) => { if (bendOff) e.preventDefault(); }} onchange={() => { if (!bendOff) onbend(value); }} />
          {text}
        </label>
      {/each}
    </div>
  </fieldset>
  <p class="lcd lcd-dim">Glow, curved pixel mask, grain, vignette and a scan line that now and then sweeps down (sometimes stalling mid-screen). BENT also bulges the picture itself (text near the edges gets softer). Reduced motion keeps the mask and grain still.</p>
</RackUnit>

<style>
  .dim { opacity: .45; }
  .dim label, .dim input { cursor: default; }
</style>
