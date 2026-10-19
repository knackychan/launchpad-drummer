import { createPreviewSet } from './model.ts';
import { PREVIEW_MIDI } from './midi.ts';
import type { AudioDevice, AudioPrefs, AudioStatus } from './model.ts';
const tauri = typeof window !== 'undefined' ? window.__TAURI__ : undefined;
export const isTauri = !!tauri;
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
const PREVIEW: Record<string, (args?: Record<string, unknown>) => unknown> = {
  get_startup: () => ({ kind: 'loaded', path: 'PREVIEW', name: 'PREVIEW KIT', fromBackup: false, backupSavedAt: null }),
  history_state: () => ({ canUndo: false, canRedo: false }),
  missing_samples: () => [],
  list_dir: (args) => {
    const path = String(args?.path ?? 'PREVIEW');
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
  get_config: () => ({ lastSet: null, libraryDir: null, soundfont: null,
    audio: { backend: 'wasapi', device: null, bufferFrames: null }, midiRoles: {} }),
};
export function invoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return tauri ? tauri.core.invoke<T>(cmd, args) : Promise.resolve(PREVIEW[cmd]?.(args) as T);
}
export function listen<T>(event: string, fn: (payload: T) => void): Promise<() => void> {
  return tauri ? tauri.event.listen<T>(event, e => fn(e.payload)) : Promise.resolve(() => {});
}
