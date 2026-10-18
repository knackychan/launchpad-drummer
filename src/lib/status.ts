// What the shell tells the player about saving, audio and the Launchpad. Pure, so it is testable.
import type { AudioStatus } from './model.ts';

export type SaveState = 'idle' | 'pending' | 'saved' | 'failed';
export interface SaveStatus { state: SaveState; reason: string | null }
export type Level = 'ok' | 'warn' | 'error' | 'info';
export interface Notice { cls: Level; text: string }

export const IDLE_SAVE: SaveStatus = { state: 'idle', reason: null };

/** Status-strip badge next to SAVE; null when there is nothing to say. */
export function saveBadge(save: SaveStatus): Notice | null {
  switch (save.state) {
    case 'pending': return { cls: 'warn', text: 'UNSAVED' };
    case 'saved': return { cls: 'ok', text: 'SAVED' };
    case 'failed': return { cls: 'error', text: 'SAVE FAILED' };
    default: return null;
  }
}

export function audioLabel(audio: AudioStatus | null): string {
  if (audio?.state === 'running') {
    return `${audio.deviceName} · ${audio.bufferSize ?? 'auto'} · ${(audio.sampleRate / 1000).toFixed(1)} kHz`;
  }
  return audio?.state === 'error' ? 'AUDIO ERROR' : 'AUDIO STOPPED';
}

export interface NoticeInputs {
  tauri: boolean;
  deviceState: string;
  save: SaveStatus;
  audio: AudioStatus | null;
  /** A short-lived message about something the player just did (a failed open, a rejected sample). */
  toast: string | null;
}

/** One line for the top strip: the most urgent thing wins. */
export function currentNotice(i: NoticeInputs): Notice {
  if (i.toast) return { cls: 'error', text: i.toast };
  if (i.save.state === 'failed') {
    return { cls: 'error', text: `Not saved — ${i.save.reason ?? 'unknown error'}` };
  }
  if (i.audio?.state === 'error') return { cls: 'error', text: 'Audio output stopped — check the audio device' };
  switch (i.deviceState) {
    case 'connected': return { cls: 'ok', text: 'Launchpad MK2 connected' };
    case 'busy': return { cls: 'warn', text: 'Launchpad port busy — close other MIDI apps' };
    case 'bootloader': return { cls: 'error', text: 'Launchpad bootloader — run updater' };
    default:
      return { cls: 'info', text: i.tauri ? 'Launchpad not connected — on-screen pads available' : 'Browser preview — demo kit, no audio engine' };
  }
}

/** Key that makes a drag on one control a single undo step; null for discrete changes. */
const CONTINUOUS = new Set(['volumeDb', 'pitchSemitones', 'trimStartMs', 'trimEndMs', 'colorChannel']);
export function padEditKey(bank: number, x: number, y: number, field: string | undefined): string | null {
  return field && CONTINUOUS.has(field) ? `pad:${bank}:${x}:${y}:${field}` : null;
}
