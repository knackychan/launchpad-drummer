<script lang="ts">
  import { onMount } from 'svelte';
  import Plate from './components/Plate.svelte';
  import HazardCap from './components/HazardCap.svelte';
  import PadGrid from './components/PadGrid.svelte';
  import PadEditor from './components/PadEditor.svelte';
  import MixerStrip from './components/MixerStrip.svelte';
  import CrtLayers from './components/CrtLayers.svelte';
  import SetupDisplay from './components/SetupDisplay.svelte';
  import HordeGame from './components/HordeGame.svelte';
  import ConsoleBackdrop from './components/ConsoleBackdrop.svelte';
  import { applyLedUpdates, createLedFrame } from './lib/led.ts';
  import type { PadInput, Rgb } from './lib/led.ts';
  import { CRT_FACTORS, readDisplay, saveDisplay } from './lib/display.ts';
  import type { CrtLevel } from './lib/display.ts';
  import { createDefaultPad } from './lib/model.ts';
  import type { AudioStatus, Group, LightModes, Pad, SetModel } from './lib/model.ts';

  let mode = $state<'play' | 'games' | 'setup'>('play');
  let game = $state('horde');
  let deviceState = $state('disconnected');
  let midiLog = $state<{ time: string; text: string }[]>([]);
  let crt = $state<CrtLevel>('low');
  let leds = $state<readonly Rgb[]>(createLedFrame());
  let selected = $state<number | null>(null);

  // App & Audio State
  let currentSet = $state<SetModel | null>(null);
  let activeBank = $state<number>(0);
  let setName = $state<string>('DEFAULT SET');
  let audioStatus = $state<AudioStatus | null>(null);

  const selectedPad = $derived.by<Pad | null>(() => {
    if (selected === null || !currentSet) return null;
    const x = (selected % 10) - 1;
    const y = Math.floor(selected / 10) - 1;
    if (x < 0 || x > 7 || y < 0 || y > 7) return null;
    const bank = currentSet.banks[activeBank];
    if (!bank) return null;
    const existing = bank.pads.find(p => p.x === x && p.y === y);
    return existing ?? createDefaultPad(x, y);
  });

  function logMidi(text: string) {
    midiLog = [{ time: new Date().toLocaleTimeString(), text }, ...midiLog].slice(0, 40);
  }

  function sendFrame(frame: readonly (readonly [number, number, number])[]) {
    leds = frame.map(([r, g, b]) => [r, g, b] as const);
    try {
      void window.__TAURI__?.core.invoke('send_led_frame', { frame }).catch(() => {});
    } catch {
      /* Browser preview has no MIDI bridge. */
    }
  }

  async function loadInitialState() {
    if (!window.__TAURI__?.core) return;
    try {
      const [set, bank, status] = await Promise.all([
        window.__TAURI__.core.invoke<SetModel>('get_current_set'),
        window.__TAURI__.core.invoke<number>('get_current_bank'),
        window.__TAURI__.core.invoke<AudioStatus>('get_audio_status'),
      ]);
      currentSet = set;
      activeBank = bank;
      audioStatus = status;
    } catch (err) {
      console.warn('Failed to load initial state:', err);
    }
  }

  onMount(() => {
    try {
      crt = readDisplay(window.localStorage);
    } catch {
      crt = 'low';
    }

    void loadInitialState();

    const api = window.__TAURI__?.event;
    if (!api) return;

    let unlistenStatus = () => {};
    let unlistenPad = () => {};
    let unlistenFrame = () => {};

    void api.listen<{ state: string; message: string }>('launchpad-status', (event) => {
      deviceState = event.payload.state;
      logMidi(event.payload.message);
    }).then((fn) => (unlistenStatus = fn));

    void api.listen<{ note: number; pressed: boolean }>('launchpad-pad', (event) => {
      logMidi(`PAD ${event.payload.note} ${event.payload.pressed ? 'DOWN' : 'UP'}`);
      padInput({ ...event.payload, source: 'hardware' });
    }).then((fn) => (unlistenPad = fn));

    void api.listen<[number, number, number][]>('launchpad-frame', (event) => {
      if (mode !== 'games') {
        leds = event.payload.map(([r, g, b]) => [r, g, b] as const);
      }
    }).then((fn) => (unlistenFrame = fn));

    return () => {
      unlistenStatus();
      unlistenPad();
      unlistenFrame();
    };
  });

  let hordeComponent = $state<{ padInput: (note: number, pressed: boolean) => void } | undefined>();

  function setCrt(level: CrtLevel) {
    crt = level;
    try {
      saveDisplay(window.localStorage, level);
    } catch {
      /* The switch still works without storage. */
    }
  }

  function padInput(input: PadInput) {
    if (mode === 'games' && game === 'horde') {
      hordeComponent?.padInput(input.note, input.pressed);
      return;
    }
    if (input.note % 10 === 9) {
      // Side buttons handled by hardware or direct bank clicks
      return;
    }

    const x = (input.note % 10) - 1;
    const y = Math.floor(input.note / 10) - 1;

    if (input.pressed) {
      selected = input.note;
      void window.__TAURI__?.core.invoke('trigger_pad', { x, y }).catch(() => {});
    } else {
      void window.__TAURI__?.core.invoke('release_pad', { x, y }).catch(() => {});
    }

    // In web preview fallback without backend painter:
    if (!window.__TAURI__?.core) {
      sendFrame(applyLedUpdates(leds, [{ note: input.note, rgb: input.pressed ? [63, 63, 63] : [0, 0, 0] }]));
    }
  }

  async function chooseBank(bank: number) {
    activeBank = bank;
    try {
      await window.__TAURI__?.core.invoke('select_bank', { bank });
    } catch (e) {
      console.warn('Bank select error:', e);
    }
  }

  async function editBankName(index: number) {
    if (!currentSet?.banks[index]) return;
    const currentName = currentSet.banks[index].name;
    const newName = window.prompt(`Rename Bank ${index + 1}:`, currentName);
    if (newName && newName.trim() && newName !== currentName) {
      currentSet.banks[index].name = newName.trim();
      await window.__TAURI__?.core.invoke('rename_bank', { bank: index, name: newName.trim() });
    }
  }

  async function toggleLightMode(modeKey: keyof LightModes, value: boolean) {
    if (!currentSet?.banks[activeBank]) return;
    currentSet.banks[activeBank].lights[modeKey] = value;
    const lights = currentSet.banks[activeBank].lights;
    await window.__TAURI__?.core.invoke('update_bank_lights', { bank: activeBank, lights });
  }

  async function updateBankHumanize(humanizeDb: number) {
    if (!currentSet?.banks[activeBank]) return;
    currentSet.banks[activeBank].humanizeDb = humanizeDb;
    await window.__TAURI__?.core.invoke('update_bank_humanize', {
      bank: activeBank,
      humanizeDb,
      humanize_db: humanizeDb,
    });
  }

  async function handlePadUpdate(pad: Pad) {
    if (!currentSet) return;
    const bank = currentSet.banks[activeBank];
    const existingIndex = bank.pads.findIndex((p) => p.x === pad.x && p.y === pad.y);
    if (existingIndex >= 0) {
      bank.pads[existingIndex] = pad;
    } else {
      bank.pads.push(pad);
    }
    await window.__TAURI__?.core.invoke('update_pad', { bank: activeBank, pad });
  }

  async function handlePadRemove(x: number, y: number) {
    if (!currentSet) return;
    const bank = currentSet.banks[activeBank];
    bank.pads = bank.pads.filter((p) => !(p.x === x && p.y === y));
    await window.__TAURI__?.core.invoke('remove_pad', { bank: activeBank, x, y });
  }

  async function handleUpdateGroup(groupIndex: number, group: Group) {
    if (!currentSet) return;
    currentSet.mixer.groups[groupIndex] = group;
    await window.__TAURI__?.core.invoke('update_group', {
      group: groupIndex,
      groupData: group,
      group_data: group,
    });
  }

  async function handleUpdateMaster(masterDb: number) {
    if (!currentSet) return;
    currentSet.mixer.masterDb = masterDb;
    await window.__TAURI__?.core.invoke('update_master_db', {
      masterDb,
      master_db: masterDb,
    });
  }

  async function handleStopAll() {
    await window.__TAURI__?.core.invoke('stop_all');
  }

  async function promptSetManagement() {
    const action = window.prompt("Set Action: type 'load', 'new', or 'save':", 'save');
    if (!action) return;
    if (action.toLowerCase() === 'save') {
      await saveSet();
    } else if (action.toLowerCase() === 'new') {
      const dir = window.prompt('New set folder path:');
      if (dir) {
        try {
          currentSet = (await window.__TAURI__?.core.invoke<SetModel>('new_set_dir', { dir })) ?? null;
          setName = dir.split(/[\\/]/).pop() || dir;
        } catch (e) {
          window.alert(`Error creating set: ${e}`);
        }
      }
    } else if (action.toLowerCase() === 'load') {
      const dir = window.prompt('Set folder path to load:');
      if (dir) {
        try {
          currentSet = (await window.__TAURI__?.core.invoke<SetModel>('load_set_dir', { dir })) ?? null;
          setName = dir.split(/[\\/]/).pop() || dir;
        } catch (e) {
          window.alert(`Error loading set: ${e}`);
        }
      }
    }
  }

  async function saveSet() {
    try {
      await window.__TAURI__?.core.invoke('save_set_dir');
      logMidi('Set saved successfully');
    } catch (e) {
      console.warn('Save set info:', e);
    }
  }
