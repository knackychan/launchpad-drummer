import test from 'node:test';
import assert from 'node:assert/strict';
import { createDefaultPad, LAUNCHPAD_PALETTE } from './model.ts';

test('createDefaultPad initializes pad with coordinates and valid defaults', () => {
  const pad = createDefaultPad(3, 4);
  assert.equal(pad.x, 3);
  assert.equal(pad.y, 4);
  assert.equal(pad.mixGroup, 4); // mix group defaults to row y
  assert.equal(pad.mode, 'oneShot');
  assert.equal(pad.volumeDb, 0);
  assert.equal(pad.pitchSemitones, 0);
  assert.equal(pad.chokeGroup, null);
  assert.deepEqual(pad.color, [63, 20, 10]);
});

test('LAUNCHPAD_PALETTE contains 16 valid 0..63 RGB color presets', () => {
  assert.equal(LAUNCHPAD_PALETTE.length, 16);
  for (const [r, g, b] of LAUNCHPAD_PALETTE) {
    assert.ok(r >= 0 && r <= 63);
    assert.ok(g >= 0 && g <= 63);
    assert.ok(b >= 0 && b <= 63);
  }
});
