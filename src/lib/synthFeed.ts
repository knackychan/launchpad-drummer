// The synth's live feed (`synth-telemetry`, keys synth spec 2.6): latest frame, smoothed spectrum, listeners.
import { specLevel } from './telemetry.ts';
import type { SynthTelemetry } from './synth-types.ts';

const BANDS = 128;
const FALL = 0.88;
type Listener = () => void;
const listeners = new Set<Listener>();

export const synthFeed = { frame: null as SynthTelemetry | null, at: -Infinity, spectrum: new Float32Array(BANDS) };

/** True while frames arrive (the last one under 500 ms old). */
export const synthLive = (now = performance.now()) => synthFeed.frame !== null && now - synthFeed.at < 500;

export function onSynthFeed(listener: Listener): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

/** A `synth-telemetry` event arrived: bars rise at once and fall 12 % per frame. */
export function pushSynthFrame(frame: SynthTelemetry, now = performance.now()): void {
  synthFeed.frame = frame;
  synthFeed.at = now;
  for (let i = 0; i < BANDS; i++) synthFeed.spectrum[i] = Math.max(specLevel(frame.spectrum[i] ?? -100), synthFeed.spectrum[i] * FALL);
  listeners.forEach((l) => l());
}