</script>

<div class="instrument" data-crt={crt} style:--crt={CRT_FACTORS[crt]}>
  <div class="console">
    <ConsoleBackdrop />
    <header class="top-strip">
      <div class="brand" aria-label="Launchpad Drummer">LPD·01</div>
      <button class="set-lcd screen" onclick={promptSetManagement} title="Set management: click to save or load set">
        <span class="silk">SET</span><span class="lcd">{setName}</span>
      </button>
      <button class="key small" aria-label="Undo" disabled>
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M7 4 3 8l4 4M3 8h8a5 5 0 0 1 0 10" /></svg>
      </button>
      <button class="key small" aria-label="Redo" disabled>
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m13 4 4 4-4 4m4-4H9a5 5 0 0 0 0 10" /></svg>
      </button>
      <div class="notice screen lcd" role="status">
        {deviceState === 'connected'
          ? 'Launchpad MK2 connected'
          : deviceState === 'busy'
            ? 'Launchpad port busy — close other MIDI apps'
            : deviceState === 'bootloader'
              ? 'Launchpad bootloader — run updater'
              : 'Launchpad not connected — on-screen pads available'}
      </div>
    </header>

    <nav class="mode-rail" aria-label="Mode">
      <button
        class="key mode-key"
        class:on={mode === 'play'}
        aria-current={mode === 'play' ? 'page' : undefined}
        onclick={() => { mode = 'play'; }}
      >
        PLAY
      </button>
      <button class="key mode-key blank" aria-disabled="true" aria-label="TRAIN — not fitted yet" title="TRAIN — not fitted yet"></button>
      <button
        class="key mode-key"
        class:on={mode === 'games'}
        aria-current={mode === 'games' ? 'page' : undefined}
        onclick={() => { mode = 'games'; }}
      >
        GAMES
      </button>
      {#each ['BAND'] as name}
        <button class="key mode-key blank" aria-disabled="true" aria-label="{name} — not fitted yet" title="{name} — not fitted yet"></button>
      {/each}
      <div class="rail-spacer"></div>
      <button
        class="key mode-key"
        class:on={mode === 'setup'}
        aria-current={mode === 'setup' ? 'page' : undefined}
        onclick={() => { mode = 'setup'; }}
      >
        SETUP
      </button>
    </nav>

    <main class="bay" aria-label={mode === 'play' ? 'Play' : mode === 'games' ? 'Games' : 'Setup'}>
      {#if mode === 'play'}
        <div class="play-stage">
          <div class="pair">
            <Plate label="GRID · LAUNCHPAD MK2" class="launchpad">
              <div class="top-buttons">
                {#each Array(7) as _}
                  <span class="round-cap" aria-hidden="true"></span>
                {/each}
                <button
                  type="button"
                  class="round-cap mixer-stop-cap"
                  title="Mixer Panic: Stop All Voices"
                  onclick={handleStopAll}
                  aria-label="Stop all sounding voices"
                ></button>
              </div>

              <div class="grid-with-side">
                <PadGrid {leds} {selected} oninput={padInput} />
                <div class="side-buttons" role="group" aria-label="Bank selector">
                  {#each Array(8) as _, index}
                    <button
                      type="button"
                      class="bank bank-btn"
                      class:active-bank-row={index === activeBank}
                      onclick={() => chooseBank(index)}
                      ondblclick={() => editBankName(index)}
                      title={`Bank ${index + 1}: ${currentSet?.banks[index]?.name ?? ''} (Double-click to rename)`}
                      aria-pressed={index === activeBank}
                    >
                      <span class="round-cap" class:bank-active={index === activeBank}></span>
                      <span class="screen lcd" class:lcd-dim={index !== activeBank}>
                        {currentSet?.banks[index]?.name ?? `BANK ${index + 1}`}
                      </span>
                    </button>
                  {/each}
                </div>
              </div>

              <!-- Bank Light Controls & Status Footer -->
              <div class="grid-footer silk">
                <div class="footer-left">
                  <span>GRID MIRROR <span class="lcd lcd-dim">{selected === null ? '—' : `R${Math.floor(selected / 10)}·C${selected % 10}`}</span></span>
                  {#if currentSet?.banks[activeBank]}
                    <div class="light-toggles" role="group" aria-label="Active bank light modes">
                      <label class="toggle-label" title="White flash decaying on pad hit">
                        <input
                          type="checkbox"
                          checked={currentSet.banks[activeBank].lights.flash}
                          onchange={(e) => toggleLightMode('flash', (e.target as HTMLInputElement).checked)}
                        />
                        FLASH
                      </label>
                      <label class="toggle-label" title="Dim inactive pads, brighten active sounding voices">
                        <input
                          type="checkbox"
                          checked={currentSet.banks[activeBank].lights.playState}
                          onchange={(e) => toggleLightMode('playState', (e.target as HTMLInputElement).checked)}
                        />
                        PLAY
                      </label>
                      <label class="toggle-label" title="Expanding concentric rings">
                        <input
                          type="checkbox"
                          checked={currentSet.banks[activeBank].lights.ripple}
                          onchange={(e) => toggleLightMode('ripple', (e.target as HTMLInputElement).checked)}
                        />
                        RIPPLE
                      </label>
                      <label class="toggle-label" title="Random velocity humanization (0 to 6 dB)">
                        HUM:
                        <input
                          type="range"
                          min="0"
                          max="6"
                          step="0.1"
                          style="width: 38px; height: 4px;"
                          value={currentSet.banks[activeBank].humanizeDb ?? 0}
                          oninput={(e) => updateBankHumanize(parseFloat((e.target as HTMLInputElement).value))}
                        />
                      </label>
                    </div>
                  {/if}
                </div>
              </div>
            </Plate>

            <!-- Pad Editor Drawer (when a pad is selected) -->
            {#if selectedPad && currentSet && selected !== null}
              {@const px = (selected % 10) - 1}
              {@const py = Math.floor(selected / 10) - 1}
              <PadEditor
                x={px}
                y={py}
                pad={selectedPad}
                groups={currentSet.mixer.groups}
                peaks={null}
                onupdate={handlePadUpdate}
                onremove={handlePadRemove}
                ontrigger={(x, y) => window.__TAURI__?.core.invoke('trigger_pad', { x, y })}
                onrelease={(x, y) => window.__TAURI__?.core.invoke('release_pad', { x, y })}
              />
            {/if}

            {#if currentSet}
              <MixerStrip
                groups={currentSet.mixer.groups}
                masterDb={currentSet.mixer.masterDb}
                onupdategroup={handleUpdateGroup}
                onupdatemaster={handleUpdateMaster}
                onstopall={handleStopAll}
              />
            {:else}
              <Plate label="MIX · LAUNCH CONTROL XL" class="mixer-cap">
                <HazardCap label="Mixer" />
              </Plate>
            {/if}
          </div>

          <Plate label="KEYS · KEYSTATION 49" class="keys-cap"><HazardCap label="Keyboard" /></Plate>
          <Plate label="SEQUENCER · STEP GRID" class="sequencer-cap"><HazardCap label="Sequencer" /></Plate>
        </div>
      {:else if mode === 'games'}
        <div class="games-stage">
          <aside class="game-picker plate" aria-label="Game picker" data-label="GAMES">
            <h1 class="silk">SELECT GAME</h1>
            {#each [{ id: 'horde', title: 'HORDE', desc: 'HOLD THE BEACH · PLAYABLE' }, { id: 'chess', title: 'CHESS', desc: 'PROTOTYPE' }, { id: 'xiangqi', title: 'XIANGQI', desc: 'PROTOTYPE' }, { id: 'cockpit', title: 'COCKPIT', desc: 'PROTOTYPE' }] as entry}
              <button
                class="game-choice key"
                class:on={game === entry.id}
                onclick={() => { game = entry.id; }}
              >
                <span>{entry.title}</span><small class="lcd lcd-dim">{entry.desc}</small>
              </button>
            {/each}
          </aside>
          <Plate label={game === 'horde' ? 'HORDE · HOLD THE BEACH' : `${game.toUpperCase()} · PROTOTYPE`} class="game-plate">
            {#if game === 'horde'}
              <HordeGame bind:this={hordeComponent} onframe={sendFrame} />
            {:else}
              <iframe class="prototype" title={`${game} prototype`} src={`/games/${game}/index.html`} sandbox="allow-scripts"></iframe>
            {/if}
          </Plate>
          <div class="game-mirror plate" data-label="LIVE LAUNCHPAD MIRROR">
            <PadGrid {leds} {selected} oninput={padInput} />
          </div>
        </div>
      {:else}
        <SetupDisplay level={crt} status={deviceState} events={midiLog} reconnect={() => sendFrame(leds)} onchange={setCrt} />
      {/if}
    </main>

    <footer class="status-strip">
      {#each ['LAUNCHPAD', 'LCXL', 'AUDIO'] as device}
        <button
          class="device-status"
          onclick={() => { mode = 'setup'; }}
          aria-label="{device}: {device === 'LAUNCHPAD' ? deviceState : device === 'AUDIO' ? (audioStatus?.state ?? 'stopped') : 'not connected'}. Open Setup"
        >
          <span
            class="lamp"
            class:stopped={device === 'AUDIO' && (!audioStatus || audioStatus.state !== 'running')}
            class:connected={(device === 'LAUNCHPAD' && deviceState === 'connected') || (device === 'AUDIO' && audioStatus?.state === 'running')}
            class:warn={device === 'LAUNCHPAD' && deviceState === 'busy'}
            class:bootloader={device === 'LAUNCHPAD' && deviceState === 'bootloader'}
          ></span>
          <span class="silk">{device}</span>
        </button>
      {/each}
      <span class="lcd lcd-dim">
        {audioStatus?.state === 'running'
          ? `${audioStatus.deviceName} · ${audioStatus.sampleRate} Hz · ${audioStatus.channels} CH`
          : 'AUDIO STOPPED'}
      </span>
      <span class="status-spacer"></span>
      <span class="lcd lcd-dim">{setName}</span>
      <button class="key small save-set-btn" onclick={saveSet} title="Save current set">
        SAVE
      </button>
    </footer>
  </div>
  <CrtLayers level={crt} />
</div>

<style>
  .bank-btn {
    background: none;
    border: none;
    padding: 0;
    text-align: left;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .bank-btn:hover .screen {
    border-color: var(--vec);
  }

  .active-bank-row .screen {
    color: var(--lcd);
    border-color: rgba(255, 91, 31, 0.6);
  }

  .mixer-stop-cap {
    cursor: pointer;
    background: radial-gradient(circle at 40% 30%, #ff5b1f, #881100);
    border-color: var(--orange);
  }

  .mixer-stop-cap:active {
    transform: translateY(2px);
    box-shadow: 0 1px 0 var(--lip0);
  }

  .footer-left {
    display: flex;
    align-items: center;
    gap: 16px;
    width: 100%;
    justify-content: space-between;
  }

  .light-toggles {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .toggle-label {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 9.5px;
    cursor: pointer;
    color: var(--silk);
  }

  .toggle-label input {
    cursor: pointer;
    accent-color: var(--orange);
    margin: 0;
  }

  .save-set-btn {
    width: auto;
    padding: 2px 8px;
    font-size: 10px;
    color: var(--silk);
  }
</style>
