<script lang="ts">
  import { getContext } from 'svelte';
  import { isLearned, isLearning, learnable, LEARN, type LearnCtx } from '../lib/learnable.ts';
  import Fader from './Fader.svelte';
  import Knob from './Knob.svelte';
  import { GROUP_COLORS, groupLegend, type Group } from '../lib/model.ts';
  import { ledToCss } from '../lib/led.ts';
  import { fmtDbShort, fmtLp, fmtPan, fmtSemis, knobToLp, lpToKnob } from '../lib/mix.ts';
  let { groups, masterDb, height, mode, inputs = [], onupdategroup, onupdatemaster, onchannel, onexpand }: {
    groups: Group[]; masterDb: number; height: number; mode: 'slim' | 'strip';
    inputs?: { pads: number; keysColor: string | null; keysLabel: string }[];
    onupdategroup: (index: number, group: Group) => void;
    onupdatemaster: (masterDb: number) => void;
    onchannel: (index: number, x: number, y: number, rename: boolean) => void;
    onexpand?: (x: number, y: number) => void;
  } = $props();
  const learn = getContext<LearnCtx | undefined>(LEARN);
  const faderH = $derived(height - 24 - 22 - 16 - 2 * 20 - 4 * 4 - 180);
  function channel(e: MouseEvent, i: number, rename: boolean) {
    const r = e.currentTarget instanceof HTMLElement ? e.currentTarget.getBoundingClientRect() : null;
    if (r) onchannel(i, r.left, r.bottom + 2, rename);
  }
</script>

