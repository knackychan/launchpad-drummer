import test from 'node:test';
import assert from 'node:assert/strict';
import { createDefaultPad, LAUNCHPAD_PALETTE, padAriaLabel, createEmptySet } from './model.ts';

test('new sets carry independent version 2 factory patches', () => {
  const first = createEmptySet(), second = createEmptySet();
  assert.equal(first.version, 2);
  assert.equal(first.keys.patch.name, 'E.PIANO');
  assert.deepEqual(first.keys.patchSource, { factory: 'ePiano' });
  first.keys.patch.params.filter.cutoff = 300;
  assert.equal(second.keys.patch.params.filter.cutoff, 12000);
});

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

test('padAriaLabel describes empty, filled and missing pads', () => {
  const groups = createEmptySet().mixer.groups;
  const kick = { ...createDefaultPad(1, 3), sample: 'samples/kick_808.wav', mixGroup: 3 };
  assert.equal(padAriaLabel(kick, 1, 3, groups, false), 'Row 4, column 2, KICK 808, mix 4, choke off');
  assert.equal(padAriaLabel(null, 0, 0, groups, false), 'Row 1, column 1, empty');
  const snare = { ...createDefaultPad(2, 1), sample: 'samples/snare.wav', mixGroup: 1, chokeGroup: 1 };
  assert.equal(padAriaLabel(snare, 2, 1, groups, true), 'Row 2, column 3, SNARE, mix 2, choke 1, sample missing');
});
