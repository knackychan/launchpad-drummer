// What SETUP › AUDIO shows, as pure functions so it is testable.
import type { AudioDevice, AudioLatency, AudioStatus } from './model.ts';

export function latencyText(l: AudioLatency): string {
  return `MIDI→BUFFER ${l.midiToBufferMs.toFixed(1)} · OUTPUT ${l.outputMs.toFixed(1)} · TOTAL ${l.totalMs.toFixed(1)} ms`;
}

export interface StreamView { lamp: 'ok' | 'warn' | 'error'; text: string; canReconnect: boolean }

/** STREAM row: lamp, state text and whether RECONNECT is live. */
export function streamView(status: AudioStatus | null, dropoutRecent: boolean): StreamView {
  switch (status?.state) {
    case 'running': return { lamp: dropoutRecent ? 'warn' : 'ok', text: 'RUNNING', canReconnect: false };
    case 'lost': return { lamp: 'error', text: 'DEVICE LOST', canReconnect: true };
    case 'error': return { lamp: 'error', text: 'ERROR', canReconnect: true };
    default: return { lamp: 'error', text: 'STOPPED', canReconnect: true };
  }
}

/** DEVICE select: the default device is marked; no saved device (or one that is gone) shows the default. */
export function deviceOptions(devices: AudioDevice[], saved: string | null): { options: { value: string; label: string }[]; selected: string } {
  const options = devices.map(d => ({ value: d.name, label: d.isDefault ? `${d.name} (default)` : d.name }));
  const found = devices.find(d => d.name === saved) ?? devices.find(d => d.isDefault) ?? devices[0];
  return { options, selected: found?.name ?? '' };
}
