import { createPreviewSet } from './model.ts';
import { SYNTH_META } from './synth-meta.ts';
import type { PatchRequest } from './patchMenu.ts';
import { PREVIEW_MIDI } from './midi.ts';
import type { AudioDevice, AudioPrefs, AudioStatus, LoadStatus, Startup } from './model.ts';
const tauri = typeof window !== 'undefined' ? window.__TAURI__ : undefined;
export const isTauri = !!tauri;
/**
 * Browser preview only: `?startup=slow|error|empty|firstrun` plays a scripted `load-status` timeline
 * (no engine is involved), so the startup screens can be checked without a native build.
 */
const seam = !tauri && typeof location !== 'undefined' ? new URLSearchParams(location.search).get('startup') : null;
/** The UI follows backend load-status events: always in the desktop app, and in the preview when a seam is set. */
export const isStaged = !!tauri || !!seam;
/** The browser preview has no engine: a fixed, healthy status and two fake devices. */
const PREVIEW_DEVICES: AudioDevice[] = [
  { name: 'Speakers (Preview)', isDefault: true, bufferSizes: [64, 128, 256, 512, 1024] },
  { name: 'Headphones (Preview)', isDefault: false, bufferSizes: [128, 256, 512] },
];
const previewStatus = (prefs?: Partial<AudioPrefs>): AudioStatus => {
  const bufferSize = prefs ? prefs.bufferFrames ?? null : 256;
  const midiToBufferMs = Math.round((bufferSize ?? 256) / 48000 * 10000) / 10;
  return { state: 'running', backend: prefs?.backend === 'asio' ? 'ASIO' : 'WASAPI', deviceName: prefs?.device ?? 'Speakers (Preview)',
    sampleRate: 48000, bufferSize, channels: 2, dropouts: 0, lastDropoutAgoMs: null, asioFallback: false,
    latency: { midiToBufferMs, outputMs: 10, totalMs: Math.round((midiToBufferMs + 10) * 10) / 10 } };
};
const PREVIEW_LOADED: Startup = { kind: 'loaded', path: 'PREVIEW', name: 'PREVIEW KIT', fromBackup: false, backupSavedAt: null };
let previewStartup: Startup = seam === 'firstrun' ? { kind: 'firstRun' } : seam === 'error' ? { kind: 'unreadable', path: 'PREVIEW', name: 'PREVIEW KIT' } : PREVIEW_LOADED;
let previewLoad: LoadStatus = { requestId: 0, revision: 0, phase: 'settings', completed: 0, total: null, failed: 0, error: null };
const loadListeners = new Set<(status: LoadStatus) => void>();
let previewTimers: number[] = [];
function previewPublish(over: Partial<LoadStatus>, requestId = previewLoad.requestId) {
  previewLoad = { completed: 0, total: null, failed: 0, error: null, ...over, requestId, revision: previewLoad.revision + 1 } as LoadStatus;
  loadListeners.forEach(fn => fn(previewLoad));
}
/** Play `steps` (delay after the previous step, status) as request `id`, replacing any timeline still running. */
function previewPlay(id: number, steps: [number, Partial<LoadStatus>][], then?: () => void) {
  previewTimers.forEach(t => window.clearTimeout(t));
  previewTimers = [];
  previewPublish({ phase: 'readingSet' }, id);
  let at = 0;
  steps.forEach(([delay, step], i) => {
    at += delay;
    previewTimers.push(window.setTimeout(() => {
      previewPublish(step, id);
      if (i === steps.length - 1) then?.();
    }, at));
  });
}
const counted = (phase: 'loadingSamples' | 'preparingSamples', total: number, ms: number): [number, Partial<LoadStatus>][] =>
  Array.from({ length: total + 1 }, (_, n) => [n === 0 ? 0 : ms, { phase, completed: n, total }]);
