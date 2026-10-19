// Labels and lookups for MIDI learn (plan 1C Task 12). Pure.
import { groupLegend, padLabel } from './model.ts';
import type { Binding, LearnTargetModel, SetModel } from './model.ts';

const GROUP_PARAMS: Record<string, string> = { volume: 'VOLUME', pan: 'PAN', pitch: 'PITCH', lowpass: 'LPF', mute: 'MUTE', solo: 'SOLO' };
const BANK_PARAMS: Record<string, string> = { humanize: 'HUMANIZE', flash: 'FLASH', playState: 'PLAY', ripple: 'RIPPLE' };
const FIXED: Record<string, string> = {
  master: 'MASTER', stopAll: 'STOP ALL', 'keys.volume': 'KEYS · VOLUME', 'keys.mod': 'KEYS · MOD',
  'seq.play': 'SEQ · PLAY', 'seq.bpm': 'SEQ · BPM', 'seq.swing': 'SEQ · SWING',
};

/** "3 TOMS · VOLUME", "MASTER", "BANK 2 · HUMANIZE", "KEYS · MOD", "STOP ALL". */
export function paramLabel(id: string, set: SetModel): string {
  if (FIXED[id]) return FIXED[id];
  const [kind, n, what] = id.split('.');
  const index = Number(n);
  if (kind === 'group' && GROUP_PARAMS[what] && set.mixer.groups[index]) {
    return `${groupLegend(index, set.mixer.groups[index].name)} · ${GROUP_PARAMS[what]}`;
  }
  if (kind === 'bank' && BANK_PARAMS[what]) return `BANK ${index + 1} · ${BANK_PARAMS[what]}`;
  return id;
}

/** Pads: "B2 R3·C4 · KICK 808" ("EMPTY" for a pad without a sample). */
export function targetLabel(target: LearnTargetModel, set: SetModel): string {
  if ('param' in target) return paramLabel(target.param, set);
  const { bank, x, y } = target.pad;
  const pad = set.banks[bank]?.pads.find(p => p.x === x && p.y === y);
  return `B${bank + 1} R${y + 1}·C${x + 1} · ${pad?.sample ? padLabel(pad.sample).toUpperCase() : 'EMPTY'}`;
}

/** "Keystation 49 · CC 21 → 4 TOMS · VOLUME". */
export function bindingLabel(b: Binding, set: SetModel): string {
  return `${b.device} · ${b.kind === 'note' ? 'NOTE' : 'CC'} ${b.number} → ${targetLabel(b.target, set)}`;
}

/** The control ids that carry a binding: parameter ids and `pad:{bank}:{x}:{y}`. */
export function boundIds(set: SetModel): Set<string> {
  return new Set((set.learn ?? []).map(b => 'param' in b.target ? b.target.param : `pad:${b.target.pad.bank}:${b.target.pad.x}:${b.target.pad.y}`));
}

/** The pad a note is bound to, on any device (the keys plate shows a dot on such keys). */
export function noteBoundTo(set: SetModel, note: number): { bank: number; x: number; y: number } | null {
  for (const b of set.learn ?? []) if (b.kind === 'note' && b.number === note && 'pad' in b.target) return { ...b.target.pad };
  return null;
}
