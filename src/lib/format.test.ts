import { test } from 'node:test';
import assert from 'node:assert/strict';
import { middleTruncate } from './format.ts';

test('paths truncate in the middle and preserve the last segment when it fits', () => {
  assert.equal(middleTruncate('C:\\kit', 20), 'C:\\kit');
  const result = middleTruncate('C:\\very-long-parent\\another-parent\\set.json', 20);
  assert.ok(result.length <= 20 && result.includes('…') && result.endsWith('set.json'));
  assert.equal(middleTruncate('abcdefghijklmnop', 9), 'abcd…mnop');
  assert.equal(middleTruncate('long', 0), '');
});
