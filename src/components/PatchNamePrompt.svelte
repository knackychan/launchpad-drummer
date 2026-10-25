<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { pushEsc } from '../lib/esc.ts';
  let { title, initial, error, x, y, onsubmit, oncancel }: { title: 'SAVE AS' | 'RENAME'; initial: string; error: string | null; x: number; y: number; onsubmit: (name: string) => void; oncancel: () => void } = $props();
  let value=$state(untrack(()=>initial)), pop=$state<HTMLDivElement>(), input:HTMLInputElement;
  let left=$state(0),top=$state(0);
  onMount(()=>{
    const previous=document.activeElement as HTMLElement|null;
    input.focus();input.select();
    const remove=pushEsc(oncancel);
    return()=>{remove();if(previous?.isConnected)previous.focus();};
  });
  $effect(()=>{error;if(!pop)return;const r=pop.getBoundingClientRect();left=Math.max(4,Math.min(x,innerWidth-r.width-4));top=Math.max(4,Math.min(y,innerHeight-r.height-4));});
  const submit=()=>onsubmit(value.trim().toUpperCase());
</script>
<div class="patch-name screen" role="dialog" aria-label={title} tabindex="-1" bind:this={pop} style:left="{left}px" style:top="{top}px">
  <span class="silk">{title}</span>
  <input class="lcd screen" bind:this={input} bind:value maxlength="24" aria-label="Patch name" onkeydown={(e)=>{if(e.key==='Enter'){e.preventDefault();e.stopPropagation();submit();}else if(e.key==='Escape'){e.preventDefault();e.stopPropagation();oncancel();}}}/>
  {#if error}<span class="error lcd" role="alert">{error}</span>{/if}
  <div><button class="key small" onclick={submit}>SAVE</button><button class="key small" onclick={oncancel}>CANCEL</button></div>
</div>
<style>
  .patch-name { position: fixed; z-index: 55; padding: 10px; display: flex; flex-direction: column; gap: 8px; width: 270px; max-width: calc(100vw - 8px); box-shadow: 0 0 0 1px #000, 0 10px 30px rgba(0,0,0,.7); }
  input { width: 100%; padding: 6px; text-transform: uppercase; }
  .error { color: var(--error); font-size: 11px; }
  .patch-name > div { display: flex; gap: 6px; justify-content: flex-end; }
</style>
