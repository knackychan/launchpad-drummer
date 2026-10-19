<script lang="ts">
  // Sequencer plate (UI spec 4.7, mockup 246–275, 399–407, JS 1353–1451). The clock runs in the
  // Rust router; this plate edits the pattern and shows the playhead from `seq-position`.
  import { getContext } from 'svelte';
  import NumLcd from './NumLcd.svelte';
  import { isLearned, isLearning, learnable, LEARN, type LearnCtx } from '../lib/learnable.ts';
  import { ledToCss } from '../lib/led.ts';
  import { BPM, clickValue, foldSummary, laneLabel, paint, SWING, setLength, type StepValue } from '../lib/seq.ts';
  import type { Bank, Pattern, SeqPosition } from '../lib/model.ts';

  let { pattern, banks, selected, activeBank, position, playing, rec, folded, onplay, onrec, onupdate, onaddlane, onfold, onselect, onaudition }: {
    pattern: Pattern;
    banks: Bank[];
    /** The selected pad of the active bank. */
    selected: { x: number; y: number } | null;
    activeBank: number;
    position: SeqPosition | null;
    playing: boolean;
    rec: boolean;
    folded: boolean;
    onplay: () => void;
    onrec: () => void;
    /** `key` folds a drag into one undo step (`seq:bpm`, `seq:swing`, `seq:paint`). */
    onupdate: (pattern: Pattern, key?: string) => void;
    onaddlane: () => void;
    onfold: (folded: boolean) => void;
    onselect: (bank: number, x: number, y: number) => void;
    onaudition: (bank: number, x: number, y: number) => void;
  } = $props();

  const learn = getContext<LearnCtx | undefined>(LEARN);
  const LENGTHS = [4, 8, 16, 32] as const;
  const padOf = (bank: number, x: number, y: number) => banks[bank]?.pads.find(p => p.x === x && p.y === y) ?? null;
  const lanes = $derived(pattern.lanes.map((lane) => {
    const pad = padOf(lane.bank, lane.x, lane.y);
    const sample = pad?.sample || null;
    return {
      lane, sample, color: pad && sample ? ledToCss(pad.color) : '#3a3c3f',
      label: laneLabel(lane.bank, sample),
      sel: !!selected && lane.bank === activeBank && lane.x === selected.x && lane.y === selected.y,
    };
  }));
  const head = $derived(playing && position ? position.step : -1);

  function editLane(index: number, change: (lane: Pattern['lanes'][number]) => Pattern['lanes'][number] | null, key?: string) {
    const next = pattern.lanes.flatMap((lane, i) => i === index ? (change(lane) ?? []) : [lane]);
    onupdate({ ...pattern, lanes: next }, key);
  }

  // Painting: pressing a cell sets the value `clickValue` gives for it; dragging across the lane
  // paints the same value over the cells under the pointer. One update per changed cell.
  let stroke: { lane: number; value: StepValue } | null = null;
  function cellAt(e: PointerEvent, el: HTMLElement): number | null {
    const r = el.getBoundingClientRect();
    const i = Math.floor(((e.clientX - r.left) / r.width) * pattern.length);
    return i >= 0 && i < pattern.length ? i : null;
  }
  function setStep(li: number, i: number, value: StepValue) {
    const lane = pattern.lanes[li];
    const before = lane.steps[i] as StepValue;
    if (before === value) return;
    editLane(li, (l) => ({ ...l, steps: paint(l.steps as StepValue[], i, value) }), 'seq:paint');
    if (before === 0 && !playing) onaudition(lane.bank, lane.x, lane.y);
  }
  function down(e: PointerEvent, li: number) {
    if (e.button !== 0 && e.button !== 2) return;
    const el = e.currentTarget as HTMLElement;
    const i = cellAt(e, el);
    if (i === null) return;
    e.preventDefault();
    el.setPointerCapture(e.pointerId);
    const value = clickValue(pattern.lanes[li].steps[i] as StepValue, e.button === 2 || e.shiftKey);
    stroke = { lane: li, value };
    setStep(li, i, value);
  }
  function move(e: PointerEvent, li: number) {
    if (!stroke || stroke.lane !== li) return;
    const i = cellAt(e, e.currentTarget as HTMLElement);
    if (i !== null) setStep(li, i, stroke.value);
  }
  const end = () => { stroke = null; };

  function clear() {
    onupdate({ ...pattern, lanes: pattern.lanes.map(l => ({ ...l, steps: l.steps.map(() => 0) })) });
  }
