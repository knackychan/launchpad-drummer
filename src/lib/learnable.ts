// Right-click / Shift+F10 on a learnable control opens "Learn MIDI CC" (and "Reset" when it has a default).
import type { MenuItem } from '../components/ContextMenu.svelte';

export interface LearnCtx {
  /** The id being learned, or null. */
  armed(): string | null;
  isBound(id: string): boolean;
  arm(id: string): void;
  openMenu(x: number, y: number, items: MenuItem[]): void;
}

export const LEARN = Symbol('learn');

export interface LearnableOptions { ctx?: LearnCtx; id?: string; reset?: () => void; extra?: () => MenuItem[] }

export const isLearning = (ctx: LearnCtx | undefined, id: string | undefined) => !!ctx && !!id && ctx.armed() === id;
export const isLearned = (ctx: LearnCtx | undefined, id: string | undefined) => !!ctx && !!id && ctx.isBound(id);

/** Svelte action: opens the learn menu for `id`. Without `ctx` or `id` it does nothing. */
export function learnable(node: HTMLElement, options: LearnableOptions) {
  let opts = options;
  const items = (): MenuItem[] => {
    const extra = opts.extra?.() ?? [];
    return [
    ...extra,
    ...(extra.length ? [{ label: '', sep: true }] : []),
    { label: 'Learn MIDI CC', action: () => opts.ctx?.arm(opts.id!) },
    ...(opts.reset ? [{ label: 'Reset', action: opts.reset }] : []),
    ];
  };
  const open = (x: number, y: number) => opts.ctx?.openMenu(x, y, items());
  const onContext = (e: MouseEvent) => {
    if (!opts.ctx || !opts.id) return;
    e.preventDefault();
    e.stopPropagation();
    open(e.clientX, e.clientY);
  };
  const onKey = (e: KeyboardEvent) => {
    if (!opts.ctx || !opts.id) return;
    if (e.key !== 'ContextMenu' && !(e.shiftKey && e.key === 'F10')) return;
    e.preventDefault();
    e.stopPropagation();
    const r = node.getBoundingClientRect();
    open(r.left + r.width / 2, r.bottom);
  };
  node.addEventListener('contextmenu', onContext);
  node.addEventListener('keydown', onKey);
  return {
    update(next: LearnableOptions) { opts = next; },
    destroy() { node.removeEventListener('contextmenu', onContext); node.removeEventListener('keydown', onKey); },
  };
}
