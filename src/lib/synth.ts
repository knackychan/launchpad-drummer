// Keys synth helpers (keys synth plan, Task 14). Pure; the metadata comes from synth-meta.ts.
import { SYNTH_META } from './synth-meta.ts';
import { fmtPan } from './mix.ts';
import type { Category, ParamMeta, SourceId, SynthParams } from './synth-types.ts';

export const PARAMS: ReadonlyMap<string, ParamMeta> = new Map(SYNTH_META.params.map((p) => [p.id, p]));

export function meta(id: string): ParamMeta {
  const m = PARAMS.get(id);
  if (!m) throw new Error(`unknown synth control ${id}`);
  return m;
}

/** The object and field a dotted id points at: `env1.attack` → (p.env[0], 'attack'), `fx.reverb.mix` → (p.fx.reverb, 'mix'). */
function slot(p: SynthParams, id: string): [Record<string, unknown>, string] {
  const parts = id.split('.');
  const key = parts.pop()!;
  let obj: unknown = p;
  for (const part of parts) {
    const m = /^(env|lfo)(\d)$/.exec(part);
    const o = obj as Record<string, unknown>;
    obj = m ? (o[m[1]] as unknown[])[Number(m[2]) - 1] : o[part];
  }
  return [obj as Record<string, unknown>, key];
}

/** Like Rust `Kind::clamp` (NaN → the minimum). */
export function clampValue(m: ParamMeta, v: number): number {
  const x = Number.isNaN(v) ? -Infinity : v;
  switch (m.kind) {
    case 'bool': return x >= 0.5 ? 1 : 0;
    case 'choice': return Math.min(m.choices!.length - 1, Math.max(0, Math.round(x)));
    case 'int': return Math.min(m.max!, Math.max(m.min!, Math.round(x)));
    default: return Math.min(m.max!, Math.max(m.min!, x));
  }
}

/** Numeric value (bools 0 / 1, selectors by index), like Rust `ParamId::get`. */
export function getParam(p: SynthParams, id: string): number {
  const m = meta(id);
  const [obj, key] = slot(p, id);
  const v = obj[key];
  if (m.kind === 'bool') return v ? 1 : 0;
  if (m.kind === 'choice') return Math.max(0, m.choices!.indexOf(String(v)));
  return Number(v);
}

/** Writes a value (clamped) into `p` and returns the number stored. */
export function setParam(p: SynthParams, id: string, value: number): number {
  const m = meta(id);
  const v = clampValue(m, value);
  const [obj, key] = slot(p, id);
  obj[key] = m.kind === 'bool' ? v >= 0.5 : m.kind === 'choice' ? m.choices![v] : v;
  return v;
}

/** Value → knob position 0–1 (Rust `Kind::to_norm`). */
export function toNorm(m: ParamMeta, v: number): number {
  let n: number;
  if (m.kind === 'bool') n = v >= 0.5 ? 1 : 0;
  else if (m.kind === 'choice') n = m.choices!.length > 1 ? v / (m.choices!.length - 1) : 0;
  else if (m.kind === 'int') n = (v - m.min!) / Math.max(1, m.max! - m.min!);
  else if (m.taper === 'log') n = Math.log(Math.max(v, m.min!) / m.min!) / Math.log(m.max! / m.min!);
  else if (m.taper === 'cubic') n = Math.cbrt(Math.max(0, v) / m.max!);
  else n = (v - m.min!) / (m.max! - m.min!);
  return Number.isNaN(n) ? 0 : Math.min(1, Math.max(0, n));
}

/** Knob position 0–1 → value (Rust `Kind::from_norm`). */
export function fromNorm(m: ParamMeta, n: number): number {
  const t = Number.isNaN(n) ? 0 : Math.min(1, Math.max(0, n));
  if (m.kind === 'bool') return t >= 0.5 ? 1 : 0;
  if (m.kind === 'choice') return Math.round(t * (m.choices!.length - 1));
  if (m.kind === 'int') return Math.round(m.min! + t * (m.max! - m.min!));
  if (m.taper === 'log') return m.min! * Math.pow(m.max! / m.min!, t);
  if (m.taper === 'cubic') return m.max! * t * t * t;
  return m.min! + t * (m.max! - m.min!);
}

function signed(v: number, unit: string, digits = 0): string {
  const s = Math.abs(v).toFixed(digits);
  if (Number(s) === 0) return `${(0).toFixed(digits)}${unit}`;
  return `${v > 0 ? '+' : '−'}${s}${unit}`;
}

