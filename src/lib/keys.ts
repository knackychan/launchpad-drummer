// Pure helpers of the keys plate (plan 1C Task 16).
import type { Group } from './model.ts';

export const SOUNDS = ['piano', 'ePiano', 'organ', 'synthPad', 'synthLead', 'synthBass'] as const;
export type KeysSound = typeof SOUNDS[number];
export const SOUND_LABELS: Record<KeysSound, string> = {
  piano: 'PIANO', ePiano: 'E.PIANO', organ: 'ORGAN', synthPad: 'SYNTH PAD', synthLead: 'SYNTH LEAD', synthBass: 'SYNTH BASS',
};
export const SOUND_COLORS: Record<KeysSound, [number, number, number]> = {
  piano: [63, 63, 63], ePiano: [63, 40, 0], organ: [63, 20, 0], synthPad: [25, 0, 63], synthLead: [0, 63, 25], synthBass: [0, 30, 63],
};

/** The keys settings in the set file (Rust `KeysSettings`). */
export interface KeysSettings { sound: KeysSound; volumeDb: number; output: number | null }
export const DEFAULT_KEYS: KeysSettings = { sound: 'ePiano', volumeDb: -6, output: null };

export const KEY_LO = 21; // A0
export const KEY_HI = 108; // C8
const NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'];

/** 60 → "C4", 61 → "C#4", 21 → "A0", 108 → "C8". */
export function noteName(n: number): string {
  return `${NAMES[((n % 12) + 12) % 12]}${Math.floor(n / 12) - 1}`;
}

export const isBlackKey = (n: number) => [1, 3, 6, 8, 10].includes(((n % 12) + 12) % 12);

/** `y` from the top of the key: the top plays softly (30), the bottom hardest (127). */
export function velocityFromY(y: number, height: number): number {
  if (height <= 0) return 127;
  return Math.min(127, Math.max(30, Math.round(30 + (97 * y) / height)));
}

const clampOctave = (octave: number) => Math.min(2, Math.max(-1, octave));

/** The 49 keys a Keystation reaches at this octave setting. */
export function windowRange(octave: number): [number, number] {
  const base = 36 + 12 * clampOctave(octave);
  return [base, base + 48];
}

/** 0 → "C2–C6". */
export function rangeLabel(octave: number): string {
  const [lo, hi] = windowRange(octave);
  return `${noteName(lo)}–${noteName(hi)}`;
}

/** null → "→ MASTER"; a channel → "→ 8 VOX" (its number and name, at most 8 characters). */
export function outputLabel(output: number | null, groups: Group[]): string {
  if (output === null || !groups[output]) return '→ MASTER';
  return `→ ${output + 1} ${groups[output].name.toUpperCase().slice(0, 8)}`;
}
