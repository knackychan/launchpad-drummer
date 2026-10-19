// Per-channel activity (0..1, ~110 ms half-life). Feeds the top-strip chips, the mixer level fills
// and the telemetry orbit radar. Source: the engine's level feed (`setLevels`, `setMaster`) while
// frames arrive; pad hits (`hitGroup`) in the browser preview and whenever the feed is silent for a second.
//
// The decay loop only runs while some channel is active: at rest nothing is scheduled, nothing
// touches the DOM and no listener is called. A hit or a level wakes it again.
import { dbToPos } from './mix.ts';

export const energy = new Float32Array(8);

/** Below this a channel counts as silent and the loop stops. */
export const REST = 0.002;
const HALF_LIFE_MS = 110;

/** Master meter level (dBFS, left and right) falling 36 dB per second between frames. */
export const MASTER_FLOOR_DB = -100;
const MASTER_REST_DB = -60;
const MASTER_FALL_DB_PER_S = 36;
export const masterDb = new Float32Array([MASTER_FLOOR_DB, MASTER_FLOOR_DB]);

/** The feed counts as live for this long after its last frame. */
export const FEED_TIMEOUT_MS = 1000;
let feedAt = -Infinity;

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
  return energy.every((e) => e === 0) && masterDb.every((d) => d === MASTER_FLOOR_DB);
}

/** True while engine level frames are arriving; pad hits then do not feed the activity. */
export function feedLive(now = performance.now()): boolean {
  return now - feedAt < FEED_TIMEOUT_MS;
}

/** One decay step of `elapsedMs`. Pure; exported for tests. */
export function decay(elapsedMs: number): void {
  const ms = Math.min(100, elapsedMs);
  const k = Math.pow(0.5, ms / HALF_LIFE_MS);
  for (let i = 0; i < 8; i++) {
    energy[i] *= k;
    if (energy[i] < REST) energy[i] = 0;
  }
  for (let c = 0; c < 2; c++) {
    if (masterDb[c] === MASTER_FLOOR_DB) continue;
    masterDb[c] -= (MASTER_FALL_DB_PER_S * ms) / 1000;
    if (masterDb[c] < MASTER_REST_DB) masterDb[c] = MASTER_FLOOR_DB;
  }
}

/** dBFS of a linear peak. */
const toDb = (x: number) => (x > 0 ? 20 * Math.log10(x) : -100);

function paint(): void {
  const live = feedLive();
  document.querySelectorAll<HTMLElement>('[data-chip]').forEach((el) => {
    el.style.opacity = (0.16 + 0.84 * Math.min(1, energy[Number(el.dataset.chip)])).toFixed(2);
  });
  document.querySelectorAll<HTMLElement>('.f-fill[data-group]').forEach((el) => {
    const e = energy[Number(el.dataset.group)];
    // Live: the channel's level on the fader's own scale. Otherwise the hit-based stand-in.
    el.style.setProperty('--m', (live ? dbToPos(toDb(e)) : e * 0.75).toFixed(3));
  });
  document.querySelectorAll<HTMLElement>('.f-fill[data-ch]').forEach((el) => {
    el.style.setProperty('--m', dbToPos(masterDb[el.dataset.ch === 'R' ? 1 : 0]).toFixed(3));
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

/** Per-channel peaks (linear) from a telemetry frame: `energy[i] = max(energy[i], groups[i])`. */
export function setLevels(groups: number[], now = performance.now()): void {
  feedAt = now;
  for (let i = 0; i < 8; i++) {
    const v = groups[i];
    if (v > energy[i]) energy[i] = v;
  }
  wake();
}

/** Master peaks (dBFS) from a telemetry frame; a quieter reading than the falling meter is ignored. */
export function setMaster(db: readonly [number, number]): void {
  for (let c = 0; c < 2; c++) {
    if (db[c] > MASTER_REST_DB && db[c] > masterDb[c]) masterDb[c] = db[c];
  }
  wake();
}

/** Enables the decay loop (it still sleeps while nothing is active); it also drives `[data-chip]`
 *  opacity and `.f-fill` heights. */
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
