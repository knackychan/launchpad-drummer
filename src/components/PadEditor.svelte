<script lang="ts">
  import { LAUNCHPAD_PALETTE } from '../lib/model.ts';
  import type { Pad, PadMode } from '../lib/model.ts';
  import { ledToCss } from '../lib/led.ts';

  let { pad, bankIndex, onupdate, onremove, onclose, ontrigger, onrelease }: {
    pad: Pad;
    bankIndex: number;
    onupdate: (pad: Pad) => void;
    onremove: (x: number, y: number) => void;
    onclose: () => void;
    ontrigger?: (x: number, y: number) => void;
    onrelease?: (x: number, y: number) => void;
  } = $props();

  function updateField<K extends keyof Pad>(key: K, value: Pad[K]) {
    onupdate({
      ...pad,
      [key]: value,
    });
  }

  function handleModeChange(mode: PadMode) {
    updateField('mode', mode);
  }

  function handleColorSelect(color: [number, number, number]) {
    updateField('color', color);
  }

  function handleChokeChange(event: Event) {
    const val = (event.target as HTMLSelectElement).value;
    updateField('chokeGroup', val === 'none' ? null : parseInt(val, 10));
  }

  function handleMixGroupChange(event: Event) {
    const val = parseInt((event.target as HTMLSelectElement).value, 10);
    updateField('mixGroup', val);
  }
</script>

