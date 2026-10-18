<script lang="ts">
  import { pushEsc } from '../lib/esc.ts';
  import { ICONS, type Notice } from '../lib/notices.ts';
  let { notices, idle, onaction }: { notices: Notice[]; idle: string; onaction: (n: Notice) => void } = $props();
  let root: HTMLDivElement;
  let popover = $state<HTMLDivElement>();
  let open = $state(false), howto = $state(false), left = $state(0), top = $state(0);
  const first = $derived(notices[0]);
  function place() { const r = root.getBoundingClientRect(); left = Math.max(4, Math.min(r.left, innerWidth - 468)); top = r.bottom + 4; }
  function action(n: Notice) { if (n.action?.id === 'howto') { place(); howto = true; open = false; } else onaction(n); }
  $effect(() => { if (open || howto) return pushEsc(() => { open = false; howto = false; }); });
</script>
<svelte:window onpointerdown={(e) => { if (!root?.contains(e.target as Node) && !popover?.contains(e.target as Node)) { open = false; howto = false; } }} />
<div bind:this={root} class="notice screen lcd {first?.level ?? ''}" class:learn={first?.prio === 1} role="status" aria-live={first?.level === 'error' ? 'assertive' : 'polite'}>
  {#if first}<span class="ico">{ICONS[first.level]}</span><span class="txt" title={first.text}>{first.text}</span>
    {#if first.action}<button class="key small" onclick={() => action(first)}>{first.action.label}</button>{/if}
  {:else}<span class="txt lcd-dim">{idle}</span>{/if}
  {#if notices.length > 1}<button class="key small" aria-expanded={open} onclick={() => { place(); open = !open; howto = false; }}>+{notices.length - 1}</button>{/if}
</div>
{#if open || howto}
  <div bind:this={popover} class="popover screen" style:left="{left}px" style:top="{top}px">
    {#if howto}
      <div>1. Run Novation's launchpad-mk2-updater.exe.</div><div>2. Unplug the Launchpad.</div><div>3. Plug it back in. The app reconnects within 2 s.</div>
    {:else}{#each notices as n}
      <div class="row" style:color="var(--{n.level === 'info' ? 'lcd-dim' : n.level})"><span>{ICONS[n.level]}</span><span class="text">{n.text}</span>
        {#if n.action}<button class="key small" onclick={() => action(n)}>{n.action.label}</button>{/if}
      </div>
    {/each}{/if}
  </div>
{/if}
<style>
  .txt { flex: 1; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; font-size: 12px; }
  .ico { font: 700 12px var(--f-lcd); width: 12px; text-align: center; }
  .popover { position: fixed; z-index: 50; width: 460px; max-width: calc(100vw - 8px); max-height: 70vh; overflow: auto; padding: 10px; font: 400 12px/1.5 var(--f-lcd); color: var(--lcd); box-shadow: 0 0 0 1px #000,0 10px 30px rgba(0,0,0,.7); }
  .row { display: flex; align-items: center; gap: 8px; padding: 4px 2px; }
  .text { flex: 1; }
</style>
