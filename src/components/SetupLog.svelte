<script lang="ts">
  import RackUnit from './RackUnit.svelte';
  let { onopen }: { onopen: () => void } = $props();
</script>

<RackUnit title="LOG">
  <div class="row">
    <button type="button" class="key" onclick={onopen}>OPEN LOG FOLDER</button>
    <p class="lcd lcd-dim">Rotating log: lpd.log + 3 older files, 1 MB each.</p>
  </div>
</RackUnit>

<style>
  .row { display: flex; align-items: center; gap: 14px; }
  p { margin: 0; }
</style>
