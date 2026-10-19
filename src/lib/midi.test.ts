import assert from 'node:assert/strict';
import test from 'node:test';
import { fixedRole, LOG_LIMIT, pushLog, PREVIEW_MIDI, roleStatus, statusLamp, STATUS_TEXT } from './midi.ts';

test('lamps: green connected, off disconnected, amber busy, red bootloader', () => {
  assert.deepEqual((['connected', 'disconnected', 'busy', 'bootloader'] as const).map(statusLamp), ['ok', '', 'warn', 'error']);
});

test('status texts', () => {
  assert.equal(STATUS_TEXT.busy, 'port busy');
  assert.equal(STATUS_TEXT.bootloader, 'bootloader — run updater');
});

test('only the Launchpad and the LCXL have a fixed role text', () => {
  assert.deepEqual((['launchpad', 'lcxl', 'keys', 'other'] as const).map(fixedRole), ['LAUNCHPAD', 'LCXL', null, null]);
});

test('role status comes from the registry, absent is disconnected', () => {
  assert.equal(roleStatus(PREVIEW_MIDI, 'launchpad'), 'connected');
  assert.equal(roleStatus(PREVIEW_MIDI, 'lcxl'), 'disconnected');
  assert.equal(roleStatus([], 'lcxl'), 'disconnected');
  assert.equal(roleStatus([{ port: 'Launch Control XL', role: 'lcxl', status: 'busy' }], 'lcxl'), 'busy');
});

test('the event log keeps the newest 40 entries first', () => {
  let log: number[] = [];
  for (let i = 0; i < 50; i++) log = pushLog(log, i);
  assert.equal(log.length, LOG_LIMIT);
  assert.equal(log[0], 49);
  assert.equal(log.at(-1), 10);
});
