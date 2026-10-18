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