<aside class="pad-editor plate" aria-label="Pad Editor Drawer" data-label="PAD EDITOR">
  <header class="editor-header">
    <div class="pad-badge">
      <span class="silk">BANK {bankIndex + 1}</span>
      <span class="lcd pad-coord">R{pad.y + 1}·C{pad.x + 1}</span>
    </div>
    <div class="header-actions">
      {#if ontrigger && onrelease}
        <button
          type="button"
          class="key small test-hit"
          title="Test trigger pad"
          onpointerdown={() => ontrigger?.(pad.x, pad.y)}
          onpointerup={() => onrelease?.(pad.x, pad.y)}
          onpointerleave={() => onrelease?.(pad.x, pad.y)}
        >
          HIT
        </button>
      {/if}
      <button type="button" class="key small close-btn" onclick={onclose} aria-label="Close Pad Editor">
        ✕
      </button>
    </div>
  </header>

  <div class="editor-body">
    <!-- Sample Name / Path -->
    <div class="field">
      <label for="pad-sample" class="silk">SAMPLE PATH</label>
      <div class="sample-input-row">
        <input
          id="pad-sample"
          class="screen lcd input-text"
          type="text"
          value={pad.sample}
          placeholder="e.g. samples/kick.wav"
          onchange={(e) => updateField('sample', (e.target as HTMLInputElement).value)}
        />
      </div>
    </div>

    <!-- Mode Selector -->
    <div class="field">
      <span class="silk field-label">PLAYBACK MODE</span>
      <div class="switch">
        <button
          type="button"
          class="mode-btn key"
          class:on={pad.mode === 'oneShot'}
          onclick={() => handleModeChange('oneShot')}
        >
          ONE-SHOT
        </button>
        <button
          type="button"
          class="mode-btn key"
          class:on={pad.mode === 'hold'}
          onclick={() => handleModeChange('hold')}
        >
          HOLD
        </button>
      </div>
    </div>

    <!-- Volume and Pitch sliders -->
    <div class="field-row">
      <div class="field flex-1">
        <div class="field-head">
          <label for="pad-vol" class="silk">GAIN</label>
          <span class="lcd readout">{pad.volumeDb > 0 ? `+${pad.volumeDb.toFixed(1)}` : pad.volumeDb.toFixed(1)} dB</span>
        </div>
        <input
          id="pad-vol"
          type="range"
          min="-60"
          max="12"
          step="0.5"
          value={pad.volumeDb}
          oninput={(e) => updateField('volumeDb', parseFloat((e.target as HTMLInputElement).value))}
        />
      </div>

      <div class="field flex-1">
        <div class="field-head">
          <label for="pad-pitch" class="silk">PITCH</label>
          <span class="lcd readout">{pad.pitchSemitones > 0 ? `+${pad.pitchSemitones.toFixed(1)}` : pad.pitchSemitones.toFixed(1)} st</span>
        </div>
        <input
          id="pad-pitch"
          type="range"
          min="-24"
          max="24"
          step="1"
          value={pad.pitchSemitones}
          oninput={(e) => updateField('pitchSemitones', parseFloat((e.target as HTMLInputElement).value))}
        />
      </div>
    </div>

    <!-- Choke and Mix Group -->
    <div class="field-row">
      <div class="field flex-1">
        <label for="pad-choke" class="silk">CHOKE GROUP</label>
        <select id="pad-choke" class="screen lcd select-box" value={pad.chokeGroup ?? 'none'} onchange={handleChokeChange}>
          <option value="none">OFF</option>
          {#each Array(8) as _, i}
            <option value={i + 1}>GROUP {i + 1}</option>
          {/each}
        </select>
      </div>

      <div class="field flex-1">
        <label for="pad-mix" class="silk">MIX BUS</label>
        <select id="pad-mix" class="screen lcd select-box" value={pad.mixGroup} onchange={handleMixGroupChange}>
          {#each Array(8) as _, i}
            <option value={i}>BUS {i + 1}</option>
          {/each}
        </select>
      </div>
    </div>

    <!-- Pad LED Color Palette -->
    <div class="field">
      <span class="silk field-label">PAD COLOR</span>
      <div class="palette-swatches">
        {#each LAUNCHPAD_PALETTE as swatch}
          {@const isSelected = pad.color[0] === swatch[0] && pad.color[1] === swatch[1] && pad.color[2] === swatch[2]}
          <button
            type="button"
            class="swatch"
            class:active={isSelected}
            style:background={ledToCss(swatch)}
            aria-label="Color RGB {swatch[0]} {swatch[1]} {swatch[2]}"
            onclick={() => handleColorSelect([...swatch])}
          ></button>
        {/each}
      </div>
    </div>

    <!-- Actions -->
    <div class="field-actions">
      <button
        type="button"
        class="key remove-btn"
        onclick={() => onremove(pad.x, pad.y)}
      >
        CLEAR PAD
      </button>
    </div>
  </div>
</aside>

<style>
  .pad-editor {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 12px 12px;
    width: 270px;
    min-width: 250px;
    background: var(--body-raised);
    border-radius: var(--r-panel);
    border: 1px solid var(--line);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6);
  }

  .editor-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--line);
    padding-bottom: 8px;
  }

  .pad-badge {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .pad-coord {
    font-size: 15px;
    color: var(--silk);
    font-weight: 700;
  }

  .header-actions {
    display: flex;
    gap: 6px;
  }

  .test-hit {
    font-size: 10px;
    width: 36px;
    height: 22px;
    color: var(--lcd);
  }

  .close-btn {
    width: 22px;
    height: 22px;
    padding: 0;
    display: grid;
    place-items: center;
    font-size: 11px;
    color: var(--lcd-dim);
  }

  .editor-body {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .field-row {
    display: flex;
    gap: 8px;
  }

  .flex-1 {
    flex: 1;
    min-width: 0;
  }

  .field-label {
    margin-bottom: 2px;
  }

  .readout {
    font-size: 11px;
    color: var(--vec);
  }

  .input-text {
    width: 100%;
    height: 26px;
    padding: 2px 8px;
    border: 1px solid var(--line);
    color: var(--lcd);
    font-size: 12px;
  }

  .select-box {
    width: 100%;
    height: 26px;
    padding: 2px 6px;
    border: 1px solid var(--line);
    color: var(--lcd);
    font-size: 11px;
    background: var(--glass);
  }

  .switch {
    display: flex;
    gap: 4px;
    background: #110f0e;
    padding: 2px;
    border-radius: var(--r-key);
    border: 1px solid var(--line);
  }

  .mode-btn {
    flex: 1;
    height: 24px;
    font-size: 10px;
    padding: 0;
  }

  input[type='range'] {
    width: 100%;
    accent-color: var(--orange);
    height: 6px;
    cursor: pointer;
  }

  .palette-swatches {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 4px;
  }

  .swatch {
    width: 100%;
    aspect-ratio: 1;
    border: 1px solid rgba(0, 0, 0, 0.6);
    border-radius: 2px;
    cursor: pointer;
    padding: 0;
    transition: transform 60ms;
  }

  .swatch:hover {
    transform: scale(1.15);
    z-index: 2;
  }

  .swatch.active {
    outline: 2px solid white;
    outline-offset: 1px;
  }

  .field-actions {
    margin-top: 6px;
    display: flex;
    justify-content: flex-end;
  }

  .remove-btn {
    font-size: 10px;
    padding: 4px 10px;
    color: var(--error);
    border-color: rgba(255, 61, 79, 0.4);
  }
</style>
