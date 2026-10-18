// Per-channel activity from pad hits (0..1, ~110 ms half-life). Feeds the top-strip chips, the mixer
// level fills and the telemetry orbit radar. Real audio levels need engine meter events (not streamed yet).
//
// The decay loop only runs while some channel is active: at rest nothing is scheduled, nothing
// touches the DOM and no listener is called. A hit wakes it again.
import { dbToPos } from './mix.ts';

export const energy = new Float32Array(8);

/** Below this a channel counts as silent and the loop stops. */
export const REST = 0.002;
const HALF_LIFE_MS = 110;

type Listener = () => void;
const listeners = new Set<Listener>();
let running = false;
let raf = 0;
let last = 0;

/** Called after each decay step, and once more when everything has come to rest. */
export function onActivity(listener: Listener): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

export function isResting(): boolean {
  return energy.every((e) => e === 0);
}

/** One decay step of `elapsedMs`. Pure; exported for tests. */
export function decay(elapsedMs: number): void {
  const k = Math.pow(0.5, Math.min(100, elapsedMs) / HALF_LIFE_MS);
  for (let i = 0; i < 8; i++) {
    energy[i] *= k;
    if (energy[i] < REST) energy[i] = 0;
  }
}

function paint(): void {
  document.querySelectorAll<HTMLElement>('[data-chip]').forEach((el) => {
    el.style.opacity = (0.16 + 0.84 * energy[Number(el.dataset.chip)]).toFixed(2);
  });
  document.querySelectorAll<HTMLElement>('.f-fill[data-group]').forEach((el) => {
    el.style.setProperty('--m', (energy[Number(el.dataset.group)] * 0.75).toFixed(3));
  });
  listeners.forEach((listener) => listener());
}

function frame(now: number): void {
  raf = 0;
  decay(now - last);
  last = now;
  paint();
  if (running && !isResting()) raf = requestAnimationFrame(frame);
}

function wake(): void {
  if (running && !raf && typeof requestAnimationFrame === 'function') {
    last = performance.now();
    raf = requestAnimationFrame(frame);
  }
}

export function hitGroup(group: number, amount: number): void {
  if (group >= 0 && group < 8) {
    energy[group] = Math.max(energy[group], Math.min(1, Math.max(0, amount)));
    wake();
  }
}

/** Enables the decay loop (it still sleeps while nothing is active); it also drives `[data-chip]`
 *  opacity and `.f-fill[data-group]` heights. */
export function runActivity(): () => void {
  running = true;
  paint();
  wake();
  return () => {
    running = false;
    cancelAnimationFrame(raf);
    raf = 0;
  };
}

/** Activity of a pad hit: its channel fader position, nothing when the channel is silenced. */
export function hitAmount(volumeDb: number, audible: boolean): number {
  return audible ? Math.min(1, dbToPos(volumeDb) / 0.75) : 0;
}