{#if mode === 'slim'}
  <div class="mixer plate slim" aria-label="Mixer" data-label="MIX" style:height="{height}px">
    {#each groups as group, i}
      <div class="ms-col" data-group={i}>
        <button class="ms-chip" style:--c={ledToCss(GROUP_COLORS[i])} title="{groupLegend(i, group.name)} — click for name and inputs" aria-haspopup="dialog" onclick={(e) => channel(e, i, false)} ondblclick={(e) => channel(e, i, true)}>{#if inputs[i]?.keysColor}<i class="kdot" style:background={inputs[i].keysColor}></i>{/if}{i + 1}</button>
        <div class="ms-knobs">
          <Knob label="PAN" ariaLabel="{group.name} pan" value={group.pan} min={-1} max={1} def={0} step={0.02} fine={0.01} size={24} fmt={fmtPan} learnId="group.{i}.pan" onchange={(pan) => onupdategroup(i, { ...group, pan })} />
          <Knob label="PITCH" ariaLabel="{group.name} pitch" value={group.pitchSemitones} min={-12} max={12} def={0} step={1} fine={0.1} size={24} fmt={fmtSemis} learnId="group.{i}.pitch" onchange={(pitchSemitones) => onupdategroup(i, { ...group, pitchSemitones })} />
          <Knob label="LPF" ariaLabel="{group.name} low-pass" value={lpToKnob(group.lowpassHz)} min={0} max={1} def={1} step={0.01} fine={0.002} size={24} fmt={(t) => fmtLp(t).replace(' kHz', 'k').replace(' Hz', '')} learnId="group.{i}.lowpass" onchange={(t) => onupdategroup(i, { ...group, lowpassHz: t >= 0.999 ? 20000 : knobToLp(t) })} />
        </div>
        <Fader label="{group.name} volume" value={group.volumeDb} height={faderH} meters={[i]} learnId="group.{i}.volume" onchange={(volumeDb) => onupdategroup(i, { ...group, volumeDb })} />
        <span class="lcd ms-db">{fmtDbShort(group.volumeDb)}</span>
        <button class="key ms-key" class:on={group.mute} class:amber={group.mute} class:learning={isLearning(learn, `group.${i}.mute`)} class:learned={isLearned(learn, `group.${i}.mute`)} data-learn="group.{i}.mute" use:learnable={{ ctx: learn, id: `group.${i}.mute` }} aria-pressed={group.mute} aria-label="{group.name} mute" onclick={() => onupdategroup(i, { ...group, mute: !group.mute })}>M</button>
        <button class="key ms-key" class:on={group.solo} class:green={group.solo} class:learning={isLearning(learn, `group.${i}.solo`)} class:learned={isLearned(learn, `group.${i}.solo`)} data-learn="group.{i}.solo" use:learnable={{ ctx: learn, id: `group.${i}.solo` }} aria-pressed={group.solo} aria-label="{group.name} solo" onclick={() => onupdategroup(i, { ...group, solo: !group.solo })}>S</button>
      </div>
    {/each}
    <div class="ms-col ms-master">
      <span class="silk">MST</span>
      <div class="ms-knobs" aria-hidden="true"></div>
      <Fader label="Master volume" value={masterDb} height={faderH} color="#ff5b1f" meters={['L', 'R']} learnId="master" onchange={onupdatemaster} />
      <span class="lcd ms-db">{fmtDbShort(masterDb)}</span>
    </div>
  </div>
{:else}
  <button class="mixer plate strip" aria-label="Mixer — click to open" data-label="MIX" style:height="{height}px" onclick={(e) => { const r = e.currentTarget.getBoundingClientRect(); onexpand?.(r.left, r.top); }}>
    {#each groups as _, i}<i class="ms-dot" data-chip={i} style:--c={ledToCss(GROUP_COLORS[i])}></i>{/each}
    <span class="ms-meter"><i class="f-fill" data-ch="L"></i><i class="f-fill" data-ch="R"></i></span>
  </button>
{/if}

<style>
  .mixer.slim { flex: none; width: 300px; box-sizing: border-box; display: flex; gap: 2px; padding: 12px 8px; }
  :global(.mix-pop) .mixer.slim { --body-raised: var(--tag); }
  .ms-col { flex: none; width: 28px; display: flex; flex-direction: column; align-items: center; gap: 4px; --fw: 22px; }
  .ms-master { width: 30px; margin-left: 4px; padding-left: 4px; border-left: 1px solid var(--line); --fw: 26px; }
  .ms-master > .silk { height: 22px; line-height: 22px; }
  .ms-chip { width: 26px; height: 22px; border: 0; border-radius: 3px; background: color-mix(in srgb, var(--c) 28%, var(--glass)); box-shadow: inset 0 0 0 1px var(--c); color: var(--lcd); font: 400 11px/22px var(--f-lcd); cursor: pointer; display: flex; align-items: center; justify-content: center; gap: 2px; }
  .kdot { width: 6px; height: 6px; border-radius: 50%; flex: none; }
  .ms-key { width: 26px; height: 20px; min-width: 0; padding: 0; font-size: 10px; }
  .ms-db { font-size: 9.5px; }
  .ms-col > :global(*) { flex-shrink: 0; }
  .ms-knobs { height: 176px; width: 28px; display: flex; flex-direction: column; align-items: center; gap: 8px; padding: 4px 0; box-sizing: border-box; }
  .ms-knobs :global(.knob-wrap) { gap: 3px; }
  .ms-knobs :global(.silk) { font-size: 8px; line-height: 10px; letter-spacing: 0; }
  .ms-knobs :global(.knob-val) { min-width: 0; font-size: 8px; line-height: 10px; }
  .mixer.strip { flex: none; width: 40px; padding: 12px 4px; display: flex; flex-direction: column; align-items: center; gap: 6px; cursor: pointer; border: 0; color: inherit; }
  .ms-dot { width: 10px; height: 10px; border-radius: 50%; background: var(--c); opacity: .16; }
  .ms-meter { position: relative; flex: 1; width: 12px; }
  .ms-meter .f-fill { position: absolute; top: 0; bottom: 0; width: 4px; background: linear-gradient(0deg, var(--ok) 0 72%, var(--warn) 72% 90%, var(--error) 90%); clip-path: inset(calc(100% - var(--m, 0) * 100%) 0 0 0); }
  .ms-meter .f-fill[data-ch="R"] { left: 6px; }
</style>
