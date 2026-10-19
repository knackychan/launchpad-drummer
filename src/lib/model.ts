export type PadMode = 'oneShot' | 'hold';

export interface Pad {
  x: number;
  y: number;
  sample: string;
  color: [number, number, number];
  volumeDb: number;
  pitchSemitones: number;
  trimStartMs: number;
  trimEndMs: number | null;
  mode: PadMode;
  chokeGroup: number | null;
  mixGroup: number;
}

export interface LightModes {
  flash: boolean;
  playState: boolean;
  ripple: boolean;
}

export interface Bank {
  name: string;
  lights: LightModes;
  humanizeDb: number;
  pads: Pad[];
}

export interface Group {
  name: string;
  volumeDb: number;
  pan: number;
  pitchSemitones: number;
  lowpassHz: number;
  mute: boolean;
  solo: boolean;
}

export interface Mixer {
  groups: Group[];
  masterDb: number;
}

export interface SetModel {
  version: number;
  banks: Bank[];
  mixer: Mixer;
}

export interface AudioLatency {
  midiToBufferMs: number;
  outputMs: number;
  totalMs: number;
}

export interface AudioStatus {
  state: 'running' | 'stopped' | 'lost' | 'error';
  backend: 'WASAPI' | 'ASIO';
  deviceName: string;
  sampleRate: number;
  /** null = the driver picks (AUTO). */
  bufferSize: number | null;
  channels: number;
  /** Output callbacks that arrived late since the stream started (or the last reset). */
  dropouts: number;
  /** Milliseconds since the last dropout when the status was read; null when none. */
  lastDropoutAgoMs: number | null;
  /** ASIO was asked for but could not be opened; WASAPI runs instead. */
  asioFallback: boolean;
  latency: AudioLatency;
}

export interface AudioDevice {
  name: string;
  isDefault: boolean;
  bufferSizes: number[];
}

export interface AsioInfo {
  available: boolean;
  /** "Built without ASIO support" | "No ASIO driver found" */
  reason: string | null;
}

/** Editor waveform data from the `sample_peaks` command. */
export interface SamplePeaks {
  peaks: number[];
  lengthMs: number;
  rate: number;
}

/** What the app opened at launch (`get_startup`). */
export type Startup =
  | { kind: 'loaded'; path: string; name: string; fromBackup: boolean; backupSavedAt: string | null }
  | { kind: 'firstRun' }
  | { kind: 'notFound'; path: string }
  | { kind: 'unreadable'; path: string; name: string };

/** A set that was just created or opened (`new_set` / `open_set`). */
export interface OpenResult {
  set: SetModel;
  path: string;
  name: string;
  fromBackup: boolean;
  backupSavedAt: string | null;
}

export interface AudioPrefs {
  backend: string;
  device: string | null;
  bufferFrames: number | null;
}

/** Per-machine settings (`get_config`). */
export interface AppConfig {
  lastSet: string | null;
  libraryDir: string | null;
  soundfont: string | null;
  audio: AudioPrefs;
  midiRoles: Record<string, string>;
}

export interface MissingPad {
  bank: number;
  x: number;
  y: number;
  sample: string;
}

export interface DropReport { set: SetModel; imported: number; leftover: number; rejected: number }

export interface KeysStatus {
  engine: string;
  name: string;
  error: string | null;
}

export function createDefaultPad(x: number, y: number): Pad {
  return {
    x,
    y,
    sample: '',
    color: [63, 20, 10],
    volumeDb: 0.0,
    pitchSemitones: 0.0,
    trimStartMs: 0.0,
    trimEndMs: null,
    mode: 'oneShot',
    chokeGroup: null,
    mixGroup: y,
  };
}

// LED-valid colour presets (0..63 per channel), two rows of eight in the pad editor.
export const LAUNCHPAD_PALETTE: readonly [number, number, number][] = [
  [63, 0, 0], [63, 20, 0], [63, 40, 0], [63, 63, 0], [30, 63, 0], [0, 63, 0], [0, 63, 25], [0, 63, 63],
  [0, 30, 63], [0, 0, 63], [25, 0, 63], [45, 0, 63], [63, 0, 63], [63, 10, 30], [63, 63, 63], [63, 45, 30],
];

