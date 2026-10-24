<script lang="ts">
  import { onMount } from 'svelte';
  import { pushEsc } from '../lib/esc.ts';
  import Knob from './Knob.svelte';
  import { groupLegend, type Group } from '../lib/model.ts';
  import { fmtLp, fmtPan, fmtSemis, knobToLp, lpToKnob } from '../lib/mix.ts';
  let { index, group, inputs, keysHere, keysWhere, x, y, rename = false, onupdategroup, onkeys, onclose }: {
    index: number; group: Group; inputs: { pads: number; keysColor: string | null; keysLabel: string };
    keysHere: boolean; keysWhere: string; x: number; y: number; rename?: boolean;
    onupdategroup: (index: number, group: Group) => void; onkeys: () => void; onclose: () => void;
  } = $props();
  let pop: HTMLDivElement;
  let left = $state(0), top = $state(0), renaming = $state(false);
  $effect(() => { renaming = rename; });
  onMount(() => {
    const previous = document.activeElement as HTMLElement | null;
    const r = pop.getBoundingClientRect();
    left = Math.max(4, Math.min(x, innerWidth - r.width - 4)); top = Math.max(4, Math.min(y, innerHeight - r.height - 4));
    if (!renaming) pop.querySelector<HTMLButtonElement>('button')?.focus();
    const remove = pushEsc(() => { if (renaming) renaming = false; else onclose(); });
    return () => { remove(); if (previous?.isConnected) previous.focus(); };
  });
  function finishRename(input: HTMLInputElement, commit: boolean) {
    if (!renaming) return; renaming = false;
    const name = input.value.trim().toUpperCase();
    if (commit && name && name !== group.name) onupdategroup(index, { ...group, name });
  }
  function focusSelect(node: HTMLInputElement) { node.focus(); node.select(); }
</script>
<svelte:window onpointerdown={(e) => { if (!pop?.contains(e.target as Node)) onclose(); }} />
<div bind:this={pop} class="chan-pop screen" role="dialog" tabindex="-1" aria-label="{groupLegend(index, group.name)} channel" style:left="{left}px" style:top="{top}px">
  <div class="cp-head">
    {#if renaming}<input class="lcd" aria-label="Group name" value={group.name} maxlength="12" use:focusSelect onkeydown={(e) => { e.stopPropagation(); if (e.key === 'Enter') finishRename(e.currentTarget, true); if (e.key === 'Escape') finishRename(e.currentTarget, false); }} onblur={(e) => finishRename(e.currentTarget, true)} />
    {:else}<button class="lcd" onclick={() => { renaming = true; }}>{groupLegend(index, group.name)}</button>{/if}
    <button class="key small" aria-label="Close" onclick={onclose}>✕</button>
  </div>
  <div class="cp-knobs">
    <Knob label="PAN" ariaLabel="{group.name} pan" value={group.pan} min={-1} max={1} def={0} step={0.02} fine={0.01} size={36} fmt={fmtPan} learnId="group.{index}.pan" onchange={(pan) => onupdategroup(index, { ...group, pan })} />
    <Knob label="PITCH" ariaLabel="{group.name} pitch" value={group.pitchSemitones} min={-12} max={12} def={0} step={1} fine={0.1} size={36} fmt={fmtSemis} learnId="group.{index}.pitch" onchange={(pitchSemitones) => onupdategroup(index, { ...group, pitchSemitones })} />
    <Knob label="LOW-PASS" ariaLabel="{group.name} low-pass" value={lpToKnob(group.lowpassHz)} min={0} max={1} def={1} step={0.01} fine={0.002} size={36} fmt={fmtLp} learnId="group.{index}.lowpass" onchange={(t) => onupdategroup(index, { ...group, lowpassHz: t >= 0.999 ? 20000 : knobToLp(t) })} />
  </div>
  <div class="cp-rows">
    <div class="lcd-dim cp-info">Pads: {inputs.pads} in group {index + 1} (pad editor › MIX GROUP)</div>
    <button class="cp-row" role="menuitemcheckbox" aria-checked={keysHere} onclick={onkeys}>{keysHere ? '✓ ' : ''}{keysWhere}</button>
  </div>
</div>
<style>
  .chan-pop { position: fixed; z-index: 50; padding: 10px; max-width: calc(100vw - 8px); box-sizing: border-box; box-shadow: 0 0 0 1px #000, 0 10px 30px rgba(0,0,0,.7); }
  .cp-head { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
  .cp-head > .lcd { background: none; border: 0; color: var(--lcd); font: 400 12px var(--f-lcd); }
  .cp-head button.lcd { cursor: pointer; }
  .cp-head input { width: 160px; }
  .cp-knobs { display: flex; justify-content: space-around; gap: 16px; padding: 12px 0; }
  .cp-rows { display: flex; flex-direction: column; gap: 6px; }
  .cp-info, .cp-row { font: 400 12px var(--f-lcd); }
  .cp-row { background: none; border: 0; color: var(--lcd); text-align: left; padding: 6px 0; cursor: pointer; }
</style>
