// Fader law shared by the mixer and the telemetry: 0 dB sits at 75 % of the travel,
// +6 dB at the top, a 40·log10 curve below 0 dB, −60 dB and under = −∞ at the bottom.
// The backend treats any value at or below DB_FLOOR as exact silence (`lpd_core::audio::FADER_FLOOR_DB`):
// keep the two equal.
export const DB_FLOOR = -60;
export const DB_CEIL = 6;

export function dbToPos(db: number): number {
  if (!(db > DB_FLOOR)) return 0;
  if (db >= 0) return Math.min(1, 0.75 + (db / DB_CEIL) * 0.25);
  return 0.75 * Math.pow(10, db / 40);
}

export function posToDb(pos: number): number {
  if (pos >= 0.75) return Math.min(DB_CEIL, ((pos - 0.75) / 0.25) * DB_CEIL);
  if (pos <= 0) return DB_FLOOR;
  return Math.max(DB_FLOOR, 40 * Math.log10(pos / 0.75));
}

/** `−∞`, `0.0`, `+3.5`, `−12.0` (true minus sign, no unit). */
export function fmtDbShort(db: number): string {
  if (db <= DB_FLOOR) return '−∞';
  const sign = db > 0.04 ? '+' : db < -0.04 ? '−' : '';
  return `${sign}${Math.abs(db).toFixed(1)}`;
}

export function fmtDb(db: number): string {
  return db <= DB_FLOOR ? '−∞ dB' : `${fmtDbShort(db)} dB`;
}

export function fmtSemis(st: number): string {
  const sign = st > 0 ? '+' : st < 0 ? '−' : '';
  return `${sign}${Math.abs(st).toFixed(Number.isInteger(st) ? 0 : 1)} st`;
}

export function fmtPan(pan: number): string {
  if (Math.abs(pan) < 0.01) return 'C';
  return pan < 0 ? `L${Math.round(-pan * 50)}` : `R${Math.round(pan * 50)}`;
}

// Low-pass knob: 0..1 maps to 200 Hz · 100^t (200 Hz … 20 kHz); the top is OFF (bypass).
export const lpToKnob = (hz: number) => Math.min(1, Math.max(0, Math.log(hz / 200) / Math.log(100)));
export const knobToLp = (t: number) => 200 * Math.pow(100, t);
export function fmtLp(t: number): string {
  const hz = knobToLp(t);
  if (hz >= 19900) return 'OFF';
  return hz >= 1000 ? `${(hz / 1000).toFixed(1)} kHz` : `${Math.round(hz)} Hz`;
}
