export type Level = 'error' | 'warn' | 'info' | 'ok';
export interface NoticeAction { label: string; id: 'reconnect' | 'dismiss' | 'howto' | 'show' | 'setup' }
export interface Notice { id: string; prio: number; level: Level; text: string; action?: NoticeAction }
export interface Confirmation { id: string; text: string; level?: Level }
export interface NoticeInputs {
  learnArmed: null | 'cc' | 'note'; audioLost: boolean; backup: null | { savedAt: string | null };
  saveFailed: string | null; launchpad: 'connected' | 'disconnected' | 'busy' | 'bootloader';
  lcxl: 'connected' | 'disconnected' | 'busy'; missing: number; asioFallback: boolean;
  dropoutRecent: boolean; confirmations: Confirmation[]; keysAbsent: boolean; dismissed: ReadonlySet<string>;
}
export const ICONS: Record<Level, string> = { error: '✖', warn: '⚠', info: '•', ok: '✓' };
export function activeNotices(i: NoticeInputs): Notice[] {
  const result: Notice[] = [];
  const add = (when: unknown, id: string, prio: number, level: Level, text: string, action?: NoticeAction) => {
    if (when && !i.dismissed.has(id)) result.push({ id, prio, level, text, ...(action ? { action } : {}) });
  };
  add(i.learnArmed, 'learn', 1, 'warn', i.learnArmed === 'cc' ? 'Move a control on any MIDI device · Esc cancels' : 'Hit a key or pad on any MIDI device · Esc cancels');
  add(i.audioLost, 'audio-lost', 2, 'error', 'Audio device lost', { label: 'RECONNECT', id: 'reconnect' });
  add(i.backup, 'backup', 3, 'warn', `set.json unreadable — loaded backup (saved ${i.backup?.savedAt ?? 'earlier'})`, { label: 'DISMISS', id: 'dismiss' });
  add(i.saveFailed, 'save-failed', 4, 'error', `Couldn't save set — ${i.saveFailed}. Retrying`);
  add(i.launchpad === 'bootloader', 'lp-bootloader', 5, 'error', "Launchpad in bootloader — run Novation's MK2 updater, then replug", { label: 'HOW TO FIX', id: 'howto' });
  add(i.launchpad === 'busy', 'lp-busy', 6, 'warn', 'Launchpad port busy — close Components, DAWs or another copy of this app');
  add(i.missing > 0, 'missing', 7, 'warn', `${i.missing} sample${i.missing === 1 ? '' : 's'} missing`, { label: 'SHOW', id: 'show' });
  add(i.asioFallback, 'asio', 8, 'warn', 'ASIO not available — using WASAPI', { label: 'DISMISS', id: 'dismiss' });
  add(i.dropoutRecent, 'dropout', 9, 'warn', 'Audio dropout — raise buffer size', { label: 'SETUP', id: 'setup' });
  add(i.lcxl === 'busy', 'lcxl-busy', 10, 'warn', 'Launch Control XL port busy — close other MIDI apps');
  for (const c of i.confirmations) add(true, `ok:${c.id}`, 11, c.level ?? 'ok', c.text);
  add(i.launchpad === 'disconnected', 'lp-off', 12, 'info', 'Launchpad not connected — checking every 2 s');
  add(i.lcxl === 'disconnected', 'lcxl-off', 13, 'info', 'Launch Control XL not connected — checking every 2 s');
  add(i.keysAbsent, 'keys-off', 14, 'info', 'Keystation 49 not connected — on-screen keys still play');
  return result;
}
