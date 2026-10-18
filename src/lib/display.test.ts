import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CRT_FACTORS, readDisplay, saveDisplay } from './display.ts';

test('CRT starts at LOW and only accepts OFF, LOW, MEDIUM', () => {
  assert.equal(readDisplay(null), 'low');
  assert.equal(readDisplay({ getItem: () => 'medium' }), 'medium');
  assert.equal(readDisplay({ getItem: () => 'unknown' }), 'low');
  assert.equal(readDisplay({ getItem: () => { throw new Error('denied'); } }), 'low');
  assert.deepEqual(CRT_FACTORS, { off: 0, low: 1, medium: 1.35 });
});

test('CRT is an app preference and storage failures leave the control usable', () => {
  const values = new Map<string, string>();
  const storage = { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => { values.set(key, value); } };
  saveDisplay(storage, 'off');
  assert.equal(readDisplay(storage), 'off');
  assert.doesNotThrow(() => saveDisplay({ setItem: () => { throw new Error('full'); } }, 'low'));
});
