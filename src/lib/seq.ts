// Pure helpers of the step sequencer plate (plan 1C Task 14).
import type { Pattern } from './model.ts';

export type StepValue = 0 | 1 | 2;

export const BPM = { min: 60, max: 200, def: 100 };
export const SWING = { min: 50, max: 75, def: 50 };
export const MAX_STEPS = 32;

/** Click: 0 → 1, 1 or 2 → 0. Accent (right-click or Shift+click): 2 → 0, otherwise 2. */
export function clickValue(current: StepValue, accent: boolean): StepValue {
  if (accent) return current === 2 ? 0 : 2;
  return current === 0 ? 1 : 0;
}

/** A copy of `steps` with `steps[index] = value`. */
export function paint(steps: StepValue[], index: number, value: StepValue): StepValue[] {
  const next = steps.slice();
  next[index] = value;
  return next;
}

/** "5 lanes · 16 steps · 100 BPM" ("1 lane"). */
export function foldSummary(lanes: number, length: number, bpm: number): string {
  return `${lanes} ${lanes === 1 ? 'lane' : 'lanes'} · ${length} steps · ${bpm} BPM`;
}

/** "1·KICK_808": bank number and the sample's file name; "EMPTY" when the pad has no sample. */
export function laneLabel(bank: number, sample: string | null): string {
  if (!sample) return 'EMPTY';
  const file = (sample.split(/[\\/]/).pop() ?? '').replace(/\.[a-z0-9]+$/i, '');
  return `${bank + 1}·${file.toUpperCase()}`;
}

/**
 * Same rule as Rust `seq::set_length`: growing fills a lane's new steps with repeats of the old
 * length only when that lane's steps from the old length on are all 0; shrinking only changes
 * `length`. Returns a new pattern.
 */
export function setLength(p: Pattern, n: 4 | 8 | 16 | 32): Pattern {
  const old = p.length;
  const lanes = p.lanes.map((lane) => {
    const steps = lane.steps.slice();
    while (steps.length < MAX_STEPS) steps.push(0);
    if (n > old && old > 0 && steps.slice(old).every((v) => v === 0)) {
      for (let j = old; j < n; j++) steps[j] = steps[j % old];
    }
    return { ...lane, steps };
  });
  return { ...p, length: n, lanes };
}

/** Angle (radians, 0 = 3 o'clock, clockwise) of the playhead: the top is step 0. */
export function stepAngle(step: number, progress: number, length: number): number {
  return -Math.PI / 2 + ((step + progress) / length) * Math.PI * 2;
}
