<script lang="ts">
  // Keys plate (UI spec 4.6, mockup 214–245, 389–398, JS 1226–1352): the 88-key keybed, wheels and
  // the sound / volume / octave / output controls. Keys bound to a pad play that pad.
  import ContextMenu, { type MenuItem } from './ContextMenu.svelte';
  import NumLcd from './NumLcd.svelte';
  import Wheel from './Wheel.svelte';
  import { ledToCss } from '../lib/led.ts';
  import { groupLegend } from '../lib/model.ts';
  import type { SetModel } from '../lib/model.ts';
  import { noteBoundTo } from '../lib/learn.ts';
  import {
    isBlackKey, KEY_HI, KEY_LO, noteName, outputLabel, rangeLabel, SOUNDS, SOUND_COLORS, SOUND_LABELS, velocityFromY, windowRange,
  } from '../lib/keys.ts';
  import type { KeysMix } from '../lib/synth-types.ts';

  let {
    short = false, folded = false, set, octave, deviceName = 'KEYSTATION 49', devicePresent = false, remoteDown = new Set<number>(),
    last = $bindable<{ note: number; velocity: number } | null>(null), wheel = { pitch: 0, mod: 0 },
    onsettings, onnote, onpad, onpitch, onmod, onoctave, onfold,
  }: {
    /** Keybed under 68 px: the third control row is dropped. */
    short?: boolean;
    folded?: boolean;
    set: SetModel;
    octave: number;
    deviceName?: string;
    devicePresent?: boolean;
    /** Notes held on a hardware keyboard (or reported back by the engine). */
    remoteDown?: ReadonlySet<number>;
    last?: { note: number; velocity: number } | null;
    /** Positions of the hardware wheels (pitch −1…1, mod 0…1). */
    wheel?: { pitch: number; mod: number };
    onsettings: (mix: KeysMix) => void;
    /** A key of the screen keybed that is not bound to a pad. */
    onnote: (note: number, velocity: number, on: boolean) => void;
    /** A key bound to a pad: the pad plays. */
    onpad: (bank: number, x: number, y: number, on: boolean) => void;
    onpitch: (value: number) => void;
    onmod: (value: number) => void;
    onoctave: (octave: number) => void;
    onfold: (folded: boolean) => void;
  } = $props();

  const keys = $derived(set.keys);
  const mix = (over: Partial<KeysMix>): KeysMix => ({ gmSound: keys.gmSound, volumeDb: keys.volumeDb, output: keys.output, ...over });
  const range = $derived(windowRange(octave));

  const KEYS: { n: number; black: boolean; left: number; white: number }[] = [];
  let whites = 0;
  for (let n = KEY_LO; n <= KEY_HI; n++) {
    if (isBlackKey(n)) KEYS.push({ n, black: true, left: (whites / 52) * 100, white: whites });
    else { KEYS.push({ n, black: false, left: 0, white: whites }); whites++; }
  }
  const whiteIdx = (n: number) => KEYS.filter((k) => !k.black && k.n < n).length;
  const winLeft = $derived((whiteIdx(range[0]) / 52) * 100);
  const winWidth = $derived(((whiteIdx(range[1]) + 1 - whiteIdx(range[0])) / 52) * 100);
  const soundColor = $derived(ledToCss(SOUND_COLORS[keys.gmSound]));

  /** The pad a key is bound to (any device), with its colour; keys outside the 88 are never asked. */
  const bound = $derived.by(() => {
    const map = new Map<number, { bank: number; x: number; y: number; color: string; sample: string }>();
    for (const b of set.learn ?? []) {
      if (b.kind !== 'note' || !('pad' in b.target) || map.has(b.number)) continue;
      const { bank, x, y } = b.target.pad;
      const pad = set.banks[bank]?.pads.find((p) => p.x === x && p.y === y);
      map.set(b.number, { bank, x, y, color: pad ? ledToCss(pad.color) : '#888', sample: pad?.sample ?? '' });
    }
    return map;
  });

  // Mouse: press = note on (lower on the key is louder), slide = glissando, release = note off.
  let local = $state<Record<number, { pad: { bank: number; x: number; y: number } | null }>>({});
  let cur: number | null = null;
  function noteOn(n: number, velocity: number) {
    const pad = noteBoundTo(set, n);
    local[n] = { pad };
    last = { note: n, velocity };
    if (pad) onpad(pad.bank, pad.x, pad.y, true); else onnote(n, velocity, true);
  }
  function noteOff(n: number) {
    const held = local[n];
    if (!held) return;
    delete local[n];
    if (held.pad) onpad(held.pad.bank, held.pad.x, held.pad.y, false); else onnote(n, 0, false);
  }
  function at(e: PointerEvent) {
    const el = (document.elementFromPoint(e.clientX, e.clientY) as HTMLElement | null)?.closest<HTMLElement>('.kbed [data-n]');
    const n = el ? Number(el.dataset.n) : null;
    if (n === cur) return;
    if (cur !== null) noteOff(cur);
    cur = n;
    if (n !== null && el) {
      const r = el.getBoundingClientRect();
      noteOn(n, velocityFromY(e.clientY - r.top, r.height));
    }
  }
  function release() { if (cur !== null) noteOff(cur); cur = null; }
  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    at(e);
  }

  const isDown = (n: number) => n in local || remoteDown.has(n);
  const keyColor = (n: number) => bound.get(n)?.color ?? soundColor;
  const keyTitle = (n: number) => (bound.has(n) ? `${noteName(n)} → ${bound.get(n)!.sample || 'pad'}` : noteName(n));

  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  function openMenu(e: Event, items: MenuItem[]) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    menu = { x: r.left, y: r.bottom + 4, items };
  }
  const soundMenu = (e: Event) => openMenu(e, SOUNDS.map((sound) => ({ label: SOUND_LABELS[sound], checked: sound === keys.gmSound, action: () => onsettings(mix({ gmSound: sound })) })));
  const outMenu = (e: Event) => openMenu(e, [
    { label: 'MASTER', checked: keys.output === null, action: () => onsettings(mix({ output: null })) },
    ...set.mixer.groups.map((g, i) => ({ label: groupLegend(i, g.name), checked: keys.output === i, action: () => onsettings(mix({ output: i })) })),
  ]);
  const activate = (open: (e: Event) => void) => (e: KeyboardEvent) => {
    if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); e.stopPropagation(); open(e); }
  };
  const fmtVol = (v: number) => (v > 0 ? `+${v}` : v < 0 ? `−${-v}` : '0');
