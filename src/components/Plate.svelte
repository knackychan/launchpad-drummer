<script lang="ts">
  import type { Snippet } from 'svelte';
  let { label, class: className = '', children }: { label: string; class?: string; children: Snippet } = $props();
</script>

<section class="plate {className}" data-label={label} aria-label={label}>
  {@render children()}
</section>
