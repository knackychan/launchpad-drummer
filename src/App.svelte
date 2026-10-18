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
  import NoticeLcd from './components/NoticeLcd.svelte';
  import SetMenu from './components/SetMenu.svelte';
  import PowerOn from './components/PowerOn.svelte';
  import { invoke as call, listen, isTauri } from './lib/api.ts';
  import { activeNotices, type Confirmation, type Level, type Notice, type NoticeInputs } from './lib/notices.ts';
  import { middleTruncate } from './lib/format.ts';
  import { padForKey } from './lib/keymap.ts';
  import { runEsc } from './lib/esc.ts';
  import { createLedFrame, ledToCss } from './lib/led.ts';
  import type { PadInput, Rgb } from './lib/led.ts';
  import { CRT_FACTORS, readDisplay, saveDisplay } from './lib/display.ts';
  import type { CrtLevel } from './lib/display.ts';
  import { createDefaultPad, createEmptySet, createPreviewSet, GROUP_COLORS, padLabel } from './lib/model.ts';
  import type { AudioStatus, Group, LightModes, MissingPad, OpenResult, Pad, SamplePeaks, SetModel, Startup } from './lib/model.ts';
  import { solvePlayLayout } from './lib/layout.ts';
  import { hitAmount, hitGroup, runActivity } from './lib/activity.ts';
  import { audioLabel, IDLE_SAVE, padEditKey } from './lib/status.ts';
  import type { SaveStatus } from './lib/status.ts';

  const tauri = isTauri;
  const editCommands = new Set(['update_pad', 'assign_sample', 'clear_pad', 'move_pad', 'drop_files', 'locate_sample', 'update_group', 'rename_bank', 'update_bank_lights', 'update_bank_humanize']);
  async function invoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    const result = await call<T>(cmd, args);
    if (editCommands.has(cmd)) refreshHistory();
    return result;
  }

  let mode = $state<'play' | 'games' | 'setup'>('play');
  let game = $state('horde');
  let gameFull = $state(false);
  let deviceState = $state<NoticeInputs['launchpad']>('disconnected');
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
  let saveStatus = $state<SaveStatus>(IDLE_SAVE);
  let startup = $state<Startup>({ kind: 'loaded', path: '', name: '', fromBackup: false, backupSavedAt: null });
  let setPath = $state('');
  let powerError = $state<string | null>(null);
  let missing = $state<MissingPad[]>([]);
  let missingIndex = 0;
  let history = $state({ canUndo: false, canRedo: false });
  let confirmations = $state<Confirmation[]>([]);
  let dismissed = $state<Set<string>>(new Set());
  let backup = $state<NoticeInputs['backup']>(null);
  let renaming = $state<number | null>(null);
  let mixerRenaming = $state<number | null>(null);
  let historyTimer = 0;
  const confirmationTimers = new Set<number>();
  let assignSeq = 0;

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

  const notices = $derived(activeNotices({ learnArmed: null, audioLost: audioStatus?.state === 'lost' || audioStatus?.state === 'error',
    backup, saveFailed: saveStatus.state === 'failed' ? saveStatus.reason : null,
    launchpad: tauri ? deviceState : 'connected', lcxl: tauri ? 'disconnected' : 'connected', missing: missing.length,
    asioFallback: false, dropoutRecent: false, confirmations, keysAbsent: false, dismissed }));
  const lampClass = (state: string) => ({ connected: 'ok', running: 'ok', busy: 'warn', bootloader: 'error', stopped: 'error', error: 'error' } as Record<string, string>)[state] ?? '';

  function logMidi(text: string) {
    midiLog = [{ time: new Date().toLocaleTimeString(), text }, ...midiLog].slice(0, 40);
  }
  /** A message about something the player just did, shown in the notice LCD for a while. */
  function showToast(text: string) {
    confirm(text, 'error');
  }
  function confirm(text: string, level: Level = 'ok') {
    const id = crypto.randomUUID();
    confirmations = [...confirmations, { id, text, level }];
    const timer = window.setTimeout(() => { confirmations = confirmations.filter(c => c.id !== id); confirmationTimers.delete(timer); }, 3000);
    confirmationTimers.add(timer);
  }
  function refreshHistory() {
    clearTimeout(historyTimer);
    historyTimer = window.setTimeout(() => { void call<typeof history>('history_state').then(h => { if (h) history = h; }).catch(e => showToast(errorText(e))); }, 100);
  }
  async function refreshMissing() { missing = await invoke<MissingPad[]>('missing_samples') ?? []; }
  async function undoRedo(cmd: 'undo' | 'redo') {
    if (cmd === 'undo' ? !history.canUndo : !history.canRedo) return;
    try { const set = await invoke<SetModel | null>(cmd); if (set) { currentSet = set; confirm(cmd === 'undo' ? 'Undone' : 'Redone'); await refreshMissing(); } refreshHistory(); }
    catch (e) { showToast(errorText(e)); }
  }
  async function noticeAction(n: Notice) {
    try {
      if (n.action?.id === 'dismiss') dismissed = new Set([...dismissed, n.id]);
      if (n.action?.id === 'setup') setMode('setup');
      if (n.action?.id === 'reconnect') { const status = await invoke<AudioStatus>('audio_reconnect'); if (status) audioStatus = status; }
      if (n.action?.id === 'show') { await refreshMissing(); const p = missing[missingIndex++ % missing.length]; if (p) { chooseBank(p.bank); selected = (p.y + 1) * 10 + p.x + 1; drawer = 'editor'; setMode('play'); } }
    } catch (e) { showToast(errorText(e)); }
  }
  const errorText = (e: unknown) => String(e instanceof Error ? e.message : e);

  function sendFrame(frame: readonly (readonly [number, number, number])[]) {
    void invoke('send_led_frame', { frame }).catch(() => {});
  }

  async function loadInitialState() {
    try {
      startup = await invoke<Startup>('get_startup');
      if (startup.kind === 'loaded') { setPath = startup.path; setName = startup.name; backup = startup.fromBackup ? { savedAt: startup.backupSavedAt } : null; }
      await refreshMissing();
      history = await invoke<typeof history>('history_state');
      if (!tauri) return;
      const [set, b, status, save] = await Promise.all([
        invoke<SetModel>('get_current_set'),
        invoke<number>('get_current_bank'),
        invoke<AudioStatus>('get_audio_status'),
        invoke<SaveStatus>('get_save_status'),
      ]);
      currentSet = set;
      activeBank = b;
      audioStatus = status;
      saveStatus = save;
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

    const unlisten: (() => void)[] = [];
    let disposed = false;
    function subscribe<T>(event: string, fn: (payload: T) => void) {
      void listen<T>(event, fn).then(stop => { if (disposed) stop(); else unlisten.push(stop); }).catch(e => showToast(errorText(e)));
    }
    if (tauri) {
      subscribe<{ state: NoticeInputs['launchpad']; message: string }>('launchpad-status', (event) => {
        deviceState = event.state;
        logMidi(event.message);
      });
      subscribe<{ note: number; pressed: boolean }>('launchpad-pad', (event) => {
        logMidi(`PAD ${event.note} ${event.pressed ? 'DOWN' : 'UP'}`);
        padInput({ ...event, source: 'hardware' });
      });
      subscribe<[number, number, number][]>('launchpad-frame', (event) => {
        if (mode === 'games') return;
        leds = event.map(([r, g, b]) => [r, g, b] as const);
        frameSeen = true;
      });
      subscribe<SaveStatus>('save-status', (event) => { saveStatus = event; });
      subscribe<AudioStatus>('audio-status', (event) => { audioStatus = event; });
    }
    return () => {
      disposed = true;
      clearTimeout(historyTimer);
      confirmationTimers.forEach(clearTimeout);
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
    releaseKeyboard();
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
  function updatePad(pad: Pad, field?: string) {
    const i = bank.pads.findIndex((p) => p.x === pad.x && p.y === pad.y);
    if (i >= 0) bank.pads[i] = pad;
    else bank.pads.push(pad);
    void invoke('update_pad', { bank: activeBank, pad, key: padEditKey(activeBank, pad.x, pad.y, field) });
  }
  /** Decode the sample on the backend, then take the set it returns. Resolves false when rejected. */
  async function assignSample(x: number, y: number, path: string): Promise<boolean> {
    if (!tauri) {
      const existing = bank.pads.find((p) => p.x === x && p.y === y);
      updatePad(existing ? { ...existing, sample: path } : { ...createDefaultPad(x, y), sample: path, color: [...GROUP_COLORS[y % 8]] as [number, number, number] });
      return true;
    }
    const seq = ++assignSeq;
    try {
      const set = await invoke<SetModel>('assign_sample', { bank: activeBank, x, y, path, defaultColor: GROUP_COLORS[y % 8] });
      if (seq === assignSeq) currentSet = set;
      return true;
    } catch (e) {
      if (seq === assignSeq) showToast(`Sample not assigned — ${errorText(e)}`);
      return false;
    }
  }
  function removePad(x: number, y: number) {
    void invoke<SetModel>('clear_pad', { bank: activeBank, x, y }).then(set => { if (set) currentSet = set; }).catch(e => showToast(errorText(e)));
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

  async function manageSet(isNew: boolean) {
    try {
      const defaultDir = await invoke<string>('default_sets_dir');
      const dir = await invoke<string | null>('pick_folder', { title: isNew ? 'New set — choose an empty folder' : 'Open set folder', defaultDir });
      if (!dir) return;
      const opened = await invoke<OpenResult>(isNew ? 'new_set' : 'open_set', { dir });
      if (!opened) return;
      currentSet = opened.set; setName = opened.name; setPath = opened.path;
      startup = { kind: 'loaded', path: opened.path, name: opened.name, fromBackup: opened.fromBackup, backupSavedAt: opened.backupSavedAt };
      backup = opened.fromBackup ? { savedAt: opened.backupSavedAt } : null;
      dismissed = new Set(); powerError = null; selected = null; peaks = null;
      activeBank = await invoke<number>('get_current_bank') ?? 0;
      await refreshMissing(); refreshHistory();
    } catch (e) {
      if (startup.kind !== 'loaded') powerError = errorText(e);
      else showToast(errorText(e));
    }
  }

  /** Games own the Launchpad pads and LEDs; every other screen leaves them to the sampler. */
  function setMode(next: typeof mode) {
    releaseKeyboard();
    const was = mode;
    mode = next;
    if (next !== 'games') exitGameFull();
    if ((was === 'games') !== (next === 'games')) {
      void invoke<number>('set_input_mode', { mode: next === 'games' ? 'game' : 'sampler' })
        .then((b) => { if (typeof b === 'number') activeBank = b; })
        .catch((e) => showToast(`Launchpad mode not switched — ${errorText(e)}`));
    }
  }
  function enterGameFull() {
    gameFull = true;
    void document.documentElement.requestFullscreen?.().catch(() => {});
  }
  function exitGameFull() {
    gameFull = false;
    if (document.fullscreenElement) void document.exitFullscreen().catch(() => {});
  }

  const keyboardHeld = new Map<string, number>();
  function keyup(event: KeyboardEvent) {
    const id = event.code || event.key.toLowerCase();
    const note = keyboardHeld.get(id);
    if (note === undefined) return;
    event.preventDefault(); keyboardHeld.delete(id);
    padInput({ note, pressed: false, source: 'screen' });
  }
  function releaseKeyboard() {
    for (const note of keyboardHeld.values()) padInput({ note, pressed: false, source: 'screen' });
    keyboardHeld.clear();
  }
  function keydown(event: KeyboardEvent) {
    if (event.defaultPrevented) return;
    const target = event.target as HTMLElement;
    if (target.closest('input, textarea, select, [contenteditable="true"], [role="menu"]')) return;
    const key = event.key.toLowerCase();
    let handled = true;
    if (key === 'escape') { if (!runEsc()) { if (gameFull) exitGameFull(); else if (mode === 'play') stopAll(); } }
    else if (event.ctrlKey) {
      if (key === 'n' || key === 'o') void manageSet(key === 'n');
      else if (key === 'z' || key === 'y') void undoRedo(key === 'y' || event.shiftKey ? 'redo' : 'undo');
      else if (key === '1') setMode('play');
      else if (key === ',') setMode('setup');
      else if (key === '2') { /* TRAIN remains a blank cap. */ }
      else if (key === 'e' && mode === 'play') drawer = drawer ? null : 'editor';
      else handled = false;
    } else if (mode === 'play' && startup.kind === 'loaded') {
      const gridFocused = !!target.closest('.pad-grid') || document.activeElement === document.body;
      if (event.altKey && /^[1-8]$/.test(key)) chooseBank(Number(key) - 1);
      else if (key === 'pageup' || key === 'pagedown') chooseBank(Math.max(0, Math.min(7, activeBank + (key === 'pageup' ? -1 : 1))));
      else if (key === 'f2') {
        const channel = target.closest<HTMLElement>('.mx-col[data-group]');
        if (channel) mixerRenaming = Number(channel.dataset.group); else renaming = activeBank;
      } else if (key === 'delete' && selXY) { removePad(selXY.x, selXY.y); confirm('Pad cleared'); }
      else if (gridFocused && ['arrowleft', 'arrowright', 'arrowup', 'arrowdown'].includes(key)) {
        const xy = selXY ?? { x: 0, y: 0 };
        const x = Math.max(0, Math.min(7, xy.x + (key === 'arrowright' ? 1 : key === 'arrowleft' ? -1 : 0)));
        const y = Math.max(0, Math.min(7, xy.y + (key === 'arrowup' ? 1 : key === 'arrowdown' ? -1 : 0)));
        selected = (y + 1) * 10 + x + 1;
      } else if (!event.altKey && !target.closest('[role="tree"]')) {
        const xy = key === ' ' && gridFocused ? selXY : padForKey(key);
        if (xy) {
          if (!event.repeat) {
            const id = event.code || key, note = (xy.y + 1) * 10 + xy.x + 1;
            if (!keyboardHeld.has(id)) { keyboardHeld.set(id, note); padInput({ note, pressed: true, source: 'screen' }); }
          }
        } else handled = false;
      } else handled = false;
    } else handled = false;
    if (handled) event.preventDefault();
  }
</script>

<svelte:window onkeydown={keydown} onkeyup={keyup} onblur={releaseKeyboard} />

<div class="instrument" data-crt={crt} style:--crt={CRT_FACTORS[crt]}>
  <div class="console" class:game-full={gameFull}>
    <ConsoleBackdrop />
    <header class="top-strip">
      <div class="brand" aria-hidden="true">LPD·01</div>
      <SetMenu name={setName} path={setPath} onnew={() => void manageSet(true)} onopen={() => void manageSet(false)} onreveal={() => void invoke('reveal_path', { path: setPath }).catch(e => showToast(errorText(e)))} />
      <button class="key small" aria-label="Undo" title="Undo (Ctrl+Z)" aria-disabled={!history.canUndo} onclick={() => void undoRedo('undo')}>↶</button>
      <button class="key small" aria-label="Redo" title="Redo (Ctrl+Y)" aria-disabled={!history.canRedo} onclick={() => void undoRedo('redo')}>↷</button>
      <NoticeLcd {notices} idle={tauri ? 'Launchpad MK2 connected' : 'Browser preview — demo kit, no audio engine'} onaction={n => void noticeAction(n)} />
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
      {#if mode === 'play' && startup.kind !== 'loaded'}
        <PowerOn {startup} error={powerError} launchpad={deviceState} lcxl="disconnected" audio={audioStatus?.state ?? 'stopped'} onnew={() => void manageSet(true)} onopen={() => void manageSet(false)} />
      {:else if mode === 'play'}
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
                    bind:renaming
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
                    bind:renaming={mixerRenaming}
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
                onassign={assignSample}
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
          <SetupDisplay level={crt} status={deviceState} events={midiLog} reconnect={() => void invoke('refresh_leds').catch(() => {})} onchange={setCrt} />
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
      <span class="lcd lcd-dim" class:audio-error={audioStatus?.state === 'error'}>{audioLabel(audioStatus)}</span>
      <span class="lcd lcd-dim">DROPS {audioStatus?.dropouts ?? 0}</span>
      <span class="lcd lcd-dim status-path" title={setPath}>{middleTruncate(setPath, 48)}</span>
      {#key saveStatus}<span class="lamp" class:ok={saveStatus.state === 'saved'} class:blink={saveStatus.state === 'saved'} class:error={saveStatus.state === 'failed'} role="img" aria-label={saveStatus.state === 'failed' ? 'Save failed' : saveStatus.state === 'saved' ? 'Saved' : 'Saving…'}></span>{/key}
    </footer>
  </div>
  <CrtLayers level={crt} />
</div>
