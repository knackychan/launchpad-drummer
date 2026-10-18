<script lang="ts">
  import { onMount } from 'svelte';
  import Plate from './components/Plate.svelte';
  import LaunchpadPanel from './components/LaunchpadPanel.svelte';
  import MixerStrip from './components/MixerStrip.svelte';
  import PadEditor from './components/PadEditor.svelte';
  import Telemetry from './components/Telemetry.svelte';
  import KeysPlate from './components/KeysPlate.svelte';
  import SequencerPlate from './components/SequencerPlate.svelte';
  import CrtLayers from './components/CrtLayers.svelte';
  import SetupDisplay from './components/SetupDisplay.svelte';
  import HordeGame from './components/HordeGame.svelte';
  import ConsoleBackdrop from './components/ConsoleBackdrop.svelte';
  import { createLedFrame, ledToCss } from './lib/led.ts';
  import type { PadInput, Rgb } from './lib/led.ts';
  import { CRT_FACTORS, readDisplay, saveDisplay } from './lib/display.ts';
  import type { CrtLevel } from './lib/display.ts';
  import { createEmptySet, createPreviewSet, GROUP_COLORS, padLabel } from './lib/model.ts';
  import type { AudioStatus, Group, LightModes, Pad, SamplePeaks, SetModel } from './lib/model.ts';
  import { solvePlayLayout } from './lib/layout.ts';
  import { hitAmount, hitGroup, runActivity } from './lib/activity.ts';

  const tauri = typeof window !== 'undefined' ? window.__TAURI__ : undefined;
  const invoke = <T = unknown>(cmd: string, args?: Record<string, unknown>) =>
    tauri ? tauri.core.invoke<T>(cmd, args) : Promise.resolve(undefined as T);

  let mode = $state<'play' | 'games' | 'setup'>('play');
  let game = $state('horde');
  let gameFull = $state(false);
  let deviceState = $state('disconnected');
  let midiLog = $state<{ time: string; text: string }[]>([]);
  let crt = $state<CrtLevel>('low');
  let leds = $state<readonly Rgb[]>(createLedFrame());
  let frameSeen = $state(false);
  let pressed = $state<number[]>([]);
  let selected = $state<number | null>(21);

  let currentSet = $state<SetModel>(tauri ? createEmptySet() : createPreviewSet());
  let activeBank = $state(0);
  let setName = $state(tauri ? 'DEFAULT SET' : 'PREVIEW KIT');
  let audioStatus = $state<AudioStatus | null>(null);

  let drawer = $state<'editor' | null>(null);
  let drawerPinned = $state(true);
  let teleFolded = $state(false);
  let bayW = $state(0);
  let bayH = $state(0);
  let peaks = $state<SamplePeaks | null>(null);

  const layout = $derived(solvePlayLayout({
    width: bayW, height: bayH, drawerOpen: drawer !== null, drawerPinned, teleFolded, lanes: 4,
  }));

  const bank = $derived(currentSet.banks[activeBank]);
  const selXY = $derived(selected === null ? null : { x: (selected % 10) - 1, y: Math.floor(selected / 10) - 1 });
  const selectedPad = $derived(selXY ? bank?.pads.find((p) => p.x === selXY.x && p.y === selXY.y) ?? null : null);

  // Frame order: index (7 - y) * 8 + x, top-left first.
  const padNames = $derived.by(() => {
    const names = Array<string>(64).fill('');
    for (const p of bank?.pads ?? []) if (p.sample) names[(7 - p.y) * 8 + p.x] = padLabel(p.sample);
    return names;
  });
  // The backend light painter streams the real frame; before the first frame (and in the browser
  // preview) the pads show their own colours, pressed pads white.
  const displayLeds = $derived.by<readonly Rgb[]>(() => {
    if (frameSeen) return leds;
    const frame: Rgb[] = createLedFrame();
    for (const p of bank?.pads ?? []) if (p.sample) frame[(7 - p.y) * 8 + p.x] = p.color;
    for (const note of pressed) frame[(8 - Math.floor(note / 10)) * 8 + (note % 10) - 1] = [63, 63, 63];
    return frame;
  });

  const notice = $derived(
    deviceState === 'connected' ? { cls: 'ok', text: 'Launchpad MK2 connected' }
      : deviceState === 'busy' ? { cls: 'warn', text: 'Launchpad port busy — close other MIDI apps' }
        : deviceState === 'bootloader' ? { cls: 'error', text: 'Launchpad bootloader — run updater' }
          : { cls: 'info', text: tauri ? 'Launchpad not connected — on-screen pads available' : 'Browser preview — demo kit, no audio engine' },
  );
  const lampClass = (state: string) => ({ connected: 'ok', running: 'ok', busy: 'warn', bootloader: 'error', stopped: 'error' } as Record<string, string>)[state] ?? '';

  function logMidi(text: string) {
    midiLog = [{ time: new Date().toLocaleTimeString(), text }, ...midiLog].slice(0, 40);
  }

  function sendFrame(frame: readonly (readonly [number, number, number])[]) {
    void invoke('send_led_frame', { frame }).catch(() => {});
  }

  async function loadInitialState() {
    if (!tauri) return;
    try {
      const [set, b, status] = await Promise.all([
        invoke<SetModel>('get_current_set'),
        invoke<number>('get_current_bank'),
        invoke<AudioStatus>('get_audio_status'),
      ]);
      currentSet = set;
      activeBank = b;
      audioStatus = status;
    } catch (err) {
      console.warn('Failed to load initial state:', err);
    }
  }

  onMount(() => {
    try { crt = readDisplay(window.localStorage); } catch { crt = 'low'; }
    if (window.innerWidth >= 1780) drawer = 'editor';
    void loadInitialState();
    const stopActivity = runActivity();
    const onFullscreen = () => { if (!document.fullscreenElement) gameFull = false; };
    document.addEventListener('fullscreenchange', onFullscreen);

    const api = tauri?.event;
    const unlisten: (() => void)[] = [];
    if (api) {
      void api.listen<{ state: string; message: string }>('launchpad-status', (event) => {
        deviceState = event.payload.state;
        logMidi(event.payload.message);
      }).then((fn) => unlisten.push(fn));
      void api.listen<{ note: number; pressed: boolean }>('launchpad-pad', (event) => {
        logMidi(`PAD ${event.payload.note} ${event.payload.pressed ? 'DOWN' : 'UP'}`);
        padInput({ ...event.payload, source: 'hardware' });
      }).then((fn) => unlisten.push(fn));
      void api.listen<[number, number, number][]>('launchpad-frame', (event) => {
        if (mode === 'games') return;
        leds = event.payload.map(([r, g, b]) => [r, g, b] as const);
        frameSeen = true;
      }).then((fn) => unlisten.push(fn));
    }
    return () => {
      stopActivity();
      document.removeEventListener('fullscreenchange', onFullscreen);
      unlisten.forEach((fn) => fn());
    };
  });

  // Editor waveform: peaks of the selected pad's sample from the backend (a stand-in shape in the preview).
  $effect(() => {
    const sample = selectedPad?.sample;
    peaks = null;
    if (!sample) return;
    if (!tauri) { peaks = previewPeaks(sample); return; }
    let live = true;
    invoke<SamplePeaks | null>('sample_peaks', { path: sample, buckets: 512 })
      .then((p) => { if (live) peaks = p ?? null; })
      .catch(() => {});
    return () => { live = false; };
  });

  function previewPeaks(sample: string): SamplePeaks {
    let seed = [...sample].reduce((h, c) => Math.imul(h ^ c.charCodeAt(0), 16777619), 2166136261) >>> 0;
    const rnd = () => ((seed = Math.imul(seed ^ (seed >>> 15), 2246822519) >>> 0) / 4294967296);
    const long = /crash|ride|china|riser|sweep|impact/.test(sample);
    return { peaks: Array.from({ length: 512 }, (_, i) => Math.exp(-(i / 512) * (long ? 2 : 5)) * (0.35 + 0.65 * rnd())), lengthMs: long ? 1800 : 450, rate: 44100 };
  }

  let hordeComponent = $state<{ padInput: (note: number, pressed: boolean) => void } | undefined>();

  function setCrt(level: CrtLevel) {
    crt = level;
    try { saveDisplay(window.localStorage, level); } catch { /* the switch still works without storage */ }
  }

  function padInput(input: PadInput) {
    if (mode === 'games' && game === 'horde') {
      hordeComponent?.padInput(input.note, input.pressed);
      return;
    }
    if (input.note % 10 === 9) {
      // Hardware side button: the router already switched the bank.
      if (input.pressed) activeBank = 8 - Math.floor(input.note / 10);
      return;
    }
    const x = (input.note % 10) - 1;
    const y = Math.floor(input.note / 10) - 1;
    if (input.pressed) {
      selected = input.note;
      const pad = bank?.pads.find((p) => p.x === x && p.y === y);
      if (pad?.sample) {
        const g = currentSet.mixer.groups[pad.mixGroup];
        const anySolo = currentSet.mixer.groups.some((q) => q.solo);
        if (g) hitGroup(pad.mixGroup, hitAmount(g.volumeDb + pad.volumeDb, anySolo ? g.solo : !g.mute));
      }
    }
    if (!frameSeen) pressed = input.pressed ? [...pressed, input.note] : pressed.filter((n) => n !== input.note);
    // Hardware hits already reached the router directly; only on-screen pads are sent.
    if (input.source === 'screen') void invoke(input.pressed ? 'trigger_pad' : 'release_pad', { x, y }).catch(() => {});
  }

  function chooseBank(b: number) {
    activeBank = b;
    void invoke('select_bank', { bank: b }).catch((e) => console.warn('Bank select error:', e));
  }
  function renameBank(index: number, name: string) {
    currentSet.banks[index].name = name;
    void invoke('rename_bank', { bank: index, name });
  }
  function toggleLight(key: keyof LightModes, value: boolean) {
    bank.lights[key] = value;
    void invoke('update_bank_lights', { bank: activeBank, lights: bank.lights });
  }
  function setHumanize(humanizeDb: number) {
    bank.humanizeDb = humanizeDb;
    void invoke('update_bank_humanize', { bank: activeBank, humanizeDb });
  }
  function updatePad(pad: Pad) {
    const i = bank.pads.findIndex((p) => p.x === pad.x && p.y === pad.y);
    if (i >= 0) bank.pads[i] = pad;
    else bank.pads.push(pad);
    void invoke('update_pad', { bank: activeBank, pad });
  }
  function removePad(x: number, y: number) {
    bank.pads = bank.pads.filter((p) => !(p.x === x && p.y === y));
    void invoke('remove_pad', { bank: activeBank, x, y });
  }
  function updateGroup(index: number, group: Group) {
    currentSet.mixer.groups[index] = group;
    void invoke('update_group', { group: index, groupData: group });
  }
  function updateMaster(masterDb: number) {
    currentSet.mixer.masterDb = masterDb;
    void invoke('update_master_db', { masterDb });
  }
  function stopAll() {
    void invoke('stop_all');
  }

  async function promptSetManagement() {
    if (!tauri) return;
    const action = window.prompt("Set Action: type 'load', 'new', or 'save':", 'save');
    if (!action) return;
    if (action.toLowerCase() === 'save') {
      await saveSet();
    } else if (action.toLowerCase() === 'new' || action.toLowerCase() === 'load') {
      const isNew = action.toLowerCase() === 'new';
      const dir = window.prompt(isNew ? 'New set folder path:' : 'Set folder path to load:');
      if (!dir) return;
      try {
        currentSet = await invoke<SetModel>(isNew ? 'new_set_dir' : 'load_set_dir', { dir });
        setName = dir.split(/[\\/]/).pop() || dir;
      } catch (e) {
        window.alert(`Error ${isNew ? 'creating' : 'loading'} set: ${e}`);
      }
    }
  }
  async function saveSet() {
    try {
      await invoke('save_set_dir');
      logMidi('Set saved successfully');
    } catch (e) {
      console.warn('Save set info:', e);
    }
  }

  function setMode(next: typeof mode) {
    mode = next;
    if (next !== 'games') exitGameFull();
  }
  function enterGameFull() {
    gameFull = true;
    void document.documentElement.requestFullscreen?.().catch(() => {});
  }
  function exitGameFull() {
    gameFull = false;
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => {});
  }

  function keydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement;
    if (target.closest('input, textarea, select')) return;
    if (event.key === 'Escape') {
      if (gameFull) { exitGameFull(); return; }
      if (mode === 'play') stopAll();
    }
    if (event.ctrlKey && event.key.toLowerCase() === 'e' && mode === 'play') {
      event.preventDefault();
      drawer = drawer ? null : 'editor';
    }
  }
