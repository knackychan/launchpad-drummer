<script lang="ts">
  import { pushEsc } from "../lib/esc.ts";
  $effect(() => { if (renaming !== null) return pushEsc(() => { renaming = null; }); });
  // 1:1 Launch Control XL mirror + master (UI spec 7.1): label column, 8 channel columns, master.
  import { getContext } from 'svelte';
  import { isLearned, isLearning, learnable, LEARN, type LearnCtx } from '../lib/learnable.ts';
  import Knob from './Knob.svelte';
  import Fader from './Fader.svelte';
  import type { Group } from '../lib/model.ts';
  import { groupLegend } from '../lib/model.ts';
  import type { MixDims } from '../lib/layout.ts';
  import { dbToPos, fmtDbShort, fmtLp, fmtPan, fmtSemis, knobToLp, lpToKnob } from '../lib/mix.ts';

  let {
    renaming = $bindable<number | null>(null),
    groups, masterDb, dims, onupdategroup, onupdatemaster, onstopall,
    inputs = [], onchannelmenu,
  }: {
    /** Per channel: pads routed to it, and the keys' sound colour and label when they play through it. */
    inputs?: { pads: number; keysColor: string | null; keysLabel: string }[];
    /** A click or right-click on a channel's scribble strip; `index` −1 closes the menu. */
    onchannelmenu?: (index: number, x: number, y: number) => void;
    renaming?: number | null;
    groups: Group[];
    masterDb: number;
    dims?: MixDims;
    onupdategroup: (index: number, group: Group) => void;
    onupdatemaster: (masterDb: number) => void;
    onstopall?: () => void;
  } = $props();

  const learn = getContext<LearnCtx | undefined>(LEARN);
  const DEFAULT_DIMS: MixDims = {
    cw: 50,
    fh: 120,
    kn: 30,
    kr: 48,
    kb: 26,
    sh: 24,
    fw: 36,
    fs: 10,
  };
  const d = $derived(dims ?? DEFAULT_DIMS);
  const narrow = $derived(d.cw < 44);
  const TICKS: [number, string][] = [[6, '+6'], [0, '0'], [-12, '−12'], [-40, '−40'], [-60, '−∞']];

  function set<K extends keyof Group>(index: number, key: K, value: Group[K]) {
    onupdategroup(index, { ...groups[index], [key]: value });
  }
  function finishRename(index: number, input: HTMLInputElement, commit: boolean) {
    if (renaming !== index) return;
    renaming = null;
    const name = input.value.trim().toUpperCase();
    if (commit && name && name !== groups[index].name) set(index, 'name', name);
  }
  function focusSelect(node: HTMLInputElement) { node.focus(); node.select(); }
</script>

<div
  class="mixer plate"
  aria-label="Mixer"
  data-label="MIX · LAUNCH CONTROL XL"
  style:--cw="{d.cw}px"
  style:--fh="{d.fh}px"
  style:--kr="{d.kr}px"
  style:--kb="{d.kb}px"
  style:--sh="{d.sh}px"
  style:--fw="{d.fw}px"
  style:--fs="{d.fs.toFixed(1)}px"
