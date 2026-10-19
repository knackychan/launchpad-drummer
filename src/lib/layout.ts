// Play screen size solver (UI spec section 4, mockup layoutPad). Pure: bay size in, sizes out.
// Pads and mixer columns grow together across the width; pads, keybed and sequencer share the height.

export interface MixDims {
  cw: number; // column width
  fh: number; // fader height
  kn: number; // knob size
  kr: number; // knob row height
  kb: number; // key height
  sh: number; // scribble strip height
  fw: number; // fader width
  fs: number; // font size
}

export interface PlayLayout {
  pad: number;
  docked: boolean;
  /** Telemetry column width; -10 = hidden, 30 = folded meter strip. */
  tw: number;
  mini: boolean;
  kh: number; // keybed height
  sq: number; // sequencer plate height
  lh: number; // sequencer lane height
  mix: MixDims;
}

export interface PlayLayoutInput {
  width: number;
  height: number;
  drawerOpen: boolean;
  drawerPinned: boolean;
  teleFolded: boolean;
  /** Keys plate and sequencer folded to a 28 px strip; the height they save goes back to the pads. */
  keysFolded?: boolean;
  seqFolded?: boolean;
  lanes: number;
}

/** A folded keys plate or sequencer. */
export const FOLDED_H = 28;
const GAP = 5;
const DRAWER_W = 300;
const DTAB_W = 28;
export const TELE_MINI = 30;
const MIX_CHROME_W = 76; // mixer width = 76 + 9 columns
const MIX_RATIO = 0.9; // a mixer column wants to be 0.9 × pad width
export const LP_CHROME_H = 16 + 22 + 2 * 6 + 37; // Launchpad plate minus the 8 pad rows

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

export function mixDims(cw: number, fh: number): MixDims {
  const kn = clamp(Math.round(cw * 0.6), 20, 60);
  return {
    cw, fh, kn, kr: kn + 20,
    kb: clamp(Math.round(cw * 0.36), 26, 38),
    sh: clamp(Math.round(cw * 0.3), 24, 32),
    fw: clamp(Math.round(cw * 0.75), 22, 64),
    fs: clamp(cw * 0.19, 8.5, 13),
  };
}

/** Mixer plate height without the fader. */
export const mixChrome = (d: MixDims) => d.sh + 3 * d.kr + 16 + 2 * d.kb + 42 + 24;

export function solvePlayLayout(input: PlayLayoutInput): PlayLayout {
  const { width: W, height: H } = input;
  const avail = H - 16 - LP_CHROME_H - 7 * GAP - 2 * 8;
  const keysOpen = !input.keysFolded;
  const seqOpen = !input.seqFolded;
  const sq0 = seqOpen ? clamp(avail * 0.25, 110, 320) : FOLDED_H;
  const keysMin = 12 + 44;

  const fit = (docked: boolean, tw: number) => {
    const stageW = W - 16 - DTAB_W - 10 - (docked ? DRAWER_W + 10 : 0) - (tw + 10);
    const S = stageW - 10 - (24 + 28 + 66 + 12) - 7 * GAP;
    const pW = (S - MIX_CHROME_W) / (8 + 9 * MIX_RATIO);
    const rest = avail - sq0;
    const pH = keysOpen ? Math.min((rest - keysMin) / 8, (rest - 12) / 8.9) : (rest - FOLDED_H) / 8;
    const p = Math.floor(clamp(Math.min(pW, pH), 36, 112));
    return {
      p,
      cw: clamp(Math.floor((S - 8 * p - MIX_CHROME_W) / 9), 30, 110),
      ok: 8 * p + 9 * 30 + MIX_CHROME_W <= S + 8,
    };
  };

  const docked = input.drawerOpen && input.drawerPinned && fit(true, -10).ok;
  const bare = fit(docked, -10).p;
  let tw = -10;
  if (!input.teleFolded) {
    for (let w = 300; w >= 220; w -= 10) {
      const f = fit(docked, w);
      if (f.ok && f.p >= bare) { tw = w; break; }
    }
    if (tw < 0) {
      const f = fit(docked, 220);
      if (f.ok && f.p >= Math.min(bare, 52)) tw = 220;
    }
  }
  if (tw < 0 && fit(docked, TELE_MINI).ok) tw = TELE_MINI;

  const { p: pad, cw } = fit(docked, tw);
  const kh = keysOpen ? clamp(Math.round(pad * 0.9), 44, 110) : FOLDED_H;
  const sq = seqOpen ? clamp(Math.floor(avail - 8 * pad - (keysOpen ? 12 + kh : FOLDED_H)), 110, 360) : FOLDED_H;
  const lanes = Math.max(1, input.lanes);
  const lh = clamp(Math.floor((sq - 12 - 22 - 6 + 3) / lanes) - 3, 14, 24);

  // The mixer plate is exactly as tall as the Launchpad plate; knobs shrink before the fader drops under 110 px.
  const lpH = LP_CHROME_H + 7 * GAP + 8 * pad;
  const mix = mixDims(cw, 0);
  const spare = lpH - mixChrome(mix) - 110;
  if (spare < 0) {
    mix.kn = Math.max(Math.min(20, mix.kn), mix.kn + Math.floor(spare / 3));
    mix.kr = mix.kn + 20;
  }
  mix.fh = Math.max(110, lpH - mixChrome(mix));

  return { pad, docked, tw, mini: tw > 0 && tw <= TELE_MINI, kh, sq, lh, mix };
}
