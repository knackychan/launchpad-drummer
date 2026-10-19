// What SETUP › MIDI DEVICES and the status strip show for the device registry. Pure, so it is testable.
import type { MidiDevice, MidiRole, MidiStatus } from './model.ts';

export const LOG_LIMIT = 40;
export const NO_DEVICES = 'No MIDI devices found. Plug one in; the list refreshes every 2 s.';

/** Registry demo for the browser preview. */
export const PREVIEW_MIDI: MidiDevice[] = [
  { port: 'Launchpad MK2', role: 'launchpad', status: 'connected' },
  { port: 'Launch Control XL', role: 'lcxl', status: 'disconnected' },
  { port: 'Keystation 49 MK3', role: 'keys', status: 'connected' },
];

export const STATUS_TEXT: Record<MidiStatus, string> = {
  connected: 'connected', disconnected: 'disconnected', busy: 'port busy', bootloader: 'bootloader — run updater',
};

/** Lamp class of a status: green connected, off disconnected, amber busy, red bootloader. */
export function statusLamp(status: MidiStatus): 'ok' | 'warn' | 'error' | '' {
  return ({ connected: 'ok', busy: 'warn', bootloader: 'error', disconnected: '' } as const)[status] ?? '';
}

/** Fixed text of the ROLE column, or null when the player chooses (KEYS | OTHER). */
export function fixedRole(role: MidiRole): string | null {
  return role === 'launchpad' ? 'LAUNCHPAD' : role === 'lcxl' ? 'LCXL' : null;
}

/** State of the first device with a role; no row at all means disconnected. */
export function roleStatus(devices: readonly MidiDevice[], role: MidiRole): MidiStatus {
  return devices.find(d => d.role === role)?.status ?? 'disconnected';
}

/** Newest first, at most LOG_LIMIT entries. */
export function pushLog<T>(log: readonly T[], entry: T): T[] {
  return [entry, ...log].slice(0, LOG_LIMIT);
}
