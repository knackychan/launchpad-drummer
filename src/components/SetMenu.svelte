<script lang="ts">
  import ContextMenu from './ContextMenu.svelte';
  let { name, path, onnew, onopen, onreveal }: { name: string; path: string; onnew: () => void; onopen: () => void; onreveal: () => void } = $props();
  let position = $state<{ x: number; y: number } | null>(null);
</script>
<button class="set-lcd screen" aria-haspopup="menu" aria-expanded={!!position} onclick={(e) => {
  const r = e.currentTarget.getBoundingClientRect(); position = position ? null : { x: r.left, y: r.bottom + 4 };
}}><span class="silk">SET</span><span class="lcd">{name}</span><span class="lcd lcd-dim">▾</span></button>
{#if position}
  <ContextMenu {...position} onclose={() => position = null} items={[
    { label: 'New set…', shortcut: 'Ctrl+N', action: onnew },
    { label: 'Open set…', shortcut: 'Ctrl+O', action: onopen },
    { label: '', sep: true }, { label: 'Show in Explorer', disabled: !path, action: onreveal },
  ]} />
{/if}
