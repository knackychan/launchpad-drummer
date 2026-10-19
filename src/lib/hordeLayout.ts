import type { Layout } from './horde.ts';

export const W = 640, H = 360;
export type Rect = { x: number; y: number; w: number; h: number };

// Keyboard arena: physical keys (event.code), so AZERTY and QWERTZ work by position. Row 4 is the number row.
export const KEY_ROWS = [
  { row: 4, offset: 0, codes: ['Digit1', 'Digit2', 'Digit3', 'Digit4', 'Digit5', 'Digit6', 'Digit7', 'Digit8', 'Digit9', 'Digit0'], labels: '1234567890' },
  { row: 3, offset: .5, codes: ['KeyQ', 'KeyW', 'KeyE', 'KeyR', 'KeyT', 'KeyY', 'KeyU', 'KeyI', 'KeyO', 'KeyP'], labels: 'QWERTYUIOP' },
  { row: 2, offset: .75, codes: ['KeyA', 'KeyS', 'KeyD', 'KeyF', 'KeyG', 'KeyH', 'KeyJ', 'KeyK', 'KeyL', 'Semicolon'], labels: 'ASDFGHJKL;' },
  { row: 1, offset: 1.25, codes: ['KeyZ', 'KeyX', 'KeyC', 'KeyV', 'KeyB', 'KeyN', 'KeyM', 'Comma', 'Period', 'Slash'], labels: 'ZXCVBNM,./' },
] as const;

export const CODE_TO_CELL: Readonly<Record<string, { column: number; row: number }>> = Object.fromEntries(
  KEY_ROWS.flatMap(line => line.codes.map((code, index) => [code, { column: index + 1, row: line.row }])));

export type ArenaLayout = {
  kind: Layout; cols: number; rows: number; S: number;
  rect: (column: number, row: number) => Rect;
  arena: Rect; sea: Rect; base: Rect; turret: { x: number; y: number };
  side: ((row: number) => Rect) | null;
  label: (column: number, row: number) => string;
};

// Game logic knows columns 1..cols and rows 1..rows; row rows+1 is the sea gate above the arena.
export function makeLayout(kind: Layout): ArenaLayout {
  if (kind === 'pad') {
    const S = 36, x0 = 158, y0 = 0;
    const rect = (c: number, r: number): Rect => ({ x: x0 + (c - 1) * S, y: y0 + (9 - Math.min(r, 9)) * S, w: S, h: S });
    return { kind, cols: 8, rows: 8, S, rect, arena: { x: x0, y: y0 + S, w: 8 * S, h: 8 * S }, sea: { x: 0, y: 0, w: W, h: y0 + S },
      base: { x: x0 - 8, y: y0 + 9 * S, w: 8 * S + 16, h: H - (y0 + 9 * S) }, turret: { x: x0 + 4 * S, y: y0 + 9 * S + 16 },
      side: r => ({ x: x0 + 8 * S + 3, y: y0 + (9 - r) * S, w: S - 2, h: S }),
      label: (c, r) => 'ABCDEFGH'[c - 1] + r };
  }
  const S = 50, x0 = 40, y0 = 60;
  const rect = (c: number, r: number): Rect => {
    if (r > 4) return { x: x0 + (c - 1) * S, y: y0 - S, w: S, h: S };
    const line = KEY_ROWS[4 - r];
    return { x: x0 + (line.offset + c - 1) * S, y: y0 + (4 - r) * S, w: S, h: S };
  };
  return { kind, cols: 10, rows: 4, S, rect, arena: { x: x0, y: y0, w: 11.25 * S, h: 4 * S }, sea: { x: 0, y: 0, w: W, h: y0 },
    base: { x: x0 + 20, y: y0 + 4 * S + 8, w: 11.25 * S - 40, h: 14 }, turret: { x: x0 + 5.6 * S, y: y0 + 4 * S + 24 },
    side: null, label: (c, r) => KEY_ROWS[4 - r].labels[c - 1] };
}

// Canvas point to game cell, or null when the point is outside every key.
export function cellAt(layout: ArenaLayout, x: number, y: number): { column: number; row: number } | null {
  for (let row = 1; row <= layout.rows; row++) for (let column = 1; column <= layout.cols; column++) {
    const rect = layout.rect(column, row);
    if (x >= rect.x && x < rect.x + rect.w && y >= rect.y && y < rect.y + rect.h) return { column, row };
  }
  return null;
}
