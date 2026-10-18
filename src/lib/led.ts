export type Rgb = readonly [number, number, number];
export type LedUpdate = { note: number; rgb: Rgb };
export type PadInput = { note: number; pressed: boolean; source: 'screen' | 'hardware' };

export function padNote(row: number, col: number): number {
  if (!Number.isInteger(row) || !Number.isInteger(col) || row < 1 || row > 8 || col < 1 || col > 8) {
    throw new RangeError('Pad row and column must be integers from 1 to 8');
  }
  return row * 10 + col;
}

// Frame order is visual: top-left to bottom-right. Hardware rows start at the bottom.
export function ledIndex(note: number): number {
  const row = Math.floor(note / 10);
  const col = note % 10;
  return Number.isInteger(note) && row >= 1 && row <= 8 && col >= 1 && col <= 8
    ? (8 - row) * 8 + col - 1 : -1;
}

export function createLedFrame(): Rgb[] {
  return Array.from({ length: 64 }, () => [0, 0, 0] as const);
}

export function applyLedUpdates(frame: readonly Rgb[], updates: readonly LedUpdate[]): Rgb[] {
  const next = [...frame];
  for (const { note, rgb } of updates) {
    const index = ledIndex(note);
    if (index < 0 || rgb.length !== 3 || rgb.some(v => !Number.isInteger(v) || v < 0 || v > 63)) {
      throw new RangeError('LED update requires a grid note and three integer RGB channels from 0 to 63');
    }
    next[index] = [...rgb];
  }
  return next;
}

export function ledToCss(rgb: Rgb): string {
  return `rgb(${rgb.map(v => Math.round(255 * (v / 63) ** 0.45)).join(' ')})`;
}