/** A value in its units: `12.0 kHz`, `632 Hz`, `0.71 Hz`, `2.0 ms`, `1.25 s`, `−7 ct`, `+3.6 oct`, `L20`, `90°`. */
export function fmtParam(id: string, v: number): string {
  const m = meta(id);
  if (m.kind === 'bool') return v >= 0.5 ? 'ON' : 'OFF';
  if (m.kind === 'choice') return m.labels![Math.round(v)] ?? '';
  switch (m.unit) {
    case 'pct': return `${Math.round(v * 100)} %`;
    case 'hz': return v >= 1000 ? `${(v / 1000).toFixed(v >= 10_000 ? 1 : 2)} kHz` : `${v.toFixed(v < 10 ? 2 : v < 100 ? 1 : 0)} Hz`;
    case 'sec': return v < 1 ? `${(v * 1000).toFixed(v < 0.01 ? 1 : 0)} ms` : `${v.toFixed(2)} s`;
    case 'cents': return signed(Math.round(v), ' ct');
    case 'semis': return signed(Math.round(v), ' st');
    case 'db': return v <= -60 ? '−∞' : `${signed(v, '', 1)} dB`;
    case 'pan': return fmtPan(v);
    case 'deg': return `${Math.round(v * 360)}°`;
    case 'envOct': return `${signed(v * 8, '', 1)} oct`;
    case 'drive24': return `${(v * 24).toFixed(1)} dB`;
    case 'drive36': return `${(v * 36).toFixed(1)} dB`;
    case 'oct': return signed(Math.round(v), '');
    case 'voices': return `${Math.round(v)}`;
    default: return String(v);
  }
}

/** A route amount in its destination's units: pitch ±24 st, cutoff in octaves (≈ 10 per 100 %), else %. */
export function fmtAmount(dest: string | null, amount: number): string {
  if (dest && dest.endsWith('pitch')) return signed(amount * 24, ' st', 2);
  if (dest === 'filter.cutoff') return signed(amount * Math.log2(1000), ' oct', 1);
  return signed(Math.round(amount * 100), ' %');
}

export const sourceColor = (id: SourceId) => SYNTH_META.sources.find((s) => s.id === id)?.color ?? '#c9d1d9';
export const sourceLabel = (id: SourceId) => SYNTH_META.sources.find((s) => s.id === id)?.label ?? id;
export const destLabel = (id: string) => SYNTH_META.dests.find((d) => d.id === id)?.label ?? id;
export const categoryColor = (c: Category): [number, number, number] =>
  SYNTH_META.categories.find((x) => x.id === c)?.color ?? [63, 40, 0];

/** The destination a control is, if any. */
export const destOf = (id: string): string | undefined => PARAMS.get(id)?.dest;

/** "LCXL row 2 · knob 1" for the 24 mapped controls, else null. */
export function lcxlTag(id: string): string | null {
  for (let row = 0; row < SYNTH_META.lcxl.length; row++) {
    const ch = SYNTH_META.lcxl[row].indexOf(id);
    if (ch >= 0) return `LCXL row ${row + 1} · knob ${ch + 1}`;
  }
  return null;
}

export interface Arc { from: number; to: number; color: string; slot: number }

/** The arcs a knob draws (newest outermost, at most 3) and how many more routes it has. */
export function arcsFor(p: SynthParams, dest: string, base: number): { arcs: Arc[]; more: number } {
  const routes = p.routes.map((r, slot) => ({ r, slot })).filter(({ r }) => r.dest === dest);
  const arcs = routes.slice(-3).reverse().map(({ r, slot }) => ({
    from: base, to: Math.min(1, Math.max(0, base + r.amount)), color: sourceColor(r.source), slot,
  }));
  return { arcs, more: Math.max(0, routes.length - 3) };
}

/** SVG path of an arc on a knob ring (0 = the −135° end, 1 = the +135° end, clockwise). */
export function arcPath(cx: number, cy: number, r: number, n0: number, n1: number): string {
  const a = (n: number) => ((-135 + 270 * n - 90) * Math.PI) / 180;
  const [s, e] = n0 <= n1 ? [n0, n1] : [n1, n0];
  const p = (n: number) => `${(cx + r * Math.cos(a(n))).toFixed(2)} ${(cy + r * Math.sin(a(n))).toFixed(2)}`;
  return `M ${p(s)} A ${r} ${r} 0 ${(e - s) * 270 > 180 ? 1 : 0} 1 ${p(e)}`;
}

/** Leading + trailing throttle; keyed calls retain the latest arguments for every key (K26). */
export function throttle<A extends unknown[]>(fn: (...args: A) => void, ms = 16, key: (...args: A) => unknown = () => 0) {
  let last = -Infinity;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const pending = new Map<unknown, A>();
  function cancel() { clearTimeout(timer); timer = undefined; pending.clear(); }
  function flush() {
    const calls = [...pending.values()];
    cancel();
    if (!calls.length) return;
    last = Date.now();
    for (const args of calls) fn(...args);
  }
  return Object.assign((...args: A) => {
    pending.set(key(...args), args);
    const wait = ms - (Date.now() - last);
    if (wait <= 0) flush();
    else timer ??= setTimeout(flush, wait);
  }, { flush, cancel });
}

/** Selected-route indices after removing `slot` (later slots move up by one). */
export function withoutRoute(selected: Record<string, number>, slot: number): Record<string, number> {
  const next: Record<string, number> = {};
  for (const [dest, s] of Object.entries(selected)) {
    if (s < slot) next[dest] = s;
    else if (s > slot) next[dest] = s - 1;
  }
  return next;
}
