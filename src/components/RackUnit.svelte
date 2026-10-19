<script lang="ts">
  import type { Snippet } from 'svelte';
  import Plate from './Plate.svelte';
  let { title, children }: { title: string; children: Snippet } = $props();
</script>

<Plate label={title} class="rack-unit">
  <span class="screw tl" aria-hidden="true"></span><span class="screw tr" aria-hidden="true"></span>
  <span class="screw bl" aria-hidden="true"></span><span class="screw br" aria-hidden="true"></span>
  <h1>{title}</h1>
  {@render children()}
</Plate>

<style>
  /* screws become registration marks: a ring with a crosshair */
  .screw { position: absolute; width: 11px; height: 11px; border-radius: 50%; box-shadow: inset 0 0 0 1px var(--vec-dim); pointer-events: none; }
  .screw::before { content: ''; position: absolute; top: -3px; bottom: -3px; left: 5px; width: 1px; background: var(--vec-dim); }
  .screw::after { content: ''; position: absolute; left: -3px; right: -3px; top: 5px; height: 1px; background: var(--vec-dim); }
  .tl { left: 12px; top: 12px; }
  .tr { right: 12px; top: 12px; }
  .bl { left: 12px; bottom: 12px; }
  .br { right: 12px; bottom: 12px; }
</style>