>
  <div class="mx-labels">
    <span class="silk">GROUP</span>
    <span class="silk">PAN</span>
    <span class="silk">PITCH</span>
    <span class="silk" title="Low-pass">LPF</span>
    <div class="mx-ticks" title="Volume scale (dB)">
      {#each TICKS as [db, text]}
        <span class="silk" style:top="{9 + (1 - dbToPos(db)) * (d.fh - 18)}px">{text}</span>
      {/each}
    </div>
    <span class="silk">dB</span>
    <span class="silk">MUTE</span>
    <span class="silk">SOLO</span>
  </div>

  <div class="mx-cols">
    {#each groups as group, i}
      <div class="mx-col" data-group={i}>
        {#if renaming === i}
          <div class="scrib screen active">
            <input
              value={group.name}
              maxlength="12"
              aria-label="Group name"
              use:focusSelect
              onkeydown={(e) => { e.stopPropagation(); if (e.key === 'Enter') finishRename(i, e.currentTarget, true); if (e.key === 'Escape') finishRename(i, e.currentTarget, false); }}
              onblur={(e) => finishRename(i, e.currentTarget, true)}
            />
          </div>
        {:else}
          <button
            type="button"
            class="scrib screen active"
            title="{inputs[i] ? `Pads: ${inputs[i].pads}${inputs[i].keysColor ? ` · Keys: ${inputs[i].keysLabel}` : ''}` : `Channel ${i + 1}`} — click for its inputs, double-click to rename"
            aria-haspopup="menu"
            onclick={(e) => { const r = e.currentTarget.getBoundingClientRect(); onchannelmenu?.(i, r.left, r.bottom + 2); }}
            oncontextmenu={(e) => { e.preventDefault(); onchannelmenu?.(i, e.clientX, e.clientY); }}
            ondblclick={() => { onchannelmenu?.(-1, 0, 0); renaming = i; }}
          >{#if inputs[i]?.keysColor}<i class="kdot" style:background={inputs[i].keysColor}></i>{/if}<span class="nm">{groupLegend(i, group.name)}</span></button>
        {/if}
        <Knob ariaLabel="{group.name} pan" value={group.pan} min={-1} max={1} def={0} step={0.02} fine={0.01} size={d.kn} fmt={fmtPan} learnId="group.{i}.pan" onchange={(v) => set(i, 'pan', v)} />
        <Knob ariaLabel="{group.name} pitch" value={group.pitchSemitones} min={-12} max={12} def={0} step={1} fine={0.1} size={d.kn} fmt={fmtSemis} learnId="group.{i}.pitch" onchange={(v) => set(i, 'pitchSemitones', v)} />
        <Knob ariaLabel="{group.name} low-pass" value={lpToKnob(group.lowpassHz)} min={0} max={1} def={1} step={0.01} fine={0.002} size={d.kn} fmt={fmtLp} learnId="group.{i}.lowpass" onchange={(t) => set(i, 'lowpassHz', t >= 0.999 ? 20000 : knobToLp(t))} />
        <Fader label="{group.name} volume" value={group.volumeDb} height={d.fh} meters={[i]} learnId="group.{i}.volume" onchange={(db) => set(i, 'volumeDb', db)} />
        <span class="lcd">{fmtDbShort(group.volumeDb)}</span>
        <button type="button" class="key" class:on={group.mute} class:amber={group.mute} class:learning={isLearning(learn, `group.${i}.mute`)} class:learned={isLearned(learn, `group.${i}.mute`)} data-learn="group.{i}.mute" use:learnable={{ ctx: learn, id: `group.${i}.mute` }} aria-pressed={group.mute} aria-label="{group.name} mute" onclick={() => set(i, 'mute', !group.mute)}>{narrow ? 'M' : 'MUTE'}</button>
        <button type="button" class="key" class:on={group.solo} class:green={group.solo} class:learning={isLearning(learn, `group.${i}.solo`)} class:learned={isLearned(learn, `group.${i}.solo`)} data-learn="group.{i}.solo" use:learnable={{ ctx: learn, id: `group.${i}.solo` }} aria-pressed={group.solo} aria-label="{group.name} solo" onclick={() => set(i, 'solo', !group.solo)}>{narrow ? 'S' : 'SOLO'}</button>
      </div>
    {/each}
  </div>

  <div class="mx-col mx-master">
    <div class="scrib screen active master-scrib"><span class="nm">MASTER</span></div>
    <div></div><div></div><div></div>
    <Fader label="Master volume" value={masterDb} height={d.fh} color="#ff5b1f" meters={['L', 'R']} learnId="master" onchange={onupdatemaster} />
    <span class="lcd">{fmtDbShort(masterDb)}</span>
    <div></div><div></div>
  </div>
</div>

<style>
  .mixer { flex: none; display: flex; gap: 4px; padding: 12px 10px; position: relative; }
  .mx-labels, .mx-col {
    display: grid; grid-template-rows: var(--sh) var(--kr) var(--kr) var(--kr) var(--fh) 16px var(--kb) var(--kb);
    row-gap: 6px; align-items: center; justify-items: center;
  }
  .mx-labels { justify-items: end; width: 30px; }
  .mx-ticks { position: relative; width: 100%; height: 100%; }
  .mx-ticks .silk { position: absolute; right: 2px; transform: translateY(-50%); font-size: 8px; letter-spacing: 0; }
  .mx-cols { display: flex; gap: 2px; }
  .mx-col { width: var(--cw); border-radius: 6px; }
  .mx-master { padding-left: 8px; margin-left: 4px; border-left: 1px solid var(--line); }
  .scrib {
    width: calc(var(--cw) - 2px); height: var(--sh); display: flex; align-items: center; justify-content: center; gap: 5px; padding: 0 2px; border: 0;
    font: 400 calc(var(--fs) - .5px)/1 var(--f-lcd); color: var(--lcd); overflow: hidden; white-space: nowrap; cursor: pointer;
  }
  .scrib .nm { overflow: hidden; text-overflow: ellipsis; }
  .kdot { width: 6px; height: 6px; border-radius: 50%; flex: none; box-shadow: 0 0 0 1px rgba(0,0,0,.6); }
  .scrib input { width: 100%; background: transparent; border: 0; color: #fff; font: inherit; padding: 0; text-align: center; }
  .master-scrib { cursor: default; }
  .mx-col .key { width: calc(var(--cw) - 6px); height: var(--kb); padding: 0; font-size: var(--fs); min-width: 0; }
  .mx-col .lcd, .mx-col :global(.knob-val) { font-size: var(--fs); min-width: 0; }
</style>
