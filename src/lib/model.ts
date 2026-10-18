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

export interface AudioStatus {
  state: string;
  deviceName: string;
  sampleRate: number;
  bufferSize: number | null;
  channels: number;
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

export const LAUNCHPAD_PALETTE: readonly [number, number, number][] = [
  [63, 0, 0], [63, 25, 0], [63, 50, 0], [63, 63, 0],
  [40, 63, 0], [0, 63, 0], [0, 63, 30], [0, 63, 63],
  [0, 35, 63], [0, 0, 63], [30, 0, 63], [63, 0, 63],
  [63, 0, 30], [63, 63, 63], [35, 35, 35], [15, 15, 15],
];