</script>

{#if menu}<ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => { menu = null; }} />{/if}

{#if folded}
  <div class="keys plate folded" aria-label="Keyboard" data-label="KEYS · KEYSTATION 49">
    <button type="button" class="kfold" aria-expanded="false" title="Show keyboard (Ctrl+K)" onclick={() => onfold(false)}>
      <span class="silk">▸ KEYS</span>
      <span class="lamp" class:ok={devicePresent} title="{deviceName} {devicePresent ? 'connected' : 'not connected'}"></span>
      <span class="silk">{deviceName}</span>
    </button>
  </div>
{:else}
  <div class="keys plate" class:short aria-label="Keyboard" data-label="KEYS · KEYSTATION 49">
    <div class="k-wheels">
      <Wheel label="Pitch bend (±2 st)" spring value={wheel.pitch} onchange={onpitch} />
      <Wheel label="Modulation (CC 1): vibrato, learnable" value={wheel.mod} learnId="keys.mod" onchange={onmod} />
    </div>
    <div class="k-ctl">
      <div class="k-row">
        <span class="lamp" class:ok={devicePresent} title="{deviceName} {devicePresent ? 'connected' : 'not connected'}"></span>
        <span class="lcd screen target" role="button" tabindex="0" aria-haspopup="menu" style:color={soundColor}
          title="Keys sound: {SOUND_LABELS[keys.gmSound]} — click to change. Notes learned to a pad play that pad."
          onclick={soundMenu} onkeydown={activate(soundMenu)}>{SOUND_LABELS[keys.gmSound]}</span>
        <NumLcd label="Keys volume (dB)" value={keys.volumeDb} min={-40} max={6} def={-6} fmt={fmtVol} learnId="keys.volume"
          onchange={(v) => onsettings(mix({ volumeDb: v }))} />
        <button type="button" class="key small" aria-label="Fold keyboard" title="Fold (Ctrl+K)" onclick={() => onfold(true)}>▾</button>
      </div>
      <div class="k-row">
        <button type="button" class="key small" aria-label="Octave down" title="Octave down" aria-disabled={octave <= -1} onclick={() => onoctave(octave - 1)}>−</button>
        <span class="lcd range" title="Octave window of the Keystation">{rangeLabel(octave)}</span>
        <button type="button" class="key small" aria-label="Octave up" title="Octave up" aria-disabled={octave >= 2} onclick={() => onoctave(octave + 1)}>+</button>
        <span class="lcd screen out" role="button" tabindex="0" aria-haspopup="menu"
          title={keys.output === null ? 'Keys output: master bus (click to send them through a mixer channel)' : `Keys output: mixer channel ${keys.output + 1} ${set.mixer.groups[keys.output]?.name} — its fader, pan, low-pass, mute and solo apply`}
          onclick={outMenu} onkeydown={activate(outMenu)}>{outputLabel(keys.output, set.mixer.groups)}</span>
      </div>
      <div class="k-row k-last">
        <span class="silk">LAST</span>
        <span class="lcd lcd-dim">{last ? `${noteName(last.note)} · VEL ${last.velocity}` : '—'}</span>
      </div>
    </div>
    <div class="kbed" role="group" aria-label="Keyboard, 88 keys" onpointerdown={down} onpointermove={(e) => { if (e.buttons & 1) at(e); }} onpointerup={release} onpointercancel={release}>
      <div class="felt"><span class="win" style:left="{winLeft}%" style:width="{winWidth}%" title="Keys the Keystation 49 reaches now"></span></div>
      <div class="kwhites">
        {#each KEYS.filter((k) => !k.black) as k (k.n)}
          <div class="kw" class:out={k.n < range[0] || k.n > range[1]} class:down={isDown(k.n)} data-n={k.n} title={keyTitle(k.n)} style:--c={keyColor(k.n)}>
            {#if k.n % 12 === 0}<span class="cn">C{k.n / 12 - 1}</span>{/if}
            {#if bound.has(k.n)}<span class="ldot" style:background={bound.get(k.n)!.color}></span>{/if}
          </div>
        {/each}
      </div>
      {#each KEYS.filter((k) => k.black) as k (k.n)}
        <div class="kb" class:out={k.n < range[0] || k.n > range[1]} class:down={isDown(k.n)} data-n={k.n} title={keyTitle(k.n)} style:left="{k.left}%" style:--c={keyColor(k.n)}>
          {#if bound.has(k.n)}<span class="ldot" style:background={bound.get(k.n)!.color}></span>{/if}
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .keys { display: flex; gap: 10px; padding: 6px 10px; flex: none; }
  .keys.folded { padding: 0; }
  .kfold { appearance: none; border: 0; background: transparent; cursor: pointer; height: 28px; width: 100%; display: flex; align-items: center; gap: 8px; padding: 0 12px; border-radius: var(--r-panel); color: inherit; }
  .kfold:hover .silk { color: var(--lcd); }
  .k-wheels { display: flex; gap: 5px; flex: none; }
  .k-ctl { flex: none; width: 176px; display: flex; flex-direction: column; justify-content: center; gap: 4px; min-width: 0; }
  .k-row { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .k-row .lcd { flex: 1; min-width: 0; height: 20px; line-height: 20px; padding: 0 6px; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .k-row .key.small { width: 22px; height: 20px; min-width: 0; padding: 0; }
  .k-row :global(.num) { flex: none; height: 20px; line-height: 20px; min-width: 34px; text-align: center; }
  .k-row .target, .k-row .out { cursor: pointer; }
  .k-row .range { text-align: center; padding: 0 2px; }
  .k-row .out { flex: none; width: 64px; font-size: 10px; padding: 0 4px; }
  .k-row .silk { font-size: 9.5px; }
  .short .k-last { display: none; }
  .kbed { flex: 1; min-width: 0; height: var(--kh); position: relative; padding-top: 5px; border-radius: 4px; background: var(--glass); box-shadow: inset 0 0 0 1px var(--line); touch-action: none; cursor: pointer; }
  .felt { position: absolute; left: 0; right: 0; top: 0; height: 5px; background: #2a1208; border-radius: 4px 4px 0 0; }
  .win { position: absolute; top: 1px; height: 3px; border-radius: 2px; background: var(--orange); box-shadow: 0 0 6px rgba(255,91,31,.7); }
  .kwhites { display: flex; gap: 1px; height: 100%; }
  .kw { flex: 1; min-width: 0; position: relative; border-radius: 0 0 2px 2px; background: #e9e1cc; box-shadow: inset 0 -2px 0 #bdb49c; }
  .kb { position: absolute; top: 5px; z-index: 1; width: calc(100% / 52 * .6); margin-left: calc(100% / 52 * -.3); height: calc((var(--kh) - 5px) * .62); border-radius: 0 0 2px 2px; background: #101826; box-shadow: inset 0 0 0 1px #33496a; }
  .kw.out { filter: brightness(.6); }
  .kb.out { filter: brightness(.7); }
  .kw.down { background: color-mix(in srgb, var(--c) 55%, #e9e1cc); box-shadow: 0 0 12px color-mix(in srgb, var(--c) 60%, transparent); filter: none; }
  .kb.down { background: var(--c); box-shadow: 0 0 10px var(--c); filter: none; }
  .cn { position: absolute; left: 0; right: 0; bottom: 6px; text-align: center; font: 600 9px/1 var(--f-silk); color: #8a826d; pointer-events: none; }
  .ldot { position: absolute; left: 50%; width: 6px; height: 6px; margin-left: -3px; border-radius: 50%; box-shadow: 0 0 0 1px rgba(0,0,0,.6); pointer-events: none; bottom: 4px; }
  .kw .ldot { bottom: 17px; }
</style>
