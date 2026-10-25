<script lang="ts">
  import { onMount, setContext, type Component } from 'svelte';
  import Plate from './components/Plate.svelte';
  import LaunchpadPanel from './components/LaunchpadPanel.svelte';
  import MixerSlim from './components/MixerSlim.svelte';
  import ChannelPopover from './components/ChannelPopover.svelte';
  import SynthPlate from './components/SynthPlate.svelte';
  import PadEditor from './components/PadEditor.svelte';
  import Telemetry from './components/Telemetry.svelte';
  import KeysPlate from './components/KeysPlate.svelte';
  import SequencerPlate from './components/SequencerPlate.svelte';
  import CrtLayers from './components/CrtLayers.svelte';
  import ConsoleBackdrop from './components/ConsoleBackdrop.svelte';
  import NoticeLcd from './components/NoticeLcd.svelte';
  import SetMenu from './components/SetMenu.svelte';
  import PowerOn from './components/PowerOn.svelte';
  import StartupLoading from './components/StartupLoading.svelte';
  import SampleBrowser from './components/SampleBrowser.svelte';
  import DragGhost from './components/DragGhost.svelte';
  import ContextMenu, { type MenuItem } from './components/ContextMenu.svelte';
  import { BANK_HOVER_MS, dropTag, hitTest, isAudioName, passedThreshold, physicalToCss, type DropTarget } from './lib/dnd.ts';
  import { invoke as call, listen, isTauri, isStaged } from './lib/api.ts';
  import { activeNotices, type Confirmation, type Level, type Notice, type NoticeInputs } from './lib/notices.ts';
  import { middleTruncate } from './lib/format.ts';
  import { padForKey } from './lib/keymap.ts';
  import { pushEsc, runEsc } from './lib/esc.ts';
  import { bindingLabel, boundIds } from './lib/learn.ts';
  import { SOUND_COLORS, SOUND_LABELS } from './lib/keys.ts';
  import type { KeysMix, Route, SourceId } from './lib/synth-types.ts';
  import { setParam, throttle, withoutRoute } from './lib/synth.ts';
  import { LEARN, type LearnCtx } from './lib/learnable.ts';
  import { createLedFrame, ledToCss } from './lib/led.ts';
  import type { PadInput, Rgb } from './lib/led.ts';
  import { CRT_FACTORS, readDisplay, saveDisplay } from './lib/display.ts';
  import type { Bend, CrtLevel } from './lib/display.ts';
  import { createEmptySet, createPreviewSet, GROUP_COLORS, groupLegend, padAriaLabel, padLabel } from './lib/model.ts';
  import type { AppConfig, AudioStatus, Binding, KeysStatus, LoadStatus, Pattern, SeqPosition, DropReport, Group, LightModes, MidiDevice, MissingPad, Pad, SamplePeaks, SetModel, Startup } from './lib/model.ts';
  import { acceptLoad, createGeneration, eventFirst, fetchReadyState, INITIAL_LOAD, isTerminal } from './lib/startup.ts';
  import { PREVIEW_MIDI, pushLog, roleStatus } from './lib/midi.ts';
  import { solvePlayLayout } from './lib/layout.ts';
  import { feedLive, hitAmount, hitGroup, runActivity } from './lib/activity.ts';
  import { audioLabel, dropoutRecent, IDLE_SAVE, padEditKey } from './lib/status.ts';
  import type { SaveStatus } from './lib/status.ts';
  import { pushFrame } from './lib/telemetry.ts';
  import type { TelemetryFrame } from './lib/telemetry.ts';

  const tauri = isTauri;
  /** The desktop app, or the browser preview playing a scripted startup: the UI waits for the backend's load status. */
  const staged = isStaged;
  const editCommands = new Set(['update_pad', 'clear_pad', 'move_pad', 'drop_files', 'locate_sample', 'update_group', 'rename_bank', 'update_bank_lights', 'update_bank_humanize', 'unbind', 'seq_update', 'seq_add_lane', 'set_keys', 'synth_param', 'synth_route', 'synth_load_patch', 'synth_revert']);
  async function invoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    if ((editCommands.has(cmd) && cmd !== 'synth_param' && cmd !== 'synth_route') || ['undo','redo','new_set','open_set','open_default_set','retry_startup','synth_load_patch','synth_revert'].includes(cmd)) sendRouteSoon.flush();
    const result = await call<T>(cmd, args);
    if (!disposed && editCommands.has(cmd)) refreshHistory();
    return result;
  }

  let mode = $state<'play' | 'games' | 'setup'>('play');
  let game = $state('horde');
  let gameFull = $state(false);
  let deviceState = $state<NoticeInputs['launchpad']>('disconnected');
  let midiLog = $state<{ time: string; text: string }[]>([]);
  let midiDevices = $state<MidiDevice[]>(tauri ? [] : structuredClone(PREVIEW_MIDI));
  // A `midi-devices` event is newer than the initial fetch.
  const midiFeed = eventFirst<MidiDevice[]>(devices => { midiDevices = devices; });
  let crt = $state<CrtLevel>('low');
  let bend = $state<Bend>('flat');
  let leds = $state<readonly Rgb[]>(createLedFrame());
  let frameSeen = $state(false);
  let pressed = $state<number[]>([]);
  let selected = $state<number | null>(null);

  let currentSet = $state<SetModel>(tauri ? createEmptySet() : createPreviewSet());
  let activeBank = $state(0);
  let setName = $state(staged ? 'LOADING…' : 'PREVIEW KIT');
  let audioStatus = $state<AudioStatus | null>(null);
  // An `audio-status` event is newer than the initial fetch, which may have been answered earlier.
  const audioFeed = eventFirst<AudioStatus>(status => { audioStatus = status; });
  let saveStatus = $state<SaveStatus>(IDLE_SAVE);
  // Startup (the outcome) is only meaningful once the backend says so; `load` (the operation) is what the screens follow.
  let startup = $state<Startup>(staged ? { kind: 'firstRun' } : { kind: 'loaded', path: '', name: '', fromBackup: false, backupSavedAt: null });
  let load = $state<LoadStatus>(staged ? INITIAL_LOAD : { ...INITIAL_LOAD, requestId: 1, revision: 1, phase: 'ready' });
  /** A ready set has been fetched and shown: the interface is usable. Never an empty placeholder. */
  let hasSet = $state(!staged);
  let startupError = $state<string | null>(null);
  /** Newest request for set state: an answer started under an older token must not replace what is shown. */
  const setGate = createGeneration();
  let manageInFlight = $state(0);
  const unlisten: (() => void)[] = [];
  let disposed = false;
  let setPath = $state('');
  let powerError = $state<string | null>(null);
  let missing = $state<MissingPad[]>([]);
  let missingIndex = 0;
  let history = $state({ canUndo: false, canRedo: false });
  let confirmations = $state<Confirmation[]>([]);
  let dismissed = $state<Set<string>>(new Set());
  let backup = $state<NoticeInputs['backup']>(null);
  let renaming = $state<number | null>(null);
  let chanPop = $state<{ index: number; x: number; y: number; rename: boolean } | null>(null);
  let mixPop = $state<{ x: number; y: number } | null>(null);
  let mixPanel = $state<HTMLDivElement>();
  $effect(() => { if (mixPop) return pushEsc(() => { mixPop = null; }); });
  let historyTimer = 0;
  const confirmationTimers = new Set<number>();
  let peaksNonce = $state(0);
  let padMenu = $state<{ x: number; y: number; at: { x: number; y: number } } | null>(null);

  // Step sequencer (Task 14): the clock runs in the router; the plate edits the pattern optimistically.
  const SEQ_FOLD_KEY = 'lpd.seq.folded';
  let seqFolded = $state(false);
  let seqPos = $state<SeqPosition | null>(null);
  let seqAt = $state(0);
  let seqRec = $state(false);
  let patternCall = 0;
  const pattern = $derived(currentSet.pattern);
  const seqPlaying = $derived(!!seqPos?.playing);
  const seqHits = $derived(Array.from({ length: 32 }, (_, i) => pattern.lanes.some(l => l.steps[i] > 0)));
  function setSeqFolded(folded: boolean) {
    seqFolded = folded;
    try { window.localStorage.setItem(SEQ_FOLD_KEY, String(folded)); } catch { /* the plate still folds without storage */ }
  }
  async function resyncSet() {
    if (!tauri) return;
    sendRouteSoon.flush();
    try { const set = await call<SetModel>('get_current_set'); if (set) replaceSet(set); } catch { /* keep what is shown */ }
  }
  /** Optimistic: show the new pattern now, send it, and take the returned set unless a newer edit is in flight. */
  function updatePattern(next: Pattern, key?: string) {
    currentSet.pattern = next;
    const id = ++patternCall;
    void invoke<SetModel>('seq_update', { pattern: next, key })
      .then(set => { if (set && id === patternCall) replaceSet(set); })
      .catch(e => { showToast(errorText(e)); void resyncSet(); });
  }
  async function addLane() {
    if (!selXY) { showToast('Select a pad first'); return; }
    if (!tauri) {
      if (!selectedPad?.sample) { showToast('Pad is empty'); return; }
      if (pattern.lanes.some(l => l.bank === activeBank && l.x === selXY.x && l.y === selXY.y)) { showToast('Lane already there'); return; }
      currentSet.pattern = { ...pattern, lanes: [...pattern.lanes, { bank: activeBank, x: selXY.x, y: selXY.y, mute: false, steps: Array(32).fill(0) }] };
      confirm('Lane added');
      return;
    }
    try {
      const set = await invoke<SetModel>('seq_add_lane', { bank: activeBank, ...selXY });
      if (set) { replaceSet(set); confirm('Lane added'); }
    } catch (e) { showToast(errorText(e)); }
  }
  function toggleSeqPlay() { void invoke('seq_play', { play: !seqPlaying }).catch(e => showToast(errorText(e))); }
  function toggleSeqRec() {
    seqRec = !seqRec;
    void invoke('seq_rec', { armed: seqRec }).catch(e => showToast(errorText(e)));
  }
  function selectLanePad(bank: number, x: number, y: number) {
    if (bank !== activeBank) chooseBank(bank);
    selected = (y + 1) * 10 + x + 1;
  }

  // Keys plate (Task 16): sound, volume and output are set data; octave and fold are UI state.
  const KEYS_FOLD_KEY = 'lpd.keys.folded', KEYS_OCTAVE_KEY = 'lpd.keys.octave';
  let keysFolded = $state(false);
  let keysOctave = $state(0);
  let keysStatus = $state<KeysStatus>({ engine: 'builtin', name: 'BUILT-IN', error: null });
  let soundfontPath = $state<string | null>(null);
  let keysDown = $state<Set<number>>(new Set());
  let keysLast = $state<{ note: number; velocity: number } | null>(null);
  let keysWheel = $state({ pitch: 0, mod: 0 });
  let keysCall = 0;
  function setKeysFolded(folded: boolean) {
    keysFolded = folded;
    try { window.localStorage.setItem(KEYS_FOLD_KEY, String(folded)); } catch { /* the plate still folds without storage */ }
  }
  function setKeysOctave(octave: number) {
    keysOctave = Math.min(2, Math.max(-1, octave));
    try { window.localStorage.setItem(KEYS_OCTAVE_KEY, String(keysOctave)); } catch { /* octave is a per-user convenience */ }
  }
  function updateKeys(mix: KeysMix) {
    currentSet.keys.gmSound = mix.gmSound;
    currentSet.keys.volumeDb = mix.volumeDb;
    currentSet.keys.output = mix.output;
    const id = ++keysCall;
    void invoke<SetModel>('set_keys', { settings: mix })
      .then(set => { if (set && id === keysCall) replaceSet(set); })
      .catch(e => { showToast(errorText(e)); void resyncSet(); });
  }
  function keysNote(note: number, velocity: number, on: boolean) {
    // The engine answers with `keys-note` (it lights the key in every mirror); the preview has no engine.
    if (!tauri) { const next = new Set(keysDown); if (on) next.add(note); else next.delete(note); keysDown = next; }
    void invoke('keys_note', { note, velocity, on }).catch(e => showToast(errorText(e)));
  }
  function keysPad(bank: number, x: number, y: number, on: boolean) {
    if (on && bank === activeBank) selected = (y + 1) * 10 + x + 1;
    void invoke(on ? 'trigger_pad_in' : 'release_pad_in', { bank, x, y }).catch(e => showToast(errorText(e)));
  }
  const keysPresent = $derived(midiDevices.some(d => d.role === 'keys' && d.status === 'connected'));
  const keysDeviceName = $derived((midiDevices.find(d => d.role === 'keys' && d.status === 'connected') ?? midiDevices.find(d => d.role === 'keys'))?.port.toUpperCase() ?? 'KEYSTATION 49');
  async function loadSoundfont() {
    try {
      const path = await invoke<string | null>('pick_file', { title: 'Choose a SoundFont', extensions: ['sf2'] });
      if (path) await applySoundfont(path);
    } catch (e) { showToast(errorText(e)); }
  }
  async function applySoundfont(path: string | null) {
    try {
      const status = await invoke<KeysStatus>('set_soundfont', { path });
      if (status) keysStatus = status;
      soundfontPath = path;
    } catch (e) { showToast(errorText(e)); }
  }
  // Mixer channel inputs (UI spec 6): which pads and whether the keys play through a channel.
  const channelInputs = $derived(currentSet.mixer.groups.map((_, i) => ({
    pads: currentSet.banks.reduce((n, b) => n + b.pads.filter(p => p.sample && p.mixGroup === i).length, 0),
    keysColor: currentSet.keys.output === i ? ledToCss(SOUND_COLORS[currentSet.keys.gmSound]) : null,
    keysLabel: SOUND_LABELS[currentSet.keys.gmSound],
  })));
  function channelMenu(i: number, x: number, y: number, rename = false) {
    chanPop = { index: i, x, y, rename };
  }
  async function useSynth() {
    try { const status = await invoke<KeysStatus>('set_keys_engine', { engine: 'synth' }); if (status) keysStatus = status; }
    catch (e) { showToast(errorText(e)); }
  }
  // Synth edits are optimistic: the set shows the value now, the backend stores it (spec 2.2).
  function synthParam(id: string, value: number) {
    const stored = setParam(currentSet.keys.patch.params, id, value);
    currentSet.keys.modified = true;
    void invoke('synth_param', { id, value: stored }).catch((e) => { showToast(errorText(e)); void resyncSet(); });
  }
  let selectedRoute = $state<Record<string, number>>({});
  const routes = () => currentSet.keys.patch.params.routes;
  function sendRoute(slot: number, route: Route | null) {
    currentSet.keys.modified = true;
    void invoke('synth_route', { slot, route }).catch((e) => { showToast(errorText(e)); void resyncSet(); });
  }
  const sendRouteSoon = throttle(sendRoute, 16, slot => slot);
  function replaceSet(set: SetModel) {
    // An authoritative snapshot may already belong to a new patch; never replay old pending edits.
    sendRouteSoon.cancel();
    selectedRoute = {};
    currentSet = set;
  }
  function addRoute(source: SourceId, dest: string | null) {
    if (routes().length >= 16) { showToast('Matrix full (16 routes)'); return; }
    sendRouteSoon.flush();
    const route: Route = { source, dest, amount: 0.25, aux: null, bypass: false };
    routes().push(route);
    if (dest) selectedRoute[dest] = routes().length - 1;
    sendRoute(routes().length - 1, route);
  }
  function updateRoute(slot: number, route: Route) {
    routes()[slot] = route;
    sendRouteSoon(slot, route);
  }
  function removeRoute(slot: number) {
    sendRouteSoon.flush();
    routes().splice(slot, 1);
    selectedRoute = withoutRoute(selectedRoute, slot);
    sendRoute(slot, null);
  }
  /** Alt+drag changes the selected route's depth, or the newest route on the destination. */
  function routeDepth(dest: string, delta: number) {
    const slots = routes().map((r, i) => r.dest === dest ? i : -1).filter(i => i >= 0);
    const slot = slots.includes(selectedRoute[dest]) ? selectedRoute[dest] : slots.at(-1);
    if (slot === undefined) return;
    const r = routes()[slot];
    updateRoute(slot, { ...r, amount: Math.min(1, Math.max(-1, r.amount + delta)) });
  }

  // MIDI learn (plan 1C Task 12): the control being learned, and the one shared menu of learnable controls.
  let learnArmed = $state<{ id: string; kind: 'cc' | 'note' } | null>(null);
  let learnMenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  const learnBound = $derived(boundIds(currentSet));
  $effect(() => { if (learnArmed) return pushEsc(cancelLearn); });
  async function startLearn(id: string, kind: 'cc' | 'note', target: unknown) {
    learnArmed = { id, kind };
    try { await call('arm_learn', { target }); }
    catch (e) { learnArmed = null; showToast(errorText(e)); }
  }
  function cancelLearn() {
    if (!learnArmed) return;
    learnArmed = null;
    void call('cancel_learn').catch(() => {});
  }
  const learnPadId = (bank: number, x: number, y: number) => `pad:${bank}:${x}:${y}`;
  const armLearnPad = (bank: number, x: number, y: number) => startLearn(learnPadId(bank, x, y), 'note', { pad: { bank, x, y } });
  setContext<LearnCtx>(LEARN, {
    armed: () => learnArmed?.id ?? null,
    isBound: (id) => learnBound.has(id),
    arm: (id) => void startLearn(id, 'cc', { param: id }),
    openMenu: (x, y, items) => { learnMenu = { x, y, items }; },
  });
  async function onLearnBound(binding: Binding) {
    learnArmed = null;
    try {
      const set = await call<SetModel>('get_current_set');
      if (set) replaceSet(set);
      confirm(`Learned ${bindingLabel(binding, currentSet)}`);
      refreshHistory();
    } catch (e) { showToast(errorText(e)); }
  }
  function padHit(bank: number, x: number, y: number) {
    if (bank !== activeBank) return;
    const note = (y + 1) * 10 + x + 1;
    padInput({ note, pressed: true, source: 'hardware' });
    window.setTimeout(() => padInput({ note, pressed: false, source: 'hardware' }), 120);
  }
  async function unbindAt(index: number) {
    try { const set = await invoke<SetModel>('unbind', { index }); if (set) replaceSet(set); }
    catch (e) { showToast(errorText(e)); }
  }
  const padBinding = $derived.by(() => {
    if (!selXY) return null;
    const i = currentSet.learn.findIndex(b => b.kind === 'note' && 'pad' in b.target && b.target.pad.bank === activeBank && b.target.pad.x === selXY.x && b.target.pad.y === selXY.y);
    const b = currentSet.learn[i];
    return b ? { index: i, number: b.number, device: b.device } : null;
  });

  let drawer = $state<'editor' | 'browser' | null>(null);
  let libraryDir = $state<string | null>(null);
  $effect(() => { if (drawer) return pushEsc(() => { drawer = null; }); });
  let drawerPinned = $state(true);
  let teleFolded = $state(false);
  let bayW = $state(0);
  let bayH = $state(0);
  let peaks = $state<SamplePeaks | null>(null);
  let launchpadPanel = $state<LaunchpadPanel>();
  let padEditor = $state<PadEditor>();
  type Drag = { source: 'pad' | 'files'; fromBank: number; fromX: number; fromY: number; paths: string[];
    label: string; color: string; startX: number; startY: number; x: number; y: number; ctrl: boolean; active: boolean; external: boolean };
  let drag = $state<Drag | null>(null);
  let dropTarget = $state<DropTarget>(null);
  let hoverBank = $state<number | null>(null);
  let bankTimer = 0;
  let suppressClick = false;
  let pointerHeld = false;
  let padRects: ReturnType<LaunchpadPanel['rects']> = [];
  let bankRects: ReturnType<LaunchpadPanel['bankRects']> = [];
  const invalidDrop = $derived(drag?.source === 'files' && !drag.paths.some(isAudioName));
  $effect(() => { if (drag?.active) return pushEsc(cancelDrag); });
  $effect(() => {
    if (!drag?.active || !invalidDrop) return;
    const previous = document.body.style.cursor; document.body.style.cursor = 'not-allowed';
    return () => { document.body.style.cursor = previous; };
  });
  function measureDropTargets() { padRects = launchpadPanel?.rects() ?? []; bankRects = launchpadPanel?.bankRects() ?? []; }
  function cancelDrag() { if (drag?.active && !drag.external && pointerHeld) suppressClick = true; clearTimeout(bankTimer); drag = null; dropTarget = null; hoverBank = null; }
  function beginPadDrag(note: number, e: PointerEvent) {
    const x = note % 10 - 1, y = Math.floor(note / 10) - 1;
    const pad = bank.pads.find(p => p.x === x && p.y === y);
    if (!pad?.sample) return;
    measureDropTargets();
    drag = { source: 'pad', fromBank: activeBank, fromX: x, fromY: y, paths: [], label: padLabel(pad.sample), color: ledToCss(pad.color), startX: e.clientX, startY: e.clientY, x: e.clientX, y: e.clientY, ctrl: e.ctrlKey, active: false, external: false };
  }
  function beginFileDrag(path: string, e: PointerEvent) {
    measureDropTargets();
    drag = { source: 'files', fromBank: activeBank, fromX: 0, fromY: 0, paths: [path], label: path.split(/[\\/]/).at(-1) ?? path, color: 'var(--orange)', startX: e.clientX, startY: e.clientY, x: e.clientX, y: e.clientY, ctrl: e.ctrlKey, active: false, external: false };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }
  function updateDrag(x: number, y: number, ctrl: boolean) {
    if (!drag) return;
    drag.x = x; drag.y = y; drag.ctrl = ctrl;
    if (!drag.active) {
      if (!passedThreshold(x - drag.startX, y - drag.startY)) return;
      drag.active = true;
      if (drag.source === 'pad') launchpadPanel?.consume((drag.fromY + 1) * 10 + drag.fromX + 1);
    }
    dropTarget = hitTest(x, y, padRects, bankRects, drawer === 'editor' && selXY && padEditor ? padEditor.rect() : null);
    const nextBank = dropTarget?.kind === 'bank' ? dropTarget.bank : null;
    if (nextBank !== hoverBank) {
      clearTimeout(bankTimer); hoverBank = nextBank;
      if (nextBank !== null && nextBank !== activeBank) bankTimer = window.setTimeout(() => {
        if (!drag?.active || hoverBank !== nextBank) return;
        chooseBank(nextBank); measureDropTargets();
      }, BANK_HOVER_MS);
    }
  }
  async function finishDrop() {
    if (!drag) return;
    const source = drag, target = dropTarget, xy = dropXY;
    if (!source.active) { cancelDrag(); return; }
    if (!source.external) { suppressClick = true; window.setTimeout(() => { suppressClick = false; }, 0); }
    const invalid = invalidDrop;
    cancelDrag();
    if (invalid) { if (xy) showToast('Not an audio file'); return; }
    if (!xy || !target) return;
    if (source.source === 'pad' && source.fromBank === activeBank && source.fromX === xy.x && source.fromY === xy.y) return;
    const toBank = activeBank;
    try {
      if (source.source === 'pad') {
        const set = await invoke<SetModel | null>('move_pad', { fromBank: source.fromBank, fromX: source.fromX, fromY: source.fromY, toBank, toX: xy.x, toY: xy.y, copy: source.ctrl });
        if (set) { replaceSet(set); selected = (xy.y + 1) * 10 + xy.x + 1; }
      } else {
        const report = await invoke<DropReport>('drop_files', { bank: toBank, x: xy.x, y: xy.y, paths: source.paths });
        if (report) {
          replaceSet(report.set);
          if (report.imported) confirm(`Imported ${report.imported}`);
          if (report.leftover) showToast(`${report.leftover} file${report.leftover === 1 ? '' : 's'} not imported — bank full`);
          if (report.rejected && !report.imported) showToast('Not an audio file');
        }
      }
      refreshHistory(); await refreshMissing();
    } catch (e) { showToast(errorText(e)); }
  }
  function externalEnter(paths: string[], position: { x: number; y: number }) {
    if (mode !== 'play') return;
    const point = physicalToCss(position, devicePixelRatio);
    measureDropTargets();
    drag = { source: 'files', fromBank: activeBank, fromX: 0, fromY: 0, paths, label: paths.length > 1 ? `${paths.length} files` : paths[0]?.split(/[\\/]/).at(-1) ?? '', color: 'var(--orange)', startX: point.x, startY: point.y, x: point.x, y: point.y, ctrl: false, active: true, external: true };
    updateDrag(point.x, point.y, false);
  }

  const layout = $derived(solvePlayLayout({
    width: bayW, height: bayH, drawerOpen: drawer !== null, drawerPinned, teleFolded, seqFolded, keysFolded, lanes: Math.max(1, currentSet.pattern.lanes.length),
  }));

  const bank = $derived(currentSet.banks[activeBank]);
  const selXY = $derived(selected === null ? null : { x: (selected % 10) - 1, y: Math.floor(selected / 10) - 1 });
  const selectedPad = $derived(selXY ? bank?.pads.find((p) => p.x === selXY.x && p.y === selXY.y) ?? null : null);
  const dropXY = $derived(dropTarget?.kind === 'pad' ? dropTarget : dropTarget?.kind === 'editor' && drag?.source === 'files' ? selXY : null);
  const dragTag = $derived(drag && dropXY && !(drag.source === 'pad' && drag.fromBank === activeBank && drag.fromX === dropXY.x && drag.fromY === dropXY.y)
    ? dropTag(drag.source, !!bank.pads.find(p => p.x === dropXY.x && p.y === dropXY.y), drag.ctrl, drag.paths.filter(isAudioName).length) : '');

  // Frame order: index (7 - y) * 8 + x, top-left first.
  const padNames = $derived.by(() => {
    const names = Array<string>(64).fill('');
    for (const p of bank?.pads ?? []) if (p.sample) names[(7 - p.y) * 8 + p.x] = padLabel(p.sample);
    return names;
  });
  const missingSet = $derived(new Set(missing.filter(m => m.bank === activeBank).map(m => `${m.x},${m.y}`)));
  const padLabels = $derived.by(() => {
    const labels = Array<string>(64).fill('');
    for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) {
      const pad = bank?.pads.find(p => p.x === x && p.y === y) ?? null;
      labels[(7 - y) * 8 + x] = padAriaLabel(pad, x, y, currentSet.mixer.groups, !!pad?.sample && missingSet.has(`${x},${y}`));
    }
    return labels;
  });
  const padMissing = $derived.by(() => {
    const flags = Array<boolean>(64).fill(false);
    for (const p of bank?.pads ?? []) if (p.sample && missingSet.has(`${p.x},${p.y}`)) flags[(7 - p.y) * 8 + p.x] = true;
    return flags;
  });
  const selectedSample = $derived(selectedPad?.sample); // a string: edits to other pad fields do not re-request the waveform
  const selectedMissing = $derived(!!selectedPad?.sample && !!selXY && missingSet.has(`${selXY.x},${selXY.y}`));
  // The backend light painter streams the real frame; before the first frame (and in the browser
  // preview) the pads show their own colours, pressed pads white.
  const displayLeds = $derived.by<readonly Rgb[]>(() => {
    if (frameSeen) return leds;
    const frame: Rgb[] = createLedFrame();
    for (const p of bank?.pads ?? []) if (p.sample) frame[(7 - p.y) * 8 + p.x] = p.color;
    for (const note of pressed) frame[(8 - Math.floor(note / 10)) * 8 + (note % 10) - 1] = [63, 63, 63];
    return frame;
  });

  // A dropout is "recent" for 10 s after it happened; re-evaluated every second.
  const statusStamp = new WeakMap<object, number>();
  let secondTick = $state(0);
  const dropoutIsRecent = $derived.by(() => {
    secondTick;
    const status = audioStatus;
    if (!status) return false;
    if (!statusStamp.has(status)) statusStamp.set(status, performance.now());
    return dropoutRecent(status.lastDropoutAgoMs, performance.now() - statusStamp.get(status)!);
  });
  // The registry knows the LCXL (Task 11 bridge reports it); no row means disconnected.
  const lcxlStatus = $derived(roleStatus(midiDevices, 'lcxl'));
  const lcxlState = $derived<NoticeInputs['lcxl']>(lcxlStatus === 'bootloader' ? 'disconnected' : lcxlStatus);
  const launchpadStatus = $derived(roleStatus(midiDevices, 'launchpad'));
  const notices = $derived(activeNotices({ learnArmed: learnArmed?.kind ?? null, audioLost: audioStatus?.state === 'lost' || audioStatus?.state === 'error',
    backup, saveFailed: saveStatus.state === 'failed' ? saveStatus.reason : null,
    launchpad: tauri ? deviceState : 'connected', lcxl: tauri ? lcxlState : 'connected', missing: missing.length,
    asioFallback: audioStatus?.asioFallback ?? false, dropoutRecent: dropoutIsRecent, confirmations, keysAbsent: tauri && !keysPresent && !keysFolded, dismissed }));
  const lampClass = (state: string) => ({ connected: 'ok', running: 'ok', busy: 'warn', bootloader: 'error', stopped: 'error', lost: 'error', error: 'error' } as Record<string, string>)[state] ?? '';
  const audioLamp = $derived(audioStatus?.state === 'running' && dropoutIsRecent ? 'warn' : lampClass(audioStatus?.state ?? 'stopped'));

  // The engine feed (scope, spectrum, levels) runs only while it can be seen: Play mode, window visible.
  let windowVisible = $state(typeof document === 'undefined' || document.visibilityState === 'visible');
  $effect(() => {
    if (!tauri) return;
    void call('set_telemetry', { on: mode === 'play' && windowVisible }).catch(() => {});
  });

  function logMidi(text: string) {
    midiLog = pushLog(midiLog, { time: new Date().toLocaleTimeString(), text });
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
    try { const set = await invoke<SetModel | null>(cmd); if (set) { replaceSet(set); confirm(cmd === 'undo' ? 'Undone' : 'Redone'); await refreshMissing(); } refreshHistory(); }
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

  // The prototype games run in a sandboxed frame that cannot open MIDI itself, and the app already
  // holds the Launchpad: pad and button events go in as MIDI-shaped messages, and the game's LEDs
  // come back here to be sent as one full frame (grid, side and top buttons).
  let protoFrame = $state<HTMLIFrameElement | undefined>();
  const protoLeds = new Map<number, [number, number, number]>();
  function resetProtoLeds() {
    protoLeds.clear();
    for (let row = 1; row <= 8; row++) for (let col = 1; col <= 9; col++) protoLeds.set(row * 10 + col, [0, 0, 0]);
    for (let led = 104; led <= 111; led++) protoLeds.set(led, [0, 0, 0]);
  }
  resetProtoLeds();
  const protoActive = () => mode === 'games' && game !== 'horde';
  function toProto(message: object) {
    protoFrame?.contentWindow?.postMessage(message, '*'); // the frame's origin is opaque (sandbox)
  }
  function flushProtoLeds() {
    if (tauri) void invoke('send_led_leds', { leds: [...protoLeds] }).catch(() => {});
  }
  function protoLoaded() {
    resetProtoLeds();
    toProto({ lpd: 'status', state: deviceState });
    flushProtoLeds(); // blank pads, so nothing of the previous game lingers
  }
  function onProtoMessage(e: MessageEvent) {
    if (!protoActive() || !protoFrame || e.source !== protoFrame.contentWindow) return;
    const data = e.data as { lpd?: string; leds?: unknown };
    if (data?.lpd === 'req_status') {
      toProto({ lpd: 'status', state: deviceState });
      return;
    }
    if (data?.lpd !== 'leds' || !Array.isArray(data.leds)) return;
    let changed = false;
    for (const led of data.leds) {
      if (!Array.isArray(led) || led.length !== 4 || !protoLeds.has(led[0])) continue;
      const rgb = led.slice(1).map((v) => Math.min(63, Math.max(0, Math.round(Number(v)) || 0))) as [number, number, number];
      const old = protoLeds.get(led[0])!;
      if (old[0] !== rgb[0] || old[1] !== rgb[1] || old[2] !== rgb[2]) { protoLeds.set(led[0], rgb); changed = true; }
    }
    if (changed) flushProtoLeds();
  }

  /** The independent device and preference requests: none waits for another, none blocks the set. */
  async function loadDeviceState() {
    const [config, keys, audio, devices] = await Promise.allSettled([
      invoke<AppConfig>('get_config'), invoke<KeysStatus>('keys_status'),
      invoke<AudioStatus>('get_audio_status'), invoke<MidiDevice[]>('midi_devices'),
    ]);
    if (config.status === 'fulfilled' && config.value) { libraryDir = config.value.libraryDir; soundfontPath = config.value.soundfont; }
    if (keys.status === 'fulfilled' && keys.value) keysStatus = keys.value;
    // The preview answers with a demo status; the desktop app with the live one.
    if (audio.status === 'fulfilled' && audio.value) audioFeed.snapshot(audio.value);
    if (devices.status === 'fulfilled' && devices.value) midiFeed.snapshot(structuredClone(devices.value));
  }

  /** Accept a load status from an event or a snapshot: only a strictly newer revision counts. */
  function applyLoad(incoming: LoadStatus) {
    const next = acceptLoad(load, incoming);
    if (next === load) return;
    load = next;
    if (next.phase === 'ready') void enterReady(next.requestId);
    else if (next.phase === 'needsSet' || next.phase === 'failed') void settleOutcome(next.requestId);
  }

  /** Subscribe (and wait for the subscription), then ask for the current status. */
  async function followLoad() {
    startupError = null;
    try {
      const stop = await listen<LoadStatus>('load-status', applyLoad);
      if (disposed) stop(); else unlisten.push(stop);
    } catch (e) { startupError = `Couldn't follow startup progress — ${errorText(e)}`; return; }
    try { applyLoad(await call<LoadStatus>('get_load_status')); }
    catch (e) { startupError = `Couldn't read the startup state — ${errorText(e)}`; }
  }

  /** A request ended without a usable set: learn why (not found, unreadable, first run) for the choices screen. */
  async function settleOutcome(requestId: number) {
    if (hasSet) return; // a later operation failed: the one that started it reports it
    try {
      const outcome = await call<Startup>('get_startup');
      if (load.requestId === requestId && outcome) startup = outcome;
    } catch (e) { startupError = `Couldn't read the startup state — ${errorText(e)}`; }
  }

  /** A set is ready: fetch what the interface shows, all at once, and apply it only if nothing newer happened. */
  async function enterReady(requestId: number) {
    const token = setGate.next();
    startupError = null;
    const result = await fetchReadyState({
      set: () => call<SetModel>('get_current_set'), startup: () => call<Startup>('get_startup'), bank: () => call<number>('get_current_bank'),
      save: () => call<SaveStatus>('get_save_status'), history: () => call<typeof history>('history_state'),
      missing: () => call<MissingPad[]>('missing_samples'), status: () => call<LoadStatus>('get_load_status'),
    }, requestId, () => setGate.isCurrent(token));
    if (result.kind === 'stale') return;
    if (result.kind === 'error') { startupError = `Couldn't read the set — ${result.error}`; return; }
    replaceSet(result.set);
    startup = result.startup;
    if (startup.kind === 'loaded') { setPath = startup.path; setName = startup.name; backup = startup.fromBackup ? { savedAt: startup.backupSavedAt } : null; }
    if (result.bank !== null) activeBank = result.bank;
    if (result.save) saveStatus = result.save;
    if (result.history) history = result.history;
    if (result.missing) missing = result.missing;
    dismissed = new Set(); powerError = null; selected = null; peaks = null;
    hasSet = true;
    performance.mark('lpd:set-ready');
  }

  /** TRY AGAIN on a startup error: repeat whichever step failed. */
  function recoverStartup() {
    startupError = null;
    if (load.phase === 'ready') void enterReady(load.requestId);
    else if (load.phase === 'needsSet' || load.phase === 'failed') void settleOutcome(load.requestId);
    else void followLoad();
  }

  /** Read the configured last set again. The backend refuses while a set is still loading. */
  async function retryStartup() {
    if (manageInFlight > 0) return;
    manageInFlight++; powerError = null;
    const token = setGate.next();
    try {
      await invoke('retry_startup');
      if (setGate.isCurrent(token)) applyLoad(await call<LoadStatus>('get_load_status'));
    } catch (e) { if (setGate.isCurrent(token)) powerError = errorText(e); }
    finally { manageInFlight--; }
  }

  // A later operation (the player opened another set) shows its progress over the set that stays usable
  // underneath; a quick one finishes before the panel would show.
  const activeRequest = $derived(hasSet && !isTerminal(load.phase) ? load.requestId : 0);
  let overlayOn = $state(false);
  $effect(() => {
    if (!activeRequest) { overlayOn = false; return; }
    const timer = window.setTimeout(() => { overlayOn = true; }, 250);
    return () => window.clearTimeout(timer);
  });

  onMount(() => {
    try { const prefs = readDisplay(window.localStorage); crt = prefs.level; bend = prefs.bend; } catch { crt = 'low'; bend = 'flat'; }
    try { seqFolded = window.localStorage.getItem(SEQ_FOLD_KEY) === 'true'; } catch { seqFolded = false; }
    try { keysFolded = window.localStorage.getItem(KEYS_FOLD_KEY) === 'true'; keysOctave = Math.min(2, Math.max(-1, Number(window.localStorage.getItem(KEYS_OCTAVE_KEY)) || 0)); } catch { keysFolded = false; keysOctave = 0; }
    if (window.innerWidth >= 1780) drawer = 'editor';
    const stopActivity = runActivity();
    const secondTimer = window.setInterval(() => { secondTick++; }, 1000);
    const onVisibility = () => { windowVisible = document.visibilityState === 'visible'; };
    document.addEventListener('visibilitychange', onVisibility);
    const onFullscreen = () => { if (!document.fullscreenElement) gameFull = false; };
    document.addEventListener('fullscreenchange', onFullscreen);
    const swallowDragClick = (e: MouseEvent) => { if (suppressClick) { e.preventDefault(); e.stopImmediatePropagation(); suppressClick = false; } };
    window.addEventListener('click', swallowDragClick, true);
    const trackDown = () => { pointerHeld = true; };
    const trackUp = () => { pointerHeld = false; if (suppressClick) window.setTimeout(() => { suppressClick = false; }, 0); };
    const trackCtrl = (e: KeyboardEvent) => { if (e.key === 'Control' && drag && !drag.external) drag.ctrl = e.type === 'keydown'; };
    window.addEventListener('pointerdown', trackDown, true);
    window.addEventListener('pointerup', trackUp, true);
    window.addEventListener('pointercancel', trackUp, true);
    window.addEventListener('keydown', trackCtrl);
    window.addEventListener('keyup', trackCtrl);
    window.addEventListener('message', onProtoMessage);

    function subscribe<T>(event: string, fn: (payload: T) => void) {
      void listen<T>(event, fn).then(stop => { if (disposed) stop(); else unlisten.push(stop); }).catch(e => showToast(errorText(e)));
    }
    if (tauri) {
      subscribe<{ state: NoticeInputs['launchpad']; message: string }>('launchpad-status', (event) => {
        deviceState = event.state;
        logMidi(event.message);
        if (protoActive()) toProto({ lpd: 'status', state: event.state });
      });
      subscribe<{ note: number; pressed: boolean }>('launchpad-pad', (event) => {
        logMidi(`PAD ${event.note} ${event.pressed ? 'DOWN' : 'UP'}`);
        padInput({ ...event, source: 'hardware' });
      });
      subscribe<{ index: number; pressed: boolean }>('launchpad-top', (event) => {
        if (protoActive()) toProto({ lpd: 'midi', data: [0xb0, 104 + event.index, event.pressed ? 127 : 0] });
      });
      subscribe<[number, number, number][]>('launchpad-frame', (event) => {
        if (mode === 'games') return;
        leds = event.map(([r, g, b]) => [r, g, b] as const);
        frameSeen = true;
      });
      subscribe<MidiDevice[]>('midi-devices', (event) => midiFeed.event(event));
      subscribe<{ text: string }>('midi-log', (event) => logMidi(event.text));
      subscribe<{ groups: SetModel['mixer']['groups']; masterDb: number }>('mixer-changed', (event) => {
        currentSet = { ...currentSet, mixer: { ...currentSet.mixer, groups: event.groups, masterDb: event.masterDb } };
      });
      subscribe<{ binding: Binding }>('learn-bound', (event) => void onLearnBound(event.binding));
      subscribe<{ reason: string }>('learn-ignored', (event) => showToast(event.reason));
      subscribe<{ note: number; velocity: number; on: boolean }>('keys-note', (event) => {
        const next = new Set(keysDown);
        if (event.on) { next.add(event.note); keysLast = { note: event.note, velocity: event.velocity }; } else next.delete(event.note);
        keysDown = next;
      });
      subscribe<{ pitch: number; mod: number }>('keys-wheel', (event) => { keysWheel = event; });
      subscribe<KeysStatus>('keys-status', (event) => { keysStatus = event; });
      subscribe<{ values: [string, number][] }>('synth-params', (event) => {
        for (const [id, v] of event.values) setParam(currentSet.keys.patch.params, id, v);
        currentSet.keys.modified = true;
      });
      subscribe<SeqPosition>('seq-position', (event) => { seqPos = event; seqAt = performance.now(); seqRec = event.rec; });
      subscribe<{ bank: number; x: number; y: number }>('pad-hit', (event) => padHit(event.bank, event.x, event.y));
      subscribe<SetModel>('set-changed', (event) => { replaceSet(event); });
      subscribe<SaveStatus>('save-status', (event) => { saveStatus = event; });
      subscribe<AudioStatus>('audio-status', (event) => audioFeed.event(event));
      subscribe<TelemetryFrame>('telemetry', (event) => pushFrame(event));
      subscribe<{ paths: string[]; position: { x: number; y: number } }>('tauri://drag-enter', e => externalEnter(e.paths, e.position));
      subscribe<{ position: { x: number; y: number } }>('tauri://drag-over', e => { if (drag?.external) { const p = physicalToCss(e.position, devicePixelRatio); updateDrag(p.x, p.y, false); } });
      subscribe<{ paths: string[]; position: { x: number; y: number } }>('tauri://drag-drop', e => { externalEnter(e.paths, e.position); void finishDrop(); });
      subscribe<unknown>('tauri://drag-leave', cancelDrag);
    }
    // After the listeners are requested: the initial fetches must not overwrite a newer event.
    void loadDeviceState();
    if (staged) void followLoad(); else void enterReady(1);
    return () => {
      sendRouteSoon.flush();
      disposed = true;
      clearTimeout(historyTimer);
      confirmationTimers.forEach(clearTimeout);
      cancelDrag();
      window.removeEventListener('click', swallowDragClick, true);
      window.removeEventListener('pointerdown', trackDown, true);
      window.removeEventListener('pointerup', trackUp, true);
      window.removeEventListener('pointercancel', trackUp, true);
      window.removeEventListener('keydown', trackCtrl);
      window.removeEventListener('keyup', trackCtrl);
      window.removeEventListener('message', onProtoMessage);
      stopActivity();
      clearInterval(secondTimer);
      document.removeEventListener('visibilitychange', onVisibility);
      document.removeEventListener('fullscreenchange', onFullscreen);
      unlisten.forEach((fn) => fn());
    };
  });

  // Editor waveform: peaks of the selected pad's sample from the backend (a stand-in shape in the preview).
  $effect(() => {
    // Only while the pad editor, the one consumer of the waveform, is on screen.
    const visible = mode === 'play' && drawer === 'editor' && hasSet;
    const sample = visible ? selectedSample : undefined;
    peaksNonce;
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

  // Screens outside Play are fetched the first time they are opened and kept afterwards.
  type Optional = 'setup' | 'horde';
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  let optional = $state<Partial<Record<Optional, Component<any, any>>>>({});
  let optionalState = $state<Record<Optional, 'idle' | 'loading' | 'error'>>({ setup: 'idle', horde: 'idle' });
  const importers = { setup: () => import('./components/SetupPanel.svelte'), horde: () => import('./components/HordeGame.svelte') };
  async function loadOptional(name: Optional) {
    if (optional[name] || optionalState[name] === 'loading') return;
    optionalState[name] = 'loading';
    try {
      const module = await importers[name]();
      if (!disposed) { optional[name] = module.default as Component<any, any>; optionalState[name] = 'idle'; }
    } catch (e) {
      console.warn(`Couldn't load the ${name} screen:`, e);
      if (!disposed) optionalState[name] = 'error';
    }
  }
  $effect(() => {
    if (mode === 'setup') void loadOptional('setup');
    else if (mode === 'games' && game === 'horde') void loadOptional('horde');
  });

  function saveDisplayPrefs() {
    try { saveDisplay(window.localStorage, { level: crt, bend }); } catch { /* the switch still works without storage */ }
  }
  function setCrt(level: CrtLevel) {
    crt = level;
    saveDisplayPrefs();
  }
  function setBend(next: Bend) {
    bend = next;
    saveDisplayPrefs();
  }

  function padInput(input: PadInput) {
    if (mode === 'games' && game === 'horde') {
      hordeComponent?.padInput(input.note, input.pressed);
      return;
    }
    if (protoActive()) {
      if (input.source === 'hardware') toProto({ lpd: 'midi', data: [0x90, input.note, input.pressed ? 127 : 0] });
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
        // While the engine feed is live it reports the real level; hits only stand in without it.
        if (g && !feedLive()) hitGroup(pad.mixGroup, hitAmount(g.volumeDb + pad.volumeDb, anySolo ? g.solo : !g.mute));
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
  function clearPad(x: number, y: number) {
    void invoke<SetModel>('clear_pad', { bank: activeBank, x, y })
      .then(set => { if (set) replaceSet(set); confirm('Pad cleared'); void refreshMissing(); })
      .catch(e => showToast(errorText(e)));
  }
  async function locateSample(x: number, y: number) {
    const bank = activeBank; // the file dialog is not modal: the bank may change while it is open
    try {
      const path = await invoke<string | null>('pick_file', { title: 'Locate sample', extensions: ['wav', 'aif', 'aiff', 'flac', 'mp3', 'ogg'] });
      if (!path) return;
      const set = await invoke<SetModel>('locate_sample', { bank, x, y, path });
      if (set) replaceSet(set);
      await refreshMissing(); peaksNonce++;
    } catch (e) { showToast(`Locate failed — ${errorText(e)}`); }
  }
  function openPadMenu(x: number, y: number, at: { x: number; y: number }) {
    selected = (y + 1) * 10 + x + 1;
    padMenu = { x, y, at };
  }
  const padMenuItems = $derived.by(() => {
    if (!padMenu) return [];
    const { x, y } = padMenu, pad = bank.pads.find(p => p.x === x && p.y === y);
    const filled = !!pad?.sample;
    return [
      { label: 'Learn MIDI note…', action: () => void armLearnPad(activeBank, x, y) },
      ...(filled ? [
        { label: 'Clear pad', shortcut: 'Del', action: () => clearPad(x, y) },
        { label: 'Show sample in Explorer', action: () => void invoke('reveal_path', { path: pad!.sample }).catch(e => showToast(errorText(e))) },
      ] : []),
    ];
  });
  function updateGroup(index: number, group: Group) {
    currentSet.mixer.groups[index] = group;
    void invoke('update_group', { group: index, groupData: group });
  }
  function updateMaster(masterDb: number) {
    currentSet.mixer.masterDb = masterDb;
    void invoke('update_master_db', { masterDb });
  }
  /** KEYS | OTHER for a generic input: the switch moves at once, the backend saves it. */
  async function setMidiRole(port: string, role: 'keys' | 'other') {
    midiDevices = midiDevices.map(d => d.port === port ? { ...d, role } : d);
    try { await invoke('set_midi_role', { port, role }); }
    catch (e) { showToast(errorText(e)); const list = await invoke<MidiDevice[]>('midi_devices').catch(() => null); if (list) midiDevices = list; }
  }
  function stopAll() {
    void invoke('stop_all');
  }

  /**
   * New / Open / Default. The outcome arrives as load-status events (which then fetch the set); the
   * command's own answer only reports an error. `supersede` is the explicit way out of a slow load.
   */
  async function manageSet(kind: 'new' | 'open' | 'default', supersede = false) {
    if (!staged) return;
    if (manageInFlight > 0 && !supersede) return; // no double submission
    manageInFlight++;
    try {
      let dir: string | null = null;
      if (kind !== 'default') {
        const defaultDir = await invoke<string>('default_sets_dir');
        dir = await invoke<string | null>('pick_folder', { title: kind === 'new' ? 'New set — choose an empty folder' : 'Open set folder', defaultDir });
        if (!dir) return;
      }
      const token = setGate.next();
      powerError = null;
      try {
        if (kind === 'default') await invoke('open_default_set');
        else await invoke(kind === 'new' ? 'new_set' : 'open_set', { dir });
        // The ready notice normally arrived first; asking once more covers a lost one.
        if (setGate.isCurrent(token)) applyLoad(await call<LoadStatus>('get_load_status'));
      } catch (e) {
        if (!setGate.isCurrent(token)) return; // superseded: the newer request speaks for itself
        if (!hasSet) powerError = errorText(e);
        else showToast(errorText(e));
      }
    } catch (e) {
      showToast(errorText(e));
    } finally { manageInFlight--; }
  }

  async function loadBrowserFile(path: string) {
    if (!selXY) { showToast('Select a pad first'); return; }
    try {
      const report = await invoke<DropReport>('load_into_pad', { bank: activeBank, ...selXY, path });
      if (report) {
        replaceSet(report.set);
        if (report.imported) confirm(`Imported ${report.imported}`);
        if (report.leftover) showToast(`${report.leftover} file${report.leftover === 1 ? '' : 's'} not imported — bank full`);
        if (report.rejected && !report.imported) showToast('Not an audio file');
        refreshHistory(); await refreshMissing();
      }
    } catch (e) { showToast(errorText(e)); }
  }

  /** Games own the Launchpad pads and LEDs; every other screen leaves them to the sampler. */
  function setMode(next: typeof mode) {
    cancelDrag();
    releaseKeyboard();
    const was = mode;
    mode = next;
    if (next !== 'games') exitGameFull();
    if ((was === 'games') !== (next === 'games')) {
      void invoke<number>('set_input_mode', { mode: next === 'games' ? 'game' : 'sampler' })
        .then((b) => { if (typeof b === 'number') activeBank = b; if (next === 'games' && protoActive()) flushProtoLeds(); })
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
      if (key === 'n' || key === 'o') void manageSet(key === 'n' ? 'new' : 'open');
      else if (key === 'z' || key === 'y') void undoRedo(key === 'y' || event.shiftKey ? 'redo' : 'undo');
      else if (key === '1') setMode('play');
      else if (key === ',') setMode('setup');
      else if (key === '2') { /* TRAIN remains a blank cap. */ }
      else if (key === 'k' && mode === 'play') setKeysFolded(!keysFolded);
      else if ((key === 'e' || key === 'b') && mode === 'play') {
        const panel = key === 'e' ? 'editor' : 'browser'; drawer = drawer === panel ? null : panel;
      }
      else handled = false;
    } else if (mode === 'play' && hasSet) {
      const gridFocused = !!target.closest('.pad-grid') || document.activeElement === document.body;
      if (event.altKey && /^[1-8]$/.test(key)) chooseBank(Number(key) - 1);
      else if (key === 'pageup' || key === 'pagedown') chooseBank(Math.max(0, Math.min(7, activeBank + (key === 'pageup' ? -1 : 1))));
      else if (key === 'f2') {
        const channel = target.closest<HTMLElement>('.ms-col[data-group]');
        if (channel) { const r = channel.getBoundingClientRect(); channelMenu(Number(channel.dataset.group), r.left, r.bottom, true); } else renaming = activeBank;
      } else if (key === 'delete' && selXY) { if (selectedPad?.sample) clearPad(selXY.x, selXY.y); }
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

<svelte:window onpointerdown={(e) => { if (mixPop && !mixPanel?.contains(e.target as Node) && !(e.target instanceof Element && e.target.closest('.chan-pop'))) mixPop = null; }} onkeydown={keydown} onkeyup={keyup} onblur={() => { releaseKeyboard(); cancelDrag(); }}
  onpointermove={e => { if (drag && !drag.external) updateDrag(e.clientX, e.clientY, e.ctrlKey); }}
  onpointerup={e => { if (drag && !drag.external) { drag.ctrl = e.ctrlKey; void finishDrop(); } }} onpointercancel={cancelDrag} />

{#if drag?.active}<DragGhost x={drag.x} y={drag.y} color={drag.color} label={drag.label} />{/if}
{#if mixPop}
  <div class="mix-pop" bind:this={mixPanel} style:position="fixed" style:z-index="40" style:left="{Math.max(4, Math.min(mixPop.x, innerWidth - 304))}px" style:top="{Math.max(4, Math.min(mixPop.y, innerHeight - 364))}px">
    <MixerSlim groups={currentSet.mixer.groups} masterDb={currentSet.mixer.masterDb} height={360} mode="slim" inputs={channelInputs} onupdategroup={updateGroup} onupdatemaster={updateMaster} onchannel={channelMenu} />
  </div>
{/if}
{#if chanPop}
  {@const i = chanPop.index}
  {@const keys = currentSet.keys}
  {@const here = keys.output === i}
  <ChannelPopover index={i} group={currentSet.mixer.groups[i]} inputs={channelInputs[i]} keysHere={here}
    keysWhere={keys.output !== null && !here ? `Keys: on ${groupLegend(keys.output, currentSet.mixer.groups[keys.output].name)}` : `Keys: ${SOUND_LABELS[keys.gmSound]}`}
    x={chanPop.x} y={chanPop.y} rename={chanPop.rename} onupdategroup={updateGroup}
    onkeys={() => updateKeys({ gmSound: keys.gmSound, volumeDb: keys.volumeDb, output: here ? null : i })} onclose={() => { chanPop = null; }} />
{/if}
{#if learnMenu}<ContextMenu x={learnMenu.x} y={learnMenu.y} items={learnMenu.items} onclose={() => { learnMenu = null; }} />{/if}
{#if padMenu}<ContextMenu x={padMenu.at.x} y={padMenu.at.y} items={padMenuItems} onclose={() => { padMenu = null; }} />{/if}

<div class="instrument" data-crt={crt} data-bend={bend === 'bent' && crt !== 'off' ? 'on' : 'off'} style:--crt={CRT_FACTORS[crt]}>
  <div class="console" class:game-full={gameFull}>
    <ConsoleBackdrop />
    <header class="top-strip">
      <div class="brand" aria-hidden="true">LPD·01</div>
      <SetMenu name={setName} path={setPath} onnew={() => void manageSet('new')} onopen={() => void manageSet('open')} ondefault={() => void manageSet('default')} onreveal={() => void invoke('reveal_path', { path: setPath }).catch(e => showToast(errorText(e)))} />
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
      {#if mode === 'play' && !hasSet}
        {#if (load.phase === 'needsSet' || load.phase === 'failed') && !startupError}
          <PowerOn {startup} error={powerError ?? (load.phase === 'failed' ? load.error : null)} launchpad={deviceState} lcxl={lcxlState} audio={audioStatus?.state ?? 'stopped'} busy={manageInFlight > 0}
            onnew={() => void manageSet('new')} onopen={() => void manageSet('open')} ondefault={() => void manageSet('default')} onretry={() => void retryStartup()} />
        {:else}
          <StartupLoading status={load} error={startupError} launchpad={deviceState} lcxl={lcxlState} audio={audioStatus?.state ?? 'opening'}
            onretry={startupError ? recoverStartup : undefined} onopen={() => void manageSet('open', true)} onreload={() => location.reload()} />
        {/if}
      {:else if mode === 'play'}
        <section
          class="play-screen"
          class:docked={layout.docked}
          style:--rowh="{layout.rowH}px"
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
                    bind:this={launchpadPanel}
                    ondragpad={beginPadDrag} onmenu={openPadMenu} labels={padLabels} missing={padMissing} dropTarget={dropXY}
                    dropTag={dragTag} {invalidDrop} hoverBank={hoverBank === activeBank ? null : hoverBank}
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
                  <MixerSlim
                    height={layout.rowH} mode={layout.mixMode}
                    groups={currentSet.mixer.groups}
                    masterDb={currentSet.mixer.masterDb}
                    onupdategroup={updateGroup}
                    onupdatemaster={updateMaster}
                    inputs={channelInputs}
                    onchannel={channelMenu}
                    onexpand={(x, y) => { mixPop = { x, y }; }}
                  />
                  <SynthPlate set={currentSet} width={layout.synthW} height={layout.rowH} engine={keysStatus.engine} onusesynth={() => void useSynth()} onparam={synthParam} {selectedRoute} onaddroute={addRoute} onupdateroute={updateRoute} onremoveroute={removeRoute} onroutedepth={routeDepth} onselectroute={(dest, slot) => { selectedRoute[dest] = slot; }} onblankroute={() => addRoute('env2', null)} />
                </div>
                <KeysPlate
                  short={layout.kh < 68} folded={keysFolded} set={currentSet} octave={keysOctave} deviceName={keysDeviceName} devicePresent={tauri ? keysPresent : true}
                  remoteDown={keysDown} bind:last={keysLast} wheel={keysWheel}
                  onsettings={updateKeys} onnote={keysNote} onpad={keysPad}
                  onpitch={(value) => void invoke('keys_pitch', { value })} onmod={(value) => void invoke('keys_mod', { value })}
                  onoctave={setKeysOctave} onfold={setKeysFolded}
                />
                <SequencerPlate
                  {pattern} banks={currentSet.banks} selected={selXY} {activeBank} position={seqPos} playing={seqPlaying} rec={seqRec} folded={seqFolded}
                  onplay={toggleSeqPlay} onrec={toggleSeqRec} onupdate={updatePattern} onaddlane={() => void addLane()}
                  onfold={setSeqFolded} onselect={selectLanePad} onaudition={(b, x, y) => void invoke('seq_audition_step', { bank: b, x, y })}
                />
              </div>
              {#if layout.tw > 0}
                <Telemetry mini={layout.mini} sampleRate={audioStatus?.sampleRate ?? 0} seq={seqPos} {seqAt} hits={seqHits} length={pattern.length} bpm={pattern.bpm} onfold={(f) => { teleFolded = f; }} />
              {/if}
            </div>
          </div>

          {#if drawer === 'browser'}
            <div class="drawer"><SampleBrowser bind:libraryDir selected={selXY ? { bank: activeBank, ...selXY } : null}
              onclose={() => drawer = null} onload={path => void loadBrowserFile(path)} ondragfile={beginFileDrag} onerror={showToast} /></div>
          {:else if drawer === 'editor' && selXY}
            <div class="drawer">
              <PadEditor
                bind:this={padEditor} drop={dropTarget?.kind === 'editor' && drag?.source === 'files' && !invalidDrop}
                bank={activeBank}
                x={selXY.x}
                y={selXY.y}
                pad={selectedPad}
                groups={currentSet.mixer.groups}
                {peaks}
                missing={selectedMissing}
                onupdate={updatePad}
                onlocate={() => void locateSample(selXY.x, selXY.y)}
                onclear={() => clearPad(selXY.x, selXY.y)}
                learning={learnArmed?.id === learnPadId(activeBank, selXY.x, selXY.y)}
                binding={padBinding}
                onlearn={() => void armLearnPad(activeBank, selXY.x, selXY.y)}
                onunbind={(index) => void unbindAt(index)}
                ontrigger={(x, y) => void invoke('trigger_pad', { x, y })}
                onrelease={(x, y) => void invoke('release_pad', { x, y })}
              />
            </div>
          {/if}
          <div class="dtabs" role="tablist" aria-label="Side panel" aria-orientation="vertical">
            <button class="key dtab" class:on={drawer === 'browser'} role="tab" aria-selected={drawer === 'browser'} title="Sample browser (Ctrl+B)" onclick={() => { drawer = drawer === 'browser' ? null : 'browser'; }}>BROWSER</button>
            <button class="key dtab" class:on={drawer === 'editor'} role="tab" aria-selected={drawer === 'editor'} title="Pad editor (Ctrl+E)" onclick={() => { drawer = drawer === 'editor' ? null : 'editor'; }}>EDITOR</button>
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
          <Plate label="" class="game-plate">
            {#if game === 'horde'}
              {#if optional.horde}
                {@const Horde = optional.horde}
                <Horde bind:this={hordeComponent} onframe={sendFrame} launchpad={deviceState} />
              {:else if optionalState.horde === 'error'}
                <div class="lazy" role="alert">Couldn't load Horde. <button class="key" onclick={() => void loadOptional('horde')}>TRY AGAIN</button></div>
              {:else}
                <div class="lazy" role="status">Loading Horde…</div>
              {/if}
            {:else}
              <iframe class="prototype" title={`${game} prototype`} src={`/games/${game}/index.html`} sandbox="allow-scripts" bind:this={protoFrame} onload={protoLoaded}></iframe>
            {/if}
          </Plate>
        </div>
      {:else}
        <div class="setup-scroll">
          <div class="setup">
            {#if optional.setup}
              {@const Panel = optional.setup}
              <Panel audioStatus={audioStatus} dropoutRecent={dropoutIsRecent} onaudiostatus={(status: AudioStatus) => { audioStatus = status; }} onerror={showToast}
                midiDevices={midiDevices} midiLog={midiLog} onrole={(port: string, role: 'keys' | 'other') => void setMidiRole(port, role)}
                keysStatus={keysStatus} soundfont={soundfontPath} onloadsoundfont={() => void loadSoundfont()} onclearsoundfont={() => void applySoundfont(null)}
                set={currentSet} onunbind={(index: number) => void unbindAt(index)} crt={crt} bend={bend} oncrt={setCrt} onbend={setBend}
                onopenlog={() => void invoke('open_log_folder').catch(e => showToast(errorText(e)))} />
            {:else if optionalState.setup === 'error'}
              <div class="lazy" role="alert">Couldn't load Setup. <button class="key" onclick={() => void loadOptional('setup')}>TRY AGAIN</button></div>
            {:else}
              <div class="lazy" role="status">Loading Setup…</div>
            {/if}
          </div>
        </div>
      {/if}
      {#if hasSet && overlayOn}
        <StartupLoading overlay status={load} launchpad={deviceState} lcxl={lcxlState} audio={audioStatus?.state ?? 'opening'} onopen={() => void manageSet('open', true)} />
      {/if}
    </main>

    <footer class="status-strip">
      <button class="device-status" onclick={() => setMode('setup')} aria-label="LAUNCHPAD: {launchpadStatus}. Open Setup">
        <span class="lamp {lampClass(launchpadStatus)}"></span><span class="silk">LAUNCHPAD</span>
      </button>
      <button class="device-status" onclick={() => setMode('setup')} aria-label="LCXL: {lcxlStatus}. Open Setup">
        <span class="lamp {lampClass(lcxlStatus)}"></span><span class="silk">LCXL</span>
      </button>
      <button class="device-status" onclick={() => setMode('setup')} aria-label="AUDIO: {audioStatus?.state ?? 'stopped'}. Open Setup">
        <span class="lamp {audioLamp}"></span><span class="silk">AUDIO</span>
      </button>
      <span class="lcd lcd-dim" class:audio-error={audioStatus?.state === 'error' || audioStatus?.state === 'lost'}>{audioLabel(audioStatus)}</span>
      <span class="lcd lcd-dim">DROPS {audioStatus?.dropouts ?? 0}</span>
      <span class="lcd lcd-dim status-path" title={setPath}>{middleTruncate(setPath, 48)}</span>
      {#key saveStatus}<span class="lamp" class:ok={saveStatus.state === 'saved'} class:blink={saveStatus.state === 'saved'} class:error={saveStatus.state === 'failed'} role="img" aria-label={saveStatus.state === 'failed' ? 'Save failed' : saveStatus.state === 'saved' ? 'Saved' : 'Saving…'}></span>{/key}
    </footer>
  </div>
  <CrtLayers level={crt} bent={bend === 'bent' && crt !== 'off'} />
</div>
