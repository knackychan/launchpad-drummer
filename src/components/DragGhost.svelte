<script lang="ts">
  let { x, y, color, label }: { x: number; y: number; color: string; label: string } = $props();
</script>
<div class="ghost screen" style:left="{x}px" style:top="{y}px" aria-hidden="true"><i style:background={color} style:box-shadow="0 0 8px {color}"></i>{label}</div>
<style>
  .ghost { position: fixed; z-index: 60; pointer-events: none; display: flex; align-items: center; gap: 6px; padding: 4px 8px 4px 4px; font: 500 11px var(--f-lcd); color: var(--lcd); transform: translate(10px,10px); }
  i { width: 22px; height: 22px; border-radius: 4px; display: block; }
</style>
