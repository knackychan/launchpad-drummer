import assert from 'node:assert/strict';
import test from 'node:test';
import type { AudioStatus } from './model.ts';
import { deviceOptions, latencyText, streamView } from './audio.ts';

const running: AudioStatus = {
  state: 'running', backend: 'WASAPI', deviceName: 'Speakers', sampleRate: 48000, bufferSize: 256, channels: 2,
  dropouts: 0, lastDropoutAgoMs: null, asioFallback: false, latency: { midiToBufferMs: 5.3, outputMs: 10, totalMs: 15.3 },
};

test('latency readout shows one decimal for each part', () => {
  assert.equal(latencyText(running.latency), 'MIDI→BUFFER 5.3 · OUTPUT 10.0 · TOTAL 15.3 ms');
  assert.equal(latencyText({ midiToBufferMs: 0, outputMs: 0, totalMs: 0 }), 'MIDI→BUFFER 0.0 · OUTPUT 0.0 · TOTAL 0.0 ms');
});

test('stream lamp: green running, amber after a recent dropout, red when stopped, lost or failed', () => {
  assert.deepEqual(streamView(running, false), { lamp: 'ok', text: 'RUNNING', canReconnect: false });
  assert.deepEqual(streamView(running, true), { lamp: 'warn', text: 'RUNNING', canReconnect: false });
  assert.deepEqual(streamView({ ...running, state: 'lost' }, false), { lamp: 'error', text: 'DEVICE LOST', canReconnect: true });
  assert.deepEqual(streamView({ ...running, state: 'stopped' }, true), { lamp: 'error', text: 'STOPPED', canReconnect: true });
  assert.deepEqual(streamView({ ...running, state: 'error' }, false), { lamp: 'error', text: 'ERROR', canReconnect: true });
  assert.deepEqual(streamView(null, false), { lamp: 'error', text: 'STOPPED', canReconnect: true });
});

test('device options mark the default and select the saved or default device', () => {
  const devices = [
    { name: 'Speakers', isDefault: true, bufferSizes: [64, 128] },
    { name: 'Headphones', isDefault: false, bufferSizes: [] },
  ];
  assert.deepEqual(deviceOptions(devices, null), {
    options: [{ value: 'Speakers', label: 'Speakers (default)' }, { value: 'Headphones', label: 'Headphones' }],
    selected: 'Speakers',
  });
  assert.equal(deviceOptions(devices, 'Headphones').selected, 'Headphones');
  // A saved device that is gone: the stream falls back to the default, so that is what shows.
  assert.equal(deviceOptions(devices, 'USB DAC').selected, 'Speakers');
  assert.deepEqual(deviceOptions([], null), { options: [], selected: '' });
});
