<script lang="ts" module>
  export interface MenuItem { label: string; shortcut?: string; disabled?: boolean; checked?: boolean; info?: boolean; sep?: boolean; action?: () => void }
</script>
<script lang="ts">
  import { onMount } from 'svelte';
  import { pushEsc } from '../lib/esc.ts';
  let { x, y, items, onclose }: { x: number; y: number; items: MenuItem[]; onclose: () => void } = $props();
  let menu: HTMLDivElement;
  let left = $state(0), top = $state(0);
  onMount(() => {
    const previous = document.activeElement as HTMLElement | null;
    const rect = menu.getBoundingClientRect();
    left = Math.max(4, Math.min(x, innerWidth - rect.width - 4));
    top = Math.max(4, Math.min(y, innerHeight - rect.height - 4));
    menu.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
    const remove = pushEsc(onclose);
    return () => { remove(); if (previous?.isConnected) previous.focus(); };
  });
  function keydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === 'Escape') { e.preventDefault(); onclose(); return; }
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    e.preventDefault();
    const buttons = [...menu.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')];
    const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
    buttons[(index + (e.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length]?.focus();
  }
</script>

<svelte:window onpointerdown={(e) => { if (!menu?.contains(e.target as Node)) onclose(); }} />
<div bind:this={menu} class="menu screen" role="menu" tabindex="-1" style:left="{left}px" style:top="{top}px" onkeydown={keydown}>
  {#each items as item}
    {#if item.sep}<div class="sep" role="separator"></div>
    {:else if item.info}<div class="info lcd-dim" role="presentation">{item.label}</div>
    {:else}
      <button role={item.checked === undefined ? 'menuitem' : 'menuitemcheckbox'} aria-checked={item.checked}
        disabled={item.disabled} onclick={() => { onclose(); item.action?.(); }}>
        <span>{item.checked ? '✓ ' : ''}{item.label}</span><span class="lcd-dim">{item.shortcut ?? ''}</span>
      </button>
    {/if}
  {/each}
</div>

<style>
  .menu { position: fixed; z-index: 50; min-width: 190px; max-width: calc(100vw - 8px); padding: 4px; display: flex; flex-direction: column; box-shadow: 0 0 0 1px #000, 0 10px 30px rgba(0,0,0,.7); }
  .menu button { background: none; border: 0; text-align: left; padding: 7px 10px; font: 400 12px var(--f-lcd); color: var(--lcd); border-radius: 3px; cursor: pointer; display: flex; gap: 24px; justify-content: space-between; }
  .menu button:hover, .menu button:focus-visible { background: rgba(94,200,255,.14); }
  .menu button:disabled { color: var(--lcd-dim); cursor: default; }
  .sep { height: 1px; background: rgba(255,255,255,.07); margin: 3px 0; }
  .info { padding: 7px 10px; font: 400 12px var(--f-lcd); }
</style>
