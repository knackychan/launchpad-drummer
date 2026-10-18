<script lang="ts">
  import { pushEsc } from "../lib/esc.ts";
  $effect(() => { if (renaming !== null) return pushEsc(() => { renaming = null; }); });
  // Launchpad MK2 mirror (UI spec 4.1–4.2): top buttons, 8×8 pads, side bank buttons + scribble strips, bank strip.
  import PadGrid from './PadGrid.svelte';
  import Knob from './Knob.svelte';
  import type { PadInput, Rgb } from '../lib/led.ts';
  import type { Bank, LightModes } from '../lib/model.ts';

  let {
    renaming = $bindable<number | null>(null),
    leds, names, selected, banks, activeBank, oninput, onbank, onrename, onlight, onhumanize, onstop,
  }: {
    renaming?: number | null;
    leds: readonly Rgb[];
    names: readonly string[];
    selected: number | null;
    banks: Bank[];
    activeBank: number;
    oninput: (input: PadInput) => void;
    onbank: (bank: number) => void;
    onrename: (bank: number, name: string) => void;
    onlight: (key: keyof LightModes, value: boolean) => void;
    onhumanize: (db: number) => void;
    onstop: () => void;
  } = $props();

  const TOP = ['▲', '▼', '◀', '▶', 'S', 'U1', 'U2'];
  const LIGHTS: [keyof LightModes, string, string][] = [
    ['flash', 'FLASH', 'White flash decaying on pad hit'],
    ['playState', 'PLAY', 'Dim idle pads, full brightness while a voice sounds'],
    ['ripple', 'RIPPLE', 'Expanding rings from the hit pad'],
  ];
  const bank = $derived(banks[activeBank]);

  function finishRename(index: number, input: HTMLInputElement, commit: boolean) {
    if (renaming !== index) return;
    renaming = null;
    const name = input.value.trim();
    if (commit && name && name !== banks[index]?.name) onrename(index, name);
  }
  function focusSelect(node: HTMLInputElement) { node.focus(); node.select(); }
</script>

<div class="lp plate" data-label="GRID · LAUNCHPAD MK2">
  <div class="toprow">
    {#each TOP as glyph}
      <span class="key round dim" aria-hidden="true" title="Not used in Part 1">{glyph}</span>
    {/each}
    <button type="button" class="key round" title="Stop all · Esc" aria-label="Stop all" onclick={onstop}>■</button>
  </div>
  <div></div><div></div>

  <PadGrid {leds} {names} {selected} {oninput} />

  <div class="side" role="group" aria-label="Banks">
    {#each banks as b, i}
      <button type="button" class="key round" class:on={i === activeBank} aria-pressed={i === activeBank} aria-label="Bank {i + 1} {b.name}" onclick={() => onbank(i)}></button>
    {/each}
  </div>
  <div class="scribs">
    {#each banks as b, i}
      {#if renaming === i}
        <div class="scrib screen active">
          <input
            value={b.name}
            maxlength="16"
            aria-label="Bank name"
            use:focusSelect
            onkeydown={(e) => { e.stopPropagation(); if (e.key === 'Enter') finishRename(i, e.currentTarget, true); if (e.key === 'Escape') finishRename(i, e.currentTarget, false); }}
            onblur={(e) => finishRename(i, e.currentTarget, true)}
          />
        </div>
      {:else}
        <button
          type="button"
          class="scrib screen"
          class:active={i === activeBank}
          class:emptybank={!b.pads.length}
          title="{b.name} — double-click to rename (F2)"
          onclick={() => onbank(i)}
          ondblclick={() => { renaming = i; }}
          onkeydown={(e) => { if (e.key === 'F2') { e.preventDefault(); renaming = i; } }}
        ><span class="nm">{b.name}</span></button>
      {/if}
    {/each}
  </div>

  <div class="bankstrip">
    {#if bank}
      <span class="silk strip-title">BANK {activeBank + 1} LIGHTS</span>
      {#each LIGHTS as [key, text, tip]}
        <button type="button" class="key" class:on={bank.lights[key]} aria-pressed={bank.lights[key]} title={tip} onclick={() => onlight(key, !bank.lights[key])}>{text}</button>
      {/each}
      <div class="sep"></div>
      <Knob
        label="HUMANIZE"
        row
        value={bank.humanizeDb}
        min={0}
        max={6}
        def={0}
        step={0.5}
        fine={0.1}
        size={30}
        fmt={(v) => (v ? `±${(v / 2).toFixed(2)} dB` : 'OFF')}
        onchange={onhumanize}
      />
    {/if}
  </div>
</div>

<style>
  .lp { padding: 8px 12px; display: grid; grid-template-columns: auto 28px 66px; column-gap: 6px; row-gap: 6px; }
  .toprow { display: grid; grid-template-columns: repeat(8, var(--pad)); gap: 5px; justify-items: center; }
  .toprow .key.round { width: 22px; height: 22px; font-size: 9px; display: grid; place-items: center; }
  .toprow .dim { opacity: .35; cursor: default; pointer-events: none; }
  .side, .scribs { display: grid; grid-auto-rows: var(--pad); gap: 5px; align-items: center; justify-items: center; }
  .scribs { justify-items: stretch; }
  .scrib {
    height: calc(var(--pad) - 14px); display: flex; align-items: center; gap: 5px; padding: 0 6px; border: 0; text-align: left;
    font: 400 10.5px/1 var(--f-lcd); color: var(--lcd-dim); cursor: pointer; overflow: hidden; white-space: nowrap;
  }
  .scrib.active { color: var(--lcd); }
  .scrib .nm { overflow: hidden; text-overflow: ellipsis; }
  .scrib.emptybank .nm { opacity: .55; }
  .scrib input { width: 100%; background: transparent; border: 0; color: #fff; font: inherit; padding: 0; }
  .side .key.round.on {
    background: radial-gradient(circle at 50% 35%, #fffaf0, var(--lcd) 60%, #cfc5ab); color: #160903;
    --lip: #8d8571; --edge: rgba(255,255,255,.3); --halo: 0 0 12px rgba(236,227,204,.5);
  }
  .bankstrip { grid-column: 1 / 4; display: flex; align-items: center; gap: 8px; padding: 6px 0 0; border-top: 1px solid var(--line); }
  .strip-title { margin-right: 4px; }
  .sep { width: 1px; height: 28px; background: var(--line); margin: 0 6px; }
</style>
