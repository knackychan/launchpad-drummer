import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CRT_FACTORS, readDisplay, saveDisplay, type DisplayPrefs } from './display.ts';

const DEFAULTS: DisplayPrefs = { level: 'low', bend: 'flat' };

function memory(initial: Record<string, string> = {}) {
  const values = new Map(Object.entries(initial));
  return {
    values,
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => { values.set(key, value); },
  };
}

test('the display starts at LOW and FLAT and only accepts the known values', () => {
  assert.deepEqual(readDisplay(null), DEFAULTS);
  assert.deepEqual(CRT_FACTORS, { off: 0, low: 1, medium: 1.35 });
  assert.deepEqual(readDisplay({ getItem: () => 'unknown' }), DEFAULTS, 'garbage in both keys');
});

test('the old string format of the CRT key stays valid', () => {
  const storage = memory({ 'launchpad-drummer.display.crt': 'medium' });
  assert.deepEqual(readDisplay(storage), { level: 'medium', bend: 'flat' });
  assert.deepEqual(readDisplay(memory({ 'launchpad-drummer.display.crt': 'off' })), { level: 'off', bend: 'flat' });
});

test('level and bend round-trip through their own keys', () => {
  const storage = memory();
  saveDisplay(storage, { level: 'low', bend: 'bent' });
  assert.equal(storage.values.get('launchpad-drummer.display.crt'), 'low');
  assert.equal(storage.values.get('launchpad-drummer.display.bend'), 'bent');
  assert.deepEqual(readDisplay(storage), { level: 'low', bend: 'bent' });
  saveDisplay(storage, { level: 'off', bend: 'flat' });
  assert.deepEqual(readDisplay(storage), { level: 'off', bend: 'flat' });
});

test('storage that throws gives the defaults and never throws on save', () => {
  const broken = { getItem: () => { throw new Error('denied'); }, setItem: () => { throw new Error('full'); } };
  assert.deepEqual(readDisplay(broken), DEFAULTS);
  assert.doesNotThrow(() => saveDisplay(broken, { level: 'low', bend: 'bent' }));
  assert.deepEqual(readDisplay(memory({ 'launchpad-drummer.display.bend': 'sideways' })), DEFAULTS);
});
