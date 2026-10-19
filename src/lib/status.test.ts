import assert from 'node:assert/strict';
import test from 'node:test';
import type { AudioStatus } from './model.ts';
import { audioLabel, currentNotice, dropoutRecent, IDLE_SAVE, padEditKey, saveBadge } from './status.ts';
import type { NoticeInputs } from './status.ts';

const running: AudioStatus = {
  state: 'running', backend: 'WASAPI', deviceName: 'Speakers', sampleRate: 48000, bufferSize: 256, channels: 2,
  dropouts: 0, lastDropoutAgoMs: null, asioFallback: false, latency: { midiToBufferMs: 5.3, outputMs: 10, totalMs: 15.3 },
};
const base: NoticeInputs = { tauri: true, deviceState: 'connected', save: IDLE_SAVE, audio: running, toast: null };

test('a failed save is a visible error with its reason, and outranks device status', () => {
  const n = currentNotice({ ...base, save: { state: 'failed', reason: 'disk is full' } });
  assert.equal(n.cls, 'error');
  assert.match(n.text, /disk is full/);
  assert.deepEqual(saveBadge({ state: 'failed', reason: 'x' }), { cls: 'error', text: 'SAVE FAILED' });
});

test('dirty and saved states have badges; idle has none', () => {
  assert.equal(saveBadge({ state: 'pending', reason: null })?.text, 'UNSAVED');
  assert.equal(saveBadge({ state: 'saved', reason: null })?.text, 'SAVED');
  assert.equal(saveBadge(IDLE_SAVE), null);
});

test('priority: action error, then save failure, then audio error, then Launchpad', () => {
  const failed = { state: 'failed', reason: 'r' } as const;
  const dead = { ...running, state: 'error' } as const;
  assert.match(currentNotice({ ...base, toast: 'Set exists', save: failed, audio: dead }).text, /Set exists/);
  assert.match(currentNotice({ ...base, save: failed, audio: dead }).text, /Not saved/);
  assert.match(currentNotice({ ...base, audio: dead }).text, /Audio output stopped/);
  assert.equal(currentNotice(base).text, 'Launchpad MK2 connected');
  assert.equal(currentNotice({ ...base, deviceState: 'busy' }).cls, 'warn');
  assert.match(currentNotice({ ...base, tauri: false, deviceState: 'disconnected' }).text, /Browser preview/);
});

test('audio label reports backend, buffer and total latency, or the stopped and lost states', () => {
  assert.equal(audioLabel(running), 'WASAPI · 256 · 15.3 ms');
  assert.equal(audioLabel({ ...running, bufferSize: null }), 'WASAPI · AUTO · 15.3 ms');
  assert.equal(audioLabel({ ...running, backend: 'ASIO', bufferSize: 64, latency: { midiToBufferMs: 1.3, outputMs: 4, totalMs: 5.3 } }), 'ASIO · 64 · 5.3 ms');
  assert.equal(audioLabel({ ...running, latency: { midiToBufferMs: 5.3, outputMs: 0, totalMs: 5.3 } }), 'WASAPI · 256 · 5.3 ms');
  assert.equal(audioLabel({ ...running, latency: { midiToBufferMs: 5.3, outputMs: 4.7, totalMs: 10 } }), 'WASAPI · 256 · 10.0 ms');
  assert.equal(audioLabel({ ...running, state: 'lost' }), 'AUDIO LOST');
  assert.equal(audioLabel({ ...running, state: 'stopped' }), 'AUDIO STOPPED');
  assert.equal(audioLabel({ ...running, state: 'error' }), 'AUDIO ERROR');
  assert.equal(audioLabel(null), 'AUDIO STOPPED');
});

test('a dropout is recent for 10 s, counted from when it happened', () => {
  assert.equal(dropoutRecent(null, 0), false);
  assert.equal(dropoutRecent(undefined, 0), false);
  assert.equal(dropoutRecent(0, 0), true);
  assert.equal(dropoutRecent(2000, 7999), true);
  assert.equal(dropoutRecent(2000, 8000), false);
  assert.equal(dropoutRecent(10_000, 0), false);
});

test('only continuous pad controls get an undo-coalescing key', () => {
  assert.equal(padEditKey(0, 2, 3, 'volumeDb'), 'pad:0:2:3:volumeDb');
  assert.equal(padEditKey(5, 0, 7, 'trimEndMs'), 'pad:5:0:7:trimEndMs');
  for (const discrete of ['mode', 'chokeGroup', 'mixGroup', 'color', 'sample', undefined]) {
    assert.equal(padEditKey(0, 0, 0, discrete), null);
  }
});