function previewStartLoad(kind: string, id = previewLoad.requestId + 1) {
  const ready = () => { previewStartup = PREVIEW_LOADED; };
  if (kind === 'slow') previewPlay(id, [[400, { phase: 'readingSet' }], ...counted('loadingSamples', 32, 500), [200, { phase: 'installing' }], [200, { phase: 'ready' }]], ready);
  else if (kind === 'empty') previewPlay(id, [[600, { phase: 'installing' }], [300, { phase: 'ready' }]], ready);
  else if (kind === 'error') previewPlay(id, [[800, { phase: 'failed', error: 'Preview: set.json could not be read.' }]]);
  else if (kind === 'firstrun') previewPlay(id, [[300, { phase: 'needsSet' }]]);
  else if (kind === 'default') previewPlay(id, [[200, { phase: 'preparingSamples' }], ...counted('preparingSamples', 12, 150), ...counted('loadingSamples', 12, 150), [100, { phase: 'installing' }], [100, { phase: 'ready' }]], ready);
}
if (seam) setTimeout(() => previewStartLoad(seam, 1), 0);
const PREVIEW: Record<string, (args?: Record<string, unknown>) => unknown> = {
  get_startup: () => previewStartup,
  get_load_status: () => seam ? previewLoad : { requestId: 1, revision: 1, phase: 'ready', completed: 0, total: null, failed: 0, error: null },
  retry_startup: () => { previewStartLoad('slow'); return previewStartup; },
  open_default_set: () => { previewStartLoad('default'); return undefined; },
  new_set: () => { previewStartLoad('empty'); return undefined; },
  open_set: () => { previewStartLoad('slow'); return undefined; },
  default_sets_dir: () => 'PREVIEW',
  pick_folder: () => 'PREVIEW/Chosen',
  get_current_set: () => createPreviewSet(),
  get_current_bank: () => 0,
  get_save_status: () => ({ state: 'saved', reason: null }),
  history_state: () => ({ canUndo: false, canRedo: false }),
  missing_samples: () => [],
  list_dir: (args) => {
    const path = String(args?.path ?? 'PREVIEW');
    // Preview only: `?files=5000` fills the library root with that many files, to try a large library.
    const many = Number(new URLSearchParams(location.search).get('files'));
    if (many > 0 && path === 'PREVIEW') return Array.from({ length: many }, (_, i) => ({ name: `sample_${String(i).padStart(5, '0')}.wav`, path: `PREVIEW/sample_${i}.wav`, isDir: false }));
    if (path === 'PREVIEW') return ['Kicks', 'Snares', 'FX'].map(name => ({ name, path: `PREVIEW/${name}`, isDir: true }));
    const pattern = path.endsWith('Kicks') ? /kick/i : path.endsWith('Snares') ? /snare|clap|rim/i : /riser|impact|sweep|zap/i;
    return [...new Set(createPreviewSet().banks[0].pads.map(p => p.sample.split('/').at(-1)!))]
      .filter(name => pattern.test(name)).sort().map(name => ({ name, path: `${path}/${name}`, isDir: false }));
  },
  get_audio_status: () => previewStatus(),
  audio_devices: (args) => (args?.backend === 'asio' ? [] : PREVIEW_DEVICES),
  asio_available: () => ({ available: false, reason: 'Built without ASIO support' }),
  set_audio: (args) => previewStatus(args?.prefs as AudioPrefs),
  audio_reconnect: () => previewStatus(),
  midi_devices: () => PREVIEW_MIDI,
  synth_patches: () => ({ factory: SYNTH_META.factory.map(({id,name,category})=>({id,name,category})), user: [] }),
  synth_load_patch: args => {
    const source=args?.source as PatchRequest;
    const set=createPreviewSet();
    const factory='factory' in source?SYNTH_META.factory.find(p=>p.id===source.factory):undefined;
    if(factory){set.keys.patch={patchVersion:1,name:factory.name,category:factory.category,params:structuredClone(factory.params)};set.keys.patchSource={factory:factory.id};}
    else {set.keys.patch={patchVersion:1,name:'INIT',category:'keys',params:structuredClone(SYNTH_META.init)};set.keys.patchSource='init';}
    set.keys.modified=false;return set;
  },
  synth_revert: () => createPreviewSet(),
  keys_status: () => ({engine:'synth',name:'SYNTH',error:null}),
  patches_folder: () => 'PREVIEW/Patches',
  synth_table: () => undefined,
  get_config: () => ({ lastSet: null, libraryDir: null, soundfont: null, keysEngine: null,
    audio: { backend: 'wasapi', device: null, bufferFrames: null }, midiRoles: {} }),
};
export function invoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return tauri ? tauri.core.invoke<T>(cmd, args) : Promise.resolve(PREVIEW[cmd]?.(args) as T);
}
export function listen<T>(event: string, fn: (payload: T) => void): Promise<() => void> {
  if (!tauri && seam && event === 'load-status') {
    const handler = fn as unknown as (status: LoadStatus) => void;
    loadListeners.add(handler);
    return Promise.resolve(() => { loadListeners.delete(handler); });
  }
  return tauri ? tauri.event.listen<T>(event, e => fn(e.payload)) : Promise.resolve(() => {});
}
