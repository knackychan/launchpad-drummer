import { test } from 'node:test';
import assert from 'node:assert/strict';
import { applyLedUpdates, createLedFrame, ledIndex, ledToCss, padNote } from './led.ts';

test('grid notes run from bottom-left 11 to top-right 88, with no side buttons', () => {
  assert.equal(padNote(1, 1), 11);
  assert.equal(padNote(8, 8), 88);
  assert.equal(ledIndex(81), 0);
  assert.equal(ledIndex(11), 56);
  assert.equal(ledIndex(89), -1);
  assert.equal(ledIndex(90), -1);
  assert.equal(ledIndex(11.5), -1);
});

test('LED frames start off and update by hardware note without mutating the old frame', () => {
  const frame = createLedFrame();
  assert.equal(frame.length, 64);
  const next = applyLedUpdates(frame, [{ note: 11, rgb: [63, 20, 0] }]);
  assert.deepEqual(next[56], [63, 20, 0]);
  assert.deepEqual(frame[56], [0, 0, 0]);
  assert.deepEqual(applyLedUpdates(next, [{ note: 11, rgb: [0, 0, 0] }])[56], [0, 0, 0]);
});

test('LED updates reject invalid notes and RGB values at the event boundary', () => {
  for (const update of [
    { note: 19, rgb: [0, 0, 0] },
    { note: 11, rgb: [64, 0, 0] },
    { note: 11, rgb: [-1, 0, 0] },
    { note: 11, rgb: [1.5, 0, 0] },
    { note: 11, rgb: [NaN, 0, 0] },
  ]) assert.throws(() => applyLedUpdates(createLedFrame(), [update as never]), RangeError);
});

test('screen channels use the locked 0.45 exponent, including black and white', () => {
  assert.equal(ledToCss([0, 0, 0]), 'rgb(0 0 0)');
  assert.equal(ledToCss([63, 63, 63]), 'rgb(255 255 255)');
  assert.equal(ledToCss([1, 20, 63]), `rgb(${Math.round(255 * (1 / 63) ** 0.45)} ${Math.round(255 * (20 / 63) ** 0.45)} 255)`);
});