/** Channel identity colours (chips, orbit radar): kick, snare, hats, toms, perc, cymbals, FX, vox. */
export const GROUP_COLORS: readonly [number, number, number][] = [0, 1, 3, 5, 7, 8, 11, 13].map((i) => LAUNCHPAD_PALETTE[i]);

/** Pad label: sample file name without folder or extension, underscores as spaces. */
export function padLabel(sample: string): string {
  const file = sample.split(/[\\/]/).pop() ?? '';
  return file.replace(/\.[a-z0-9]+$/i, '').replace(/_/g, ' ');
}

/** Screen-reader label of a grid pad: position, sample, mix group, choke group and whether the sample is missing. */
export function padAriaLabel(pad: Pad | null, x: number, y: number, _groups: Group[], missing: boolean): string {
  const where = `Row ${y + 1}, column ${x + 1}`;
  if (!pad?.sample) return `${where}, empty`;
  const choke = pad.chokeGroup ?? 'off';
  return `${where}, ${padLabel(pad.sample).toUpperCase()}, mix ${pad.mixGroup + 1}, choke ${choke}${missing ? ', sample missing' : ''}`;
}

/** Mixer channel legend: "1 KICK", or "GROUP 1" while the channel still has its default name. */
export function groupLegend(index: number, name: string): string {
  const upper = name.trim().toUpperCase();
  return upper === `GROUP ${index + 1}` || !upper ? `GROUP ${index + 1}` : `${index + 1} ${upper}`;
}

export function createEmptySet(): SetModel {
  return {
    version: 1,
    banks: Array.from({ length: 8 }, (_, i) => ({
      name: `Bank ${i + 1}`,
      lights: { flash: true, playState: true, ripple: false },
      humanizeDb: 0,
      pads: [],
    })),
    mixer: {
      groups: Array.from({ length: 8 }, (_, i) => ({
        name: `Group ${i + 1}`, volumeDb: 0, pan: 0, pitchSemitones: 0, lowpassHz: 20000, mute: false, solo: false,
      })),
      masterDb: 0,
    },
  };
}

/** Browser preview only (no Tauri backend): the mockup's demo kit, so the Play screen can be judged by eye. */
export function createPreviewSet(): SetModel {
  const set = createEmptySet();
  const names = ['KICK', 'SNARE', 'HATS', 'TOMS', 'PERC', 'CYMB', 'FX', 'VOX'];
  set.mixer.groups.forEach((g, i) => { g.name = names[i]; });
  const cat = (n: string) =>
    /kick/.test(n) ? 0 : /snare|clap|rim/.test(n) ? 1 : /hat|shaker/.test(n) ? 2 : /tom/.test(n) ? 3
      : /crash|ride|china/.test(n) ? 5 : /vox/.test(n) ? 7 : /riser|impact|zap|sweep/.test(n) ? 6 : 4;
  const rows = [
    ['riser_8bar', 'impact', 'reverse_sweep', 'zap', 'vox_hey', '', '', ''],
    ['crash_1', 'china', '', '', '', '', 'ride_1', 'ride_bell'],
    ['conga_lo', 'conga_hi', 'cowbell', 'clave', 'tamb', 'shaker', '', ''],
    ['tom_low', 'tom_mid', 'tom_high', '', '', 'tom_low', 'tom_mid', 'tom_high'],
    ['hat_closed', 'hat_pedal', 'hat_open', '', '', 'hat_closed', 'hat_pedal', 'hat_open'],
    ['snare_808', 'rim_808', 'clap_808', '', '', 'clap_808', 'rim_808', 'snare_909'],
    ['kick_808', 'kick_sub', '', '', '', '', 'kick_sub', 'kick_909'],
    ['kick_acoustic', '', '', '', '', '', '', 'kick_acoustic'],
  ];
  set.banks[0].name = 'KITS 808';
  set.banks[1].name = 'PERC';
  set.banks[2].name = 'FX';
  rows.forEach((row, ri) => row.forEach((n, x) => {
    if (!n) return;
    const y = 7 - ri, group = cat(n);
    set.banks[0].pads.push({
      ...createDefaultPad(x, y), sample: `samples/${n}.wav`, color: [...GROUP_COLORS[group]] as [number, number, number],
      mixGroup: group, chokeGroup: /hat_/.test(n) ? 1 : null,
    });
  }));
  return set;
}
