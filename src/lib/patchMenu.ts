// The patch menu (keys synth spec 8.4) as plain menu items; App supplies the actions. Pure.
import type { MenuItem } from '../components/ContextMenu.svelte';
import { SYNTH_META } from './synth-meta.ts';
import type { KeysSettings, PatchRef, PatchSource } from './synth-types.ts';

export type PatchRequest = { factory: string } | { user: string } | { init: true };
export interface PatchActions {
  load(request: PatchRequest): void; save(): void; saveAs(): void; rename(): void; remove(): void;
  revert(): void; showFolder(): void; openFactory(): void; openUser(): void;
}

const isUser = (s: PatchSource): s is { user: string } => typeof s === 'object' && 'user' in s;
const isFactory = (s: PatchSource): s is { factory: string } => typeof s === 'object' && 'factory' in s;

/** The patch LCD text: the name, plus ` *` while modified. */
export const patchLabel = (k: KeysSettings) => (k.modified ? `${k.patch.name} *` : k.patch.name);

export function patchMenuItems(k: KeysSettings, userCount: number, a: PatchActions): MenuItem[] {
  return [
    { label: 'FACTORY ▸', action: a.openFactory },
    { label: 'USER ▸', action: a.openUser, disabled: userCount === 0 },
    { label: '', sep: true },
    { label: 'SAVE', action: a.save },
    { label: 'SAVE AS…', action: a.saveAs },
    { label: 'RENAME…', action: a.rename, disabled: !isUser(k.patchSource) },
    { label: 'DELETE', action: a.remove, disabled: !isUser(k.patchSource) },
    { label: '', sep: true },
    { label: 'INIT', action: () => a.load({ init: true }) },
    { label: 'REVERT', action: a.revert, disabled: !k.modified },
    { label: '', sep: true },
    { label: 'SHOW FOLDER', action: a.showFolder },
  ];
}

/** Factory patches grouped under their category (spec 8.4 FACTORY ▸). */
export function factoryItems(list: PatchRef[], current: PatchSource, load: (r: PatchRequest) => void): MenuItem[] {
  const items: MenuItem[] = [];
  for (const cat of SYNTH_META.categories) {
    const inCat = list.filter((p) => p.category === cat.id);
    if (!inCat.length) continue;
    items.push({ label: cat.label, info: true });
    for (const p of inCat) items.push({ label: p.name, checked: isFactory(current) && current.factory === p.id, action: () => load({ factory: p.id }) });
  }
  return items;
}

const byName = (a: PatchRef, b: PatchRef) => a.name.localeCompare(b.name);

/** User patches, alphabetical (spec 8.4 USER ▸). */
export function userItems(list: PatchRef[], current: PatchSource, load: (r: PatchRequest) => void): MenuItem[] {
  return [...list].sort(byName).map((p) => ({ label: p.name, checked: isUser(current) && current.user === p.id, action: () => load({ user: p.id }) }));
}

/** ◀ ▶: the factory list then the user list, wrapping; from INIT (or a source not in the lists) the first or last. */
export function neighbour(lists: { factory: PatchRef[]; user: PatchRef[] }, current: PatchSource, step: 1 | -1): PatchRequest | null {
  const all: PatchRequest[] = [...lists.factory.map((p) => ({ factory: p.id })), ...[...lists.user].sort(byName).map((p) => ({ user: p.id }))];
  if (!all.length) return null;
  const i = all.findIndex((r) => ('factory' in r && isFactory(current) && current.factory === r.factory)
    || ('user' in r && isUser(current) && current.user === r.user));
  if (i < 0) return all[step > 0 ? 0 : all.length - 1];
  return all[(i + step + all.length) % all.length];
}
