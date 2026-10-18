<script lang="ts">
  import type { Group } from '../lib/model.ts';

  let {
    groups,
    masterDb,
    onupdategroup,
    onupdatemaster,
    onstopall,
  }: {
    groups: Group[];
    masterDb: number;
    onupdategroup: (index: number, group: Group) => void;
    onupdatemaster: (masterDb: number) => void;
    onstopall: () => void;
  } = $props();

  function updateGroupParam<K extends keyof Group>(index: number, key: K, value: Group[K]) {
    const updated = {
      ...groups[index],
      [key]: value,
    };
    onupdategroup(index, updated);
  }

  function toggleMute(index: number) {
    updateGroupParam(index, 'mute', !groups[index].mute);
  }

  function toggleSolo(index: number) {
    updateGroupParam(index, 'solo', !groups[index].solo);
  }
</script>

<div class="mixer-console" aria-label="8-Channel Mixer and Master Section">
  <div class="strips-container">
    {#each groups as group, index}
      <div class="channel-strip" class:muted={group.mute} class:soloed={group.solo}>
        <div class="strip-header">
          <span class="silk strip-num">{index + 1}</span>
          <span class="lcd strip-name">{group.name || `CH ${index + 1}`}</span>
        </div>

        <!-- Mute and Solo -->
        <div class="strip-buttons">
          <button
            type="button"
            class="key mute-btn"
            class:active-mute={group.mute}
            onclick={() => toggleMute(index)}
            aria-label="Mute channel {index + 1}"
          >
            M
          </button>
          <button
            type="button"
            class="key solo-btn"
            class:active-solo={group.solo}
            onclick={() => toggleSolo(index)}
            aria-label="Solo channel {index + 1}"
          >
            S
          </button>
        </div>

        <!-- Lowpass Filter Cutoff -->
        <div class="knob-row">
          <label class="silk param-label" for="lp-{index}">LP</label>
          <input
            id="lp-{index}"
            type="range"
            min="200"
            max="20000"
            step="100"
            class="mini-slider"
            value={group.lowpassHz}
            title={`Lowpass: ${group.lowpassHz >= 20000 ? 'Bypass' : `${Math.round(group.lowpassHz)} Hz`}`}
            oninput={(e) => updateGroupParam(index, 'lowpassHz', parseFloat((e.target as HTMLInputElement).value))}
          />
        </div>

        <!-- Pan -->
        <div class="knob-row">
          <label class="silk param-label" for="pan-{index}">PAN</label>
          <input
            id="pan-{index}"
            type="range"
            min="-1"
            max="1"
            step="0.05"
            class="mini-slider"
            value={group.pan}
            title={`Pan: ${group.pan === 0 ? 'C' : group.pan < 0 ? `L${Math.round(Math.abs(group.pan) * 100)}` : `R${Math.round(group.pan * 100)}`}`}
            oninput={(e) => updateGroupParam(index, 'pan', parseFloat((e.target as HTMLInputElement).value))}
          />
        </div>

        <!-- Volume Fader -->
        <div class="fader-container">
          <input
            type="range"
            min="-60"
            max="12"
            step="0.5"
            class="v-fader"
            value={group.volumeDb}
            aria-label="Volume channel {index + 1}"
            oninput={(e) => updateGroupParam(index, 'volumeDb', parseFloat((e.target as HTMLInputElement).value))}
          />
          <div class="fader-scale">
            <span>+12</span>
            <span class="zero-mark">0</span>
            <span>-12</span>
            <span>-30</span>
            <span>-∞</span>
          </div>
        </div>

        <span class="lcd fader-val">
          {group.volumeDb <= -60 ? '-∞' : `${group.volumeDb > 0 ? '+' : ''}${group.volumeDb.toFixed(0)}`}
        </span>
      </div>
    {/each}

    <!-- Master Section -->
    <div class="master-strip">
      <div class="strip-header master-header">
        <span class="silk strip-name">MASTER</span>
      </div>

      <button
        type="button"
        class="key stop-all-btn"
        title="Emergency stop all sounding voices"
        onclick={onstopall}
      >
        STOP ALL
      </button>

      <div class="fader-container">
        <input
          type="range"
          min="-60"
          max="12"
          step="0.5"
          class="v-fader master-fader"
          value={masterDb}
          aria-label="Master volume"
          oninput={(e) => onupdatemaster(parseFloat((e.target as HTMLInputElement).value))}
        />
        <div class="fader-scale">
          <span>+12</span>
          <span class="zero-mark">0</span>
          <span>-12</span>
          <span>-30</span>
          <span>-∞</span>
        </div>
      </div>

      <span class="lcd fader-val master-val">
        {masterDb <= -60 ? '-∞' : `${masterDb > 0 ? '+' : ''}${masterDb.toFixed(1)} dB`}
      </span>
    </div>
  </div>
</div>

<style>
  .mixer-console {
    width: 100%;
    height: 100%;
    display: flex;
    overflow-x: auto;
    overflow-y: hidden;
    padding: 4px;
    background: var(--body);
    border-radius: var(--r-panel);
  }

  .strips-container {
    display: flex;
    gap: 6px;
    height: 100%;
    min-width: min-content;
  }

  .channel-strip {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 48px;
    height: 100%;
    padding: 4px 2px;
    background: var(--body-raised);
    border: 1px solid var(--line);
    border-radius: 3px;
    gap: 4px;
  }

  .master-strip {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 62px;
    height: 100%;
    padding: 4px 2px;
    background: #1c1816;
    border: 1px solid var(--orange);
    border-radius: 3px;
    gap: 4px;
  }

  .strip-header {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    border-bottom: 1px solid var(--line);
    padding-bottom: 2px;
  }

  .strip-num {
    font-size: 11px;
    color: var(--silk);
  }

  .strip-name {
    font-size: 9px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 44px;
    color: var(--lcd-dim);
  }

  .master-header .strip-name {
    color: var(--orange);
    font-weight: 700;
  }

  .strip-buttons {
    display: flex;
    gap: 3px;
  }

  .mute-btn,
  .solo-btn {
    width: 18px;
    height: 16px;
    padding: 0;
    font-size: 9px;
    display: grid;
    place-items: center;
    color: var(--lcd-dim);
  }

  .active-mute {
    background: var(--error) !important;
    color: white !important;
    border-color: #ff6b7b !important;
  }

  .active-solo {
    background: var(--warn) !important;
    color: #110b05 !important;
    border-color: #ffcc55 !important;
  }

  .stop-all-btn {
    width: 52px;
    height: 20px;
    font-size: 8px;
    padding: 0;
    background: #2a0808;
    color: var(--error);
    border-color: #881a1a;
  }

  .stop-all-btn:active {
    background: var(--error);
    color: white;
  }

  .knob-row {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 100%;
    gap: 1px;
  }

  .param-label {
    font-size: 8px;
    color: var(--silk);
  }

  .mini-slider {
    width: 40px;
    height: 4px;
    accent-color: var(--vec);
    cursor: pointer;
  }

  .fader-container {
    display: flex;
    align-items: center;
    gap: 2px;
    flex: 1;
    min-height: 50px;
    position: relative;
  }

  .v-fader {
    writing-mode: vertical-lr;
    direction: rtl;
    width: 18px;
    height: 100%;
    accent-color: var(--orange);
    cursor: pointer;
  }

  .master-fader {
    accent-color: #ff3300;
  }

  .fader-scale {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    height: 100%;
    font-size: 7px;
    color: var(--lcd-dim);
    line-height: 1;
    padding: 2px 0;
  }

  .zero-mark {
    color: var(--vec);
    font-weight: 700;
  }

  .fader-val {
    font-size: 9px;
    color: var(--lcd);
    height: 12px;
  }

  .master-val {
    color: var(--orange);
    font-size: 9px;
    font-weight: 700;
  }

  .channel-strip.muted {
    opacity: 0.6;
  }
</style>
