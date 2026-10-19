<script lang="ts">
  import RackUnit from './RackUnit.svelte';
  import type { CrtLevel } from '../lib/display.ts';
  let { level, onchange }: { level: CrtLevel; onchange: (level: CrtLevel) => void } = $props();
  const levels: CrtLevel[] = ['off', 'low', 'medium'];
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
  <p class="lcd lcd-dim">Glow, curved pixel mask, grain, vignette and an occasional scan line.</p>
  <p class="lcd lcd-dim">Reduced motion keeps the mask and grain still.</p>
</RackUnit>