</script>

{#if folded}
  <div class="seq plate folded" aria-label="Sequencer" data-label="SEQUENCER · STEP GRID">
    <button type="button" class="kfold" aria-expanded="false" title="Show sequencer" onclick={() => onfold(false)}>
      <span class="silk">▸ SEQUENCER</span>
      <span class="lcd lcd-dim sum">{foldSummary(pattern.lanes.length, pattern.length, pattern.bpm)}{playing ? ' · playing' : ''}</span>
    </button>
  </div>
{:else}
  <div class="seq plate" aria-label="Sequencer" data-label="SEQUENCER · STEP GRID">
    <div class="sq-head">
      <div class="sq-left">
        <button type="button" class="key round" class:on={playing} class:learning={isLearning(learn, 'seq.play')} class:learned={isLearned(learn, 'seq.play')}
          data-learn="seq.play" use:learnable={{ ctx: learn, id: 'seq.play' }} aria-pressed={playing}
          aria-label={playing ? 'Stop sequencer' : 'Play sequencer'} title="Play / stop the pattern (Esc stops everything)" onclick={onplay}>{playing ? '■' : '▶'}</button>
        <button type="button" class="key round rec" class:on={rec} aria-pressed={rec} aria-label="Record"
          title="Record: pads you play while the pattern runs are written to the nearest step (starts playback)" onclick={onrec}>●</button>
        <span class="silk">BPM</span>
        <NumLcd label="Tempo" value={pattern.bpm} min={BPM.min} max={BPM.max} def={BPM.def} learnId="seq.bpm"
          onchange={(v) => onupdate({ ...pattern, bpm: v }, 'seq:bpm')} />
        <span class="silk">SWING</span>
        <NumLcd label="Swing" value={pattern.swing} min={SWING.min} max={SWING.max} def={SWING.def} fmt={(v) => `${v}%`} learnId="seq.swing"
          onchange={(v) => onupdate({ ...pattern, swing: v }, 'seq:swing')} />
      </div>
      <span class="silk">STEPS</span>
      <div class="switch" role="radiogroup" aria-label="Pattern length">
        {#each LENGTHS as n}
          <button type="button" class="key" class:on={pattern.length === n} role="radio" aria-checked={pattern.length === n}
            onclick={() => { if (pattern.length !== n) onupdate(setLength(pattern, n)); }}>{n}</button>
        {/each}
      </div>
      <button type="button" class="key" title="Add the selected pad as a lane" onclick={onaddlane}>+ LANE</button>
      <button type="button" class="key" title="Clear all steps (undoable)" onclick={clear}>CLEAR</button>
      <span class="spacer"></span>
      <button type="button" class="key small" aria-label="Fold sequencer" title="Fold" onclick={() => onfold(true)}>▾</button>
    </div>
    <div class="sq-body">
      {#if lanes.length === 0}<div class="sq-hint">No lanes. Select a pad and press + LANE.</div>{/if}
      {#each lanes as { lane, sample, color, label, sel }, li (`${lane.bank}:${lane.x}:${lane.y}`)}
        <div class="sq-lane" class:muted={lane.mute} class:gone={!sample}>
          <div class="sq-lh">
            <span class="sw" style:background={color}></span>
            <button type="button" class="lcd screen lane-name" class:sel title={sample ? `Bank ${lane.bank + 1} R${lane.y + 1}·C${lane.x + 1} — click to select the pad` : 'Pad is empty now'}
              onclick={() => onselect(lane.bank, lane.x, lane.y)}>{label}</button>
            <button type="button" class="key small" class:on={lane.mute} class:amber={lane.mute} aria-pressed={lane.mute} aria-label="Mute lane" title="Mute lane"
              onclick={() => editLane(li, (l) => ({ ...l, mute: !l.mute }))}>M</button>
            <button type="button" class="key small" aria-label="Remove lane" title="Remove lane" onclick={() => editLane(li, () => null)}>×</button>
          </div>
          <div class="sq-steps" role="group" aria-label="Steps of {label}" style:--n={pattern.length}
            onpointerdown={(e) => down(e, li)} onpointermove={(e) => move(e, li)} onpointerup={end} onpointercancel={end}
            oncontextmenu={(e) => e.preventDefault()}>
            {#each Array.from({ length: pattern.length }, (_, i) => i) as i}
              {@const v = lane.steps[i]}
              <div class="st" class:beat={i % 4 === 0} class:on={v === 1} class:acc={v === 2} class:ph={i === head} style:--c={color}></div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .seq { display: flex; flex-direction: column; gap: 6px; padding: 6px 10px; height: var(--sq, 120px); min-height: 0; flex: none; }
  .seq.folded { padding: 0; height: 28px; }
  .kfold { appearance: none; border: 0; background: transparent; cursor: pointer; height: 28px; width: 100%; display: flex; align-items: center; gap: 8px; padding: 0 12px; border-radius: var(--r-panel); color: inherit; }
  .kfold:hover .silk { color: var(--lcd); }
  .sum { font-size: 11px; }
  .sq-head { flex: none; display: flex; align-items: center; gap: 8px; height: 22px; }
  .sq-head .key { height: 22px; display: inline-flex; align-items: center; }
  .sq-head .key.round { width: 22px; height: 22px; font-size: 10px; justify-content: center; }
  .sq-head .key.round.on { background: linear-gradient(var(--cap1), var(--cap2)); color: var(--ok); text-shadow: 0 0 8px var(--ok); --lip: #04070d; --edge: var(--ok); --halo: 0 0 10px rgba(92,255,157,.3); }
  .sq-head .key.round.rec.on { background: linear-gradient(#ff6b79, #ff3d4f 60%, #ee2c40); color: #1c0307; text-shadow: none; --lip: #8a1220; --edge: rgba(255,255,255,.12); --halo: 0 0 12px rgba(255,61,79,.5); }
  .sq-left { width: 223px; flex: none; display: flex; align-items: center; gap: 6px; }
  .switch .key { height: 20px; padding: 0 8px; }
  .spacer { flex: 1; }
  .sq-body { flex: 1; min-height: 0; overflow-y: auto; display: flex; flex-direction: column; gap: 3px; scrollbar-width: thin; scrollbar-color: var(--line-hi) transparent; }
  .sq-hint { padding: 4px 2px; font-size: 11px; color: var(--lcd-dim); }
  .sq-lane { flex: none; display: flex; gap: 10px; height: var(--lh, 18px); }
  .sq-lh { width: 223px; flex: none; display: flex; align-items: center; gap: 5px; min-width: 0; }
  .sq-lh .sw { width: 4px; height: calc(var(--lh, 18px) - 4px); border-radius: 2px; flex: none; }
  .sq-lh .lane-name { flex: 1; min-width: 0; height: var(--lh, 18px); line-height: var(--lh, 18px); padding: 0 6px; border: 0; text-align: left; font-size: 10.5px; color: var(--lcd-dim); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; cursor: pointer; }
  .sq-lh .lane-name.sel { color: var(--lcd); }
  .sq-lane.gone .lane-name { font-style: italic; }
  .sq-lh .key.small { width: 20px; min-width: 0; height: min(18px, var(--lh, 18px)); padding: 0; font-size: 9px; display: grid; place-items: center; }
  .sq-steps { flex: 1; min-width: 0; display: grid; grid-template-columns: repeat(var(--n), 1fr); gap: 2px; touch-action: none; cursor: pointer; }
  .st { border-radius: 2px; background: var(--st); box-shadow: inset 0 0 0 1px var(--stl), 0 3px 0 #0b0908, 0 5px 6px rgba(0,0,0,.45); }
  .st.beat { background: var(--stb); }
  .st.on { background: color-mix(in srgb, var(--c) 48%, #000); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--c) 70%, transparent), 0 3px 0 #0b0908, 0 5px 6px rgba(0,0,0,.45); }
  .st.acc { background: var(--c); box-shadow: inset 0 1px 0 rgba(255,255,255,.35), 0 3px 0 color-mix(in srgb, var(--c) 40%, #000), 0 5px 6px rgba(0,0,0,.45), 0 0 8px color-mix(in srgb, var(--c) 55%, transparent); }
  .st.ph { box-shadow: inset 0 0 0 1px var(--playhead), 0 3px 0 #0b0908, 0 0 8px rgba(255,91,31,.55); }
  .st.ph.on, .st.ph.acc { filter: brightness(1.45); }
  .sq-lane.muted .st.on, .sq-lane.muted .st.acc { opacity: .3; }
</style>
