export interface TreeNode { name: string; path: string; isDir: boolean; depth: number; open?: boolean; children?: TreeNode[] }
export function flatten(nodes: TreeNode[]): TreeNode[] {
  return nodes.flatMap(n => [n, ...(n.isDir && n.open ? flatten(n.children ?? []) : [])]);
}
export type TreeAction = 'open' | 'close' | 'toggle' | 'audition' | 'load' | 'parent';
export function keyNav(rows: TreeNode[], index: number, key: string): { index: number; action?: TreeAction } {
  const node = rows[index];
  if (!node) return { index: 0 };
  if (key === 'ArrowDown') return { index: Math.min(rows.length - 1, index + 1) };
  if (key === 'ArrowUp') return { index: Math.max(0, index - 1) };
  if (key === 'ArrowRight' && node.isDir) {
    if (!node.open) return { index, action: 'open' };
    if (rows[index + 1]?.depth > node.depth) return { index: index + 1 };
  }
  if (key === 'ArrowLeft') {
    if (node.open && node.isDir) return { index, action: 'close' };
    for (let i = index - 1; i >= 0; i--) if (rows[i].depth < node.depth) return { index: i, action: 'parent' };
  }
  if (key === ' ') return { index, action: node.isDir ? 'toggle' : 'audition' };
  if (key === 'Enter') return { index, action: node.isDir ? 'toggle' : 'load' };
  return { index };
}

/** Row height of the sample browser (matches `.ti` in SampleBrowser.svelte) and rows rendered beyond the viewport. */
export const ROW_H = 22;
export const OVERSCAN = 10;
/** Used until the real height of the list is known. */
const GUESS_VIEW_H = 600;

/** Rows `[start, end)` to render for a list of `count` rows scrolled to `scrollTop` (`pad`: space above the first row). */
export function visibleRange(count: number, scrollTop: number, viewH: number, rowH = ROW_H, overscan = OVERSCAN, pad = 0): { start: number; end: number } {
  if (count <= 0) return { start: 0, end: 0 };
  const h = viewH > 0 ? viewH : GUESS_VIEW_H;
  const first = Math.min(count - 1, Math.max(0, Math.floor((scrollTop - pad) / rowH)));
  const last = Math.min(count, Math.max(first + 1, Math.ceil((scrollTop + h - pad) / rowH)));
  return { start: Math.max(0, first - overscan), end: Math.min(count, last + overscan) };
}

/** The scroll position that brings row `index` into view with the least movement (unchanged when already visible). */
export function revealScrollTop(index: number, scrollTop: number, viewH: number, rowH = ROW_H, pad = 0): number {
  const h = viewH > 0 ? viewH : GUESS_VIEW_H;
  const top = pad + index * rowH, bottom = top + rowH;
  if (top < scrollTop) return Math.max(0, top - pad);
  if (bottom > scrollTop + h) return bottom - h;
  return scrollTop;
}

/** A selection index that still points at a row after the list shrank (a folder collapsed). */
export const clampIndex = (index: number, count: number) => Math.max(0, Math.min(index, count - 1));
