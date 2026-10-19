// Telemetry feed (UI spec 4.8): the `telemetry` event from the engine taps, plus the pure maths the
// instruments use (spectrum smoothing, peak holds, meter scales, readouts). The Telemetry panel and
// the mixer fills read `feed`; nothing here touches the DOM.
import { setLevels, setMaster } from './activity.ts';

export interface TelemetryFrame {
  /** 1024 samples, -1..1, starting at a rising zero crossing. */
  scope: number[];
  /** 28 log-spaced bands, 40 Hz..16 kHz, dBFS (floor -100). */
  bands: number[];
  peakHz: number;
  /** dBFS, left and right. */
  master: [number, number];
  /** Linear peaks 0..1, one per mixer channel. */
  groups: number[];
}

export const BANDS = 28;
export const SPEC_FLOOR_DB = -72;
export const VU_MIN_DB = -48;
export const VU_MAX_DB = 3;
export const SILENCE_DB = -90;
export const SILENCE_MS = 500;
const BAR_FALL = 0.88;
const CAP_HOLD_MS = 400;
const CAP_FALL = 0.01;
const PEAK_HOLD_MS = 1200;
const PEAK_FALL_DB_PER_S = 30;

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

/** -72..0 dBFS to 0..1 of the spectrum height. */
export const specLevel = (db: number) => clamp01((db - SPEC_FLOOR_DB) / -SPEC_FLOOR_DB);
/** -48..+3 dB to 0..1 along a master meter. */
export const vuPos = (db: number) => clamp01((db - VU_MIN_DB) / (VU_MAX_DB - VU_MIN_DB));
/** Green below -9 dB, amber up to 0, red above. */
export const meterColor = (db: number) => (db > 0 ? '#ff3d4f' : db > -9 ? '#ffb21a' : '#5cff9d');

export interface Spectrum {
  /** Smoothed bar heights, 0..1. */
  bands: number[];
  caps: number[];
  /** When each cap last took a new value (ms). */
  capAt: number[];
}

export const createSpectrum = (): Spectrum => ({
  bands: Array(BANDS).fill(0), caps: Array(BANDS).fill(0), capAt: Array(BANDS).fill(0),
});

/** A cap holds for 400 ms, then falls 1 % of the height per frame, never below its bar. */
export function capStep(cap: number, bar: number, heldMs: number): number {
  if (bar >= cap) return bar;
  return heldMs > CAP_HOLD_MS ? Math.max(bar, cap - CAP_FALL) : cap;
}

/** One frame: bars rise at once and fall 12 % per frame; caps follow `capStep`. */
export function stepBands(s: Spectrum, dbs: readonly number[], now: number): void {
  for (let b = 0; b < BANDS; b++) {
    const bar = Math.max(specLevel(dbs[b] ?? -100), s.bands[b] * BAR_FALL);
    s.bands[b] = bar;
    if (bar >= s.caps[b]) s.capAt[b] = now;
    s.caps[b] = capStep(s.caps[b], bar, now - s.capAt[b]);
  }
}

export interface Hold { v: number; t: number }

/** The held peak: steady for 1.2 s, then falling 30 dB per second. */
export function holdValue(h: Hold, now: number): number {
  const over = now - h.t - PEAK_HOLD_MS;
  return over <= 0 ? h.v : Math.max(-100, h.v - (PEAK_FALL_DB_PER_S * over) / 1000);
}

export function holdUpdate(h: Hold, db: number, now: number): void {
  if (db >= holdValue(h, now)) { h.v = db; h.t = now; }
}

/** Master below -90 dBFS for 500 ms. `since` is the state kept between frames. */
export function silenceStep(s: { since: number | null }, masterDb: number, now: number): boolean {
  if (masterDb >= SILENCE_DB) { s.since = null; return false; }
  s.since ??= now;
  return now - s.since >= SILENCE_MS;
}

/** `94 Hz`, `1.2 kHz`; `—` when there is no peak. */
export function fmtPeakHz(hz: number): string {
  if (!(hz > 0)) return '—';
  return hz >= 1000 ? `${(hz / 1000).toFixed(1)} kHz` : `${Math.round(hz)} Hz`;
}

/** `PK −∞`, `PK −6.0 dB`, `PK +1.3 dB` (true minus). */
export function fmtPk(db: number): string {
  if (db < -60) return 'PK −∞';
  const sign = db > 0.04 ? '+' : db < -0.04 ? '−' : '';
  return `PK ${sign}${Math.abs(db).toFixed(1)} dB`;
}

/** 1024 scope samples across 8 divisions. */
export function msPerDiv(sampleRate: number): string {
  return sampleRate > 0 ? `${(((1024 / sampleRate) * 1000) / 8).toFixed(1)} ms/div` : '— ms/div';
}

/** When the app's UI started (this module loads with it); the telemetry clock counts from here. */
export const APP_START = performance.now();

/** `T+mm:ss.t` for a time in ms; works in whole tenths so it never shows `:60.0`. */
export function fmtClock(ms: number): string {
  const tenths = Math.max(0, Math.floor(ms / 100));
  const mm = String(Math.floor(tenths / 600)).padStart(2, '0');
  const ss = String(Math.floor(tenths / 10) % 60).padStart(2, '0');
  return `T+${mm}:${ss}.${tenths % 10}`;
}

// ---- shared live state -------------------------------------------------------------------

type FeedListener = () => void;
const listeners = new Set<FeedListener>();

export const feed = {
  frame: null as TelemetryFrame | null,
  spectrum: createSpectrum(),
  holds: [{ v: -100, t: 0 }, { v: -100, t: 0 }] as Hold[],
  silence: { since: null as number | null },
  silent: true,
};

/** Called after every frame (about 60 times a second while the feed is on). */
export function onFeed(listener: FeedListener): () => void {
  listeners.add(listener);
  return () => { listeners.delete(listener); };
}

/** A `telemetry` event arrived. */
export function pushFrame(frame: TelemetryFrame, now = performance.now()): void {
  feed.frame = frame;
  feed.silent = silenceStep(feed.silence, Math.max(frame.master[0], frame.master[1]), now);
  stepBands(feed.spectrum, frame.bands, now);
  holdUpdate(feed.holds[0], frame.master[0], now);
  holdUpdate(feed.holds[1], frame.master[1], now);
  setLevels(frame.groups);
  setMaster(frame.master);
  listeners.forEach((listener) => listener());
}
