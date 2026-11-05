const LIMIT = 32 * 1024;
const keyOK = (key: unknown): key is string => typeof key === 'string' && /^[a-zA-Z0-9][\w.-]{0,127}$/.test(key);
const prefix = (game: string) => {
  if (!/^[a-z0-9-]{1,64}$/.test(game)) throw new Error('Invalid game id');
  return `lpd.game.${game}.`;
};
const bytes = (store: Record<string, string>) => new TextEncoder().encode(JSON.stringify(store)).length;

export function readGameStore(storage: Storage, game: string): Record<string, string> {
  const start = prefix(game), store: Record<string, string> = Object.create(null);
  for (let i = 0; i < storage.length; i++) {
    const full = storage.key(i);
    if (!full?.startsWith(start)) continue;
    const key = full.slice(start.length), value = storage.getItem(full);
    if (!keyOK(key) || value === null) continue;
    store[key] = value;
    if (bytes(store) > LIMIT) delete store[key];
  }
  return store;
}

export function setGameStore(storage: Storage, game: string, key: unknown, value: unknown): void {
  const start = prefix(game);
  if (!keyOK(key) || (typeof value !== 'string' && value !== null)) throw new Error('Invalid game setting');
  if (value === null) { storage.removeItem(start + key);return; }
  const store = readGameStore(storage, game);
  store[key] = value;
  if (bytes(store) > LIMIT) throw new Error('Game settings exceed 32 KB');
  storage.setItem(start + key, value);
}
