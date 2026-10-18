import { test } from 'node:test';
import assert from 'node:assert/strict';
import { padForKey } from './keymap.ts';

test('computer keys map to the four lower pad rows', () => {
  ['12345678', 'qwertyui', 'asdfghjk', 'zxcvbnm,'].forEach((row, r) => {
    [...row].forEach((key, x) => assert.deepEqual(padForKey(key), { x, y: 3 - r }));
  });
  assert.deepEqual(padForKey('Q'), padForKey('q'));
  for (const key of ['9', 'p', '.']) assert.equal(padForKey(key), null);
});