</script>

<svelte:window onkeydown={keydown} />

<div class="instrument" data-crt={crt} style:--crt={CRT_FACTORS[crt]}>
  <div class="console" class:game-full={gameFull}>
    <ConsoleBackdrop />
    <header class="top-strip">
      <div class="brand" aria-hidden="true">LPD·01</div>
      <button class="set-lcd screen" onclick={promptSetManagement} title="Set: save, load or create">
        <span class="silk">SET</span><span class="lcd">{setName}</span><span class="lcd lcd-dim">▾</span>
      </button>
      <button class="key small" aria-label="Undo" title="Undo (Ctrl+Z)" aria-disabled="true">↶</button>
      <button class="key small" aria-label="Redo" title="Redo (Ctrl+Y)" aria-disabled="true">↷</button>
      <div class="notice screen lcd {notice.cls}" role="status">{notice.text}</div>
      <div class="chips" aria-hidden="true">
        {#each GROUP_COLORS as c, i}<i data-chip={i} style:--c={ledToCss(c)}></i>{/each}
      </div>
    </header>

    <nav class="mode-rail" aria-label="Mode">
      <button class="key mode-key" class:on={mode === 'play'} aria-current={mode === 'play' ? 'page' : undefined} onclick={() => setMode('play')}>PLAY</button>
      <button class="key mode-key blank" aria-disabled="true" aria-label="TRAIN — not fitted yet" title="TRAIN — not fitted yet"></button>
      <button class="key mode-key" class:on={mode === 'games'} aria-current={mode === 'games' ? 'page' : undefined} onclick={() => setMode('games')}>GAMES</button>
      <button class="key mode-key blank" aria-disabled="true" aria-label="BAND — not fitted yet" title="BAND — not fitted yet"></button>
      <div class="rail-spacer"></div>
      <button class="key mode-key" class:on={mode === 'setup'} aria-current={mode === 'setup' ? 'page' : undefined} onclick={() => setMode('setup')}>SETUP</button>
    </nav>

    <main class="bay" aria-label={mode === 'play' ? 'Play' : mode === 'games' ? 'Games' : 'Setup'} bind:clientWidth={bayW} bind:clientHeight={bayH}>
      {#if mode === 'play'}
        <section
          class="play-screen"
          class:docked={layout.docked}
          style:--pad="{layout.pad}px"
          style:--kh="{layout.kh}px"
          style:--sq="{layout.sq}px"
          style:--lh="{layout.lh}px"
          style:--tw="{Math.max(0, layout.tw)}px"
        >
          <div class="stage">
            <div class="rig">
              <div class="stack">
                <div class="pair">
                  <LaunchpadPanel
                    leds={displayLeds}
                    names={padNames}
                    {selected}
                    banks={currentSet.banks}
                    {activeBank}
                    oninput={padInput}
                    onbank={chooseBank}
                    onrename={renameBank}
                    onlight={toggleLight}
                    onhumanize={setHumanize}
                    onstop={stopAll}
                  />
                  <MixerStrip
                    groups={currentSet.mixer.groups}
                    masterDb={currentSet.mixer.masterDb}
                    dims={layout.mix}
                    onupdategroup={updateGroup}
                    onupdatemaster={updateMaster}
                  />
                </div>
                <KeysPlate short={layout.kh < 68} />
                <SequencerPlate />
              </div>
              {#if layout.tw > 0}
                <Telemetry mini={layout.mini} onfold={(f) => { teleFolded = f; }} />
              {/if}
            </div>
          </div>

          {#if drawer && selXY}
            <div class="drawer">
              <PadEditor
                x={selXY.x}
                y={selXY.y}
                pad={selectedPad}
                groups={currentSet.mixer.groups}
                {peaks}
                onupdate={updatePad}
                onremove={removePad}
                ontrigger={(x, y) => void invoke('trigger_pad', { x, y })}
                onrelease={(x, y) => void invoke('release_pad', { x, y })}
              />
            </div>
          {/if}
          <div class="dtabs" role="tablist" aria-label="Side panel" aria-orientation="vertical">
            <button class="key dtab blank" aria-disabled="true" title="Sample browser — not fitted yet" aria-label="Browser — not fitted yet"></button>
            <button class="key dtab" class:on={drawer === 'editor'} role="tab" aria-selected={drawer === 'editor'} title="Pad editor (Ctrl+E)" onclick={() => { drawer = drawer ? null : 'editor'; }}>EDITOR</button>
            <button
              class="key round pin"
              class:on={drawerPinned}
              aria-pressed={drawerPinned}
              aria-label="Dock side panel"
              title={drawerPinned ? 'Docked: shrinks the Launchpad area. Click to float.' : 'Floating over the mixer. Click to dock.'}
              onclick={() => { drawerPinned = !drawerPinned; }}
            >⇥</button>
          </div>
        </section>
      {:else if mode === 'games'}
        <div class="games-stage" class:full={gameFull}>
          {#if gameFull}
            <button class="key small exit-full" title="Leave full screen (Esc)" onclick={exitGameFull}>✕ EXIT · ESC</button>
          {:else}
            <div class="game-bar" role="tablist" aria-label="Games">
              <span class="silk">GAMES</span>
              {#each [{ id: 'horde', title: 'HORDE', desc: 'hold the beach' }, { id: 'chess', title: 'CHESS', desc: 'prototype' }, { id: 'xiangqi', title: 'XIANGQI', desc: 'prototype' }, { id: 'cockpit', title: 'COCKPIT', desc: 'prototype' }] as entry}
                <button class="key" class:on={game === entry.id} role="tab" aria-selected={game === entry.id} onclick={() => { game = entry.id; }}>
                  {entry.title}<small>{entry.desc}</small>
                </button>
              {/each}
              <span class="status-spacer"></span>
              <button class="key" title="Full screen (Esc leaves)" onclick={enterGameFull}>⛶ FULL SCREEN</button>
            </div>
          {/if}
          <Plate label={game === 'horde' ? 'HORDE · HOLD THE BEACH' : `${game.toUpperCase()} · PROTOTYPE`} class="game-plate">
            {#if game === 'horde'}
              <HordeGame bind:this={hordeComponent} onframe={sendFrame} />
            {:else}
              <iframe class="prototype" title={`${game} prototype`} src={`/games/${game}/index.html`} sandbox="allow-scripts"></iframe>
            {/if}
          </Plate>
        </div>
      {:else}
        <div class="setup-scroll">
          <SetupDisplay level={crt} status={deviceState} events={midiLog} reconnect={() => sendFrame(leds)} onchange={setCrt} />
        </div>
      {/if}
    </main>

    <footer class="status-strip">
      <button class="device-status" onclick={() => setMode('setup')} aria-label="LAUNCHPAD: {deviceState}. Open Setup">
        <span class="lamp {lampClass(deviceState)}"></span><span class="silk">LAUNCHPAD</span>
      </button>
      <button class="device-status" onclick={() => setMode('setup')} aria-label="LCXL: not connected. Open Setup">
        <span class="lamp"></span><span class="silk">LCXL</span>
      </button>
      <button class="device-status" onclick={() => setMode('setup')} aria-label="AUDIO: {audioStatus?.state ?? 'stopped'}. Open Setup">
        <span class="lamp {lampClass(audioStatus?.state ?? 'stopped')}"></span><span class="silk">AUDIO</span>
      </button>
      <span class="lcd lcd-dim">
        {audioStatus?.state === 'running'
          ? `${audioStatus.deviceName} · ${audioStatus.bufferSize ?? 'auto'} · ${(audioStatus.sampleRate / 1000).toFixed(1)} kHz`
          : 'AUDIO STOPPED'}
      </span>
      <span class="lcd lcd-dim status-path">{setName}</span>
      <button class="key small" onclick={saveSet} title="Save current set">SAVE</button>
    </footer>
  </div>
  <CrtLayers level={crt} />
</div>
