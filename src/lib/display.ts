export const CRT_FACTORS = { off: 0, low: 1, medium: 1.35 } as const;
export type CrtLevel = keyof typeof CRT_FACTORS;
const KEY = 'launchpad-drummer.display.crt';

export function readDisplay(storage: Pick<Storage, 'getItem'> | null): CrtLevel {
  try {
    const value = storage?.getItem(KEY);
    if (value === 'off' || value === 'low' || value === 'medium') return value;
  } catch { /* App preferences must also work when storage is unavailable. */ }
  return 'low';
}

export function saveDisplay(storage: Pick<Storage, 'setItem'> | null, level: CrtLevel): void {
  try { storage?.setItem(KEY, level); } catch { /* Keep the in-memory preference. */ }
}
