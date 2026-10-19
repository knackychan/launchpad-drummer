export const CRT_FACTORS = { off: 0, low: 1, medium: 1.35 } as const;
export type CrtLevel = keyof typeof CRT_FACTORS;
export type Bend = 'flat' | 'bent';
export interface DisplayPrefs { level: CrtLevel; bend: Bend }

const LEVEL_KEY = 'launchpad-drummer.display.crt';
const BEND_KEY = 'launchpad-drummer.display.bend';
const DEFAULTS: DisplayPrefs = { level: 'low', bend: 'flat' };

/** The CRT level is the old key (a bare string, still valid); the bend is a new key. */
export function readDisplay(storage: Pick<Storage, 'getItem'> | null): DisplayPrefs {
  const prefs = { ...DEFAULTS };
  try {
    const level = storage?.getItem(LEVEL_KEY);
    if (level === 'off' || level === 'low' || level === 'medium') prefs.level = level;
    const bend = storage?.getItem(BEND_KEY);
    if (bend === 'flat' || bend === 'bent') prefs.bend = bend;
  } catch { /* App preferences must also work when storage is unavailable. */ }
  return prefs;
}

export function saveDisplay(storage: Pick<Storage, 'setItem'> | null, prefs: DisplayPrefs): void {
  try {
    storage?.setItem(LEVEL_KEY, prefs.level);
    storage?.setItem(BEND_KEY, prefs.bend);
  } catch { /* Keep the in-memory preference. */ }
}
