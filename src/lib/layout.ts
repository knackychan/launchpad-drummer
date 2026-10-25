// Play screen size solver (UI spec 4, keys synth spec 6.1). Pure: bay size in, sizes out.
// The window height alone sets the Launchpad row, the keybed and the sequencer. Across the row the
// Launchpad, the slim mixer and the synth share the width; the synth keeps at least 480 px.

export interface PlayLayout {
  /** Pad size actually drawn. */
  pad: number;
  /** Pad size the height allows: it sets the row, the keybed and the sequencer. */
  pH: number;
  /** Height of the Launchpad row = height of the mixer and synth plates. */
  rowH: number;
  docked: boolean;
  /** Telemetry column width; -10 = hidden, 30 = folded strip. */
  tw: number;
  mini: boolean;
  kh: number; // keybed height
  sq: number; // sequencer plate height
  lh: number; // sequencer lane height
  mixMode: 'slim' | 'strip';
  mixW: number;
  synthW: number;
}

export interface PlayLayoutInput {
  width: number;
  height: number;
  drawerOpen: boolean;
  drawerPinned: boolean;
  teleFolded: boolean;
  /** Keys plate and sequencer folded to a 28 px strip; the height they save goes to the row. */
  keysFolded?: boolean;
  seqFolded?: boolean;
  lanes: number;
}

export type SynthPageSet = 'wide' | 'narrow' | 'compact';

/** A folded keys plate or sequencer. */
export const FOLDED_H = 28;
const GAP = 5;
const DRAWER_W = 300;
const DTAB_W = 28;
export const TELE_MINI = 30;
export const LP_CHROME_H = 16 + 22 + 2 * 6 + 37; // Launchpad plate minus the 8 pad rows
export const LP_CHROME_W = 24 + 28 + 66 + 12 + 7 * GAP; // Launchpad plate minus the 8 pad columns
const PLATE_GAP = 10;
export const MIX_SLIM_W = 300;
export const MIX_STRIP_W = 40;
export const SYNTH_MIN = 480;
export const SYNTH_ONE_PAGE = 760;
const PAD_KEEP = 56;
const PAD_MIN = 36;
const PAD_MAX = 112;

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

export function solvePlayLayout(input: PlayLayoutInput): PlayLayout {
  const { width: W, height: H } = input;
  const keysOpen = !input.keysFolded;
  const seqOpen = !input.seqFolded;
  const avail = H - 16 - LP_CHROME_H - 7 * GAP - 2 * 8;
  const sq0 = seqOpen ? clamp(avail * 0.25, 110, 320) : FOLDED_H;
  const rest = avail - sq0;
  const raw = keysOpen ? Math.min((rest - 12 - 44) / 8, (rest - 12) / 8.9) : (rest - FOLDED_H) / 8;
  const pH = Math.floor(clamp(raw, PAD_MIN, PAD_MAX));
  const kh = keysOpen ? clamp(Math.round(pH * 0.9), 44, 110) : FOLDED_H;
  const sq = seqOpen ? clamp(Math.floor(avail - 8 * pH - (keysOpen ? 12 + kh : FOLDED_H)), 110, 360) : FOLDED_H;
  const lanes = Math.max(1, input.lanes);
  const lh = clamp(Math.floor((sq - 12 - 22 - 6 + 3) / lanes) - 3, 14, 24);

  const fit = (docked: boolean, tw: number, mix: 'slim' | 'strip') => {
    const stageW = W - 16 - DTAB_W - 10 - (docked ? DRAWER_W + 10 : 0) - (tw + 10);
    const mixW = mix === 'slim' ? MIX_SLIM_W : MIX_STRIP_W;
    const room = stageW - LP_CHROME_W - 2 * PLATE_GAP - mixW;
    // Rule 3: pads give way until the synth has one page, but not below 56 px. Rule 2: the synth keeps 480 px.
    let p = Math.min(pH, Math.max(Math.floor((room - SYNTH_ONE_PAGE) / 8), Math.min(PAD_KEEP, pH)));
    p = Math.min(p, Math.floor((room - SYNTH_MIN) / 8));
    return { p, room, mixW, ok: p >= PAD_MIN };
  };
  // Telemetry: the widest of 300…230 px that costs the pads and the synth nothing, else 220.
  const pickTele = (dock: boolean) => {
    if (input.teleFolded) return TELE_MINI;
    const bare = fit(dock, -10, 'slim');
    for (let w = 300; w > 220; w -= 10) {
      const f = fit(dock, w, 'slim');
      if (f.ok && f.p >= bare.p && f.room - 8 * f.p >= Math.min(bare.room - 8 * bare.p, SYNTH_ONE_PAGE)) return w;
    }
    return 220;
  };
  // Rule 2's order: pads (inside fit), then a docked drawer floats, the telemetry folds, the mixer collapses.
  let docked = input.drawerOpen && input.drawerPinned;
  let mix: 'slim' | 'strip' = 'slim';
  let tw = pickTele(docked);
  if (!fit(docked, tw, mix).ok && docked) {
    docked = false;
    tw = pickTele(false);
  }
  if (!fit(docked, tw, mix).ok) tw = TELE_MINI;
  if (!fit(docked, tw, mix).ok) mix = 'strip';
  const f = fit(docked, tw, mix);
  const pad = Math.max(PAD_MIN, f.p);
  return {
    pad, pH, rowH: LP_CHROME_H + 7 * GAP + 8 * pH, docked, tw, mini: tw > 0 && tw <= TELE_MINI,
    kh, sq, lh, mixMode: mix, mixW: f.mixW, synthW: f.room - 8 * pad,
  };
}

/** The synth plate's page set (spec 6.3, K8). */
export function synthPageSet(width: number, height: number): SynthPageSet {
  if (height < 480) return 'compact';
  return width >= SYNTH_ONE_PAGE && height >= 600 ? 'wide' : 'narrow';
}

export const SYNTH_PAGES: Record<SynthPageSet, readonly string[]> = {
  wide: ['MAIN', 'FX', 'MATRIX'],
  narrow: ['OSC', 'MOD', 'FX', 'MATRIX'],
  compact: ['OSC', 'FILTER', 'ENV', 'LFO', 'FX', 'MATRIX'],
};

/** Knob size on the synth plate: 17 % of a row of sections, 24–44 px. */
export function synthKnob(height: number, set: SynthPageSet): number {
  const rows = set === 'wide' ? 3 : set === 'narrow' ? 2 : 1;
  return clamp(Math.round(((height - 64) / rows) * 0.17), 24, 44);
}
