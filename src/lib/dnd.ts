export interface Rect { left: number; top: number; width: number; height: number }
export type DropTarget = { kind: 'pad'; x: number; y: number } | { kind: 'bank'; bank: number } | { kind: 'editor' } | null;
export function hitTest(px: number, py: number, pads: {x:number;y:number;rect:Rect}[], banks: {bank:number;rect:Rect}[], editor: Rect | null): DropTarget {
  const inside = (r: Rect) => px >= r.left && py >= r.top && px < r.left + r.width && py < r.top + r.height;
  const pad = pads.find(p => inside(p.rect)); if (pad) return { kind: 'pad', x: pad.x, y: pad.y };
  const bank = banks.find(b => inside(b.rect)); if (bank) return { kind: 'bank', bank: bank.bank };
  return editor && inside(editor) ? { kind: 'editor' } : null;
}
export type DropTag = 'IMPORT' | `IMPORT ×${number}` | 'REPLACE' | 'MOVE' | 'SWAP' | 'COPY';
export function dropTag(source: 'files' | 'pad', targetFilled: boolean, ctrl: boolean, count: number): DropTag {
  return source === 'files' ? count > 1 ? `IMPORT ×${count}` : targetFilled ? 'REPLACE' : 'IMPORT' : ctrl ? 'COPY' : targetFilled ? 'SWAP' : 'MOVE';
}
export const DRAG_THRESHOLD = 4;
export const BANK_HOVER_MS = 400;
export function physicalToCss(p: { x: number; y: number }, dpr: number): { x: number; y: number } { return { x: p.x / dpr, y: p.y / dpr }; }
export function isAudioName(name: string): boolean { return /\.(wav|aif|aiff|flac|mp3|ogg)$/i.test(name); }
export function passedThreshold(dx: number, dy: number): boolean { return Math.hypot(dx, dy) >= DRAG_THRESHOLD; }
