import assert from 'node:assert/strict';
import test from 'node:test';
import { noteName, outputLabel, rangeLabel, SOUNDS, SOUND_COLORS, SOUND_LABELS, velocityFromY, windowRange } from './keys.ts';
import { createEmptySet } from './model.ts';

test('noteName: C4 is note 60, sharps use #', () => {
  assert.equal(noteName(21), 'A0');
  assert.equal(noteName(60), 'C4');
  assert.equal(noteName(61), 'C#4');
  assert.equal(noteName(108), 'C8');
});

test('velocityFromY: top 30, bottom 127, linear, clamped', () => {
  assert.equal(velocityFromY(0, 100), 30);
  assert.equal(velocityFromY(100, 100), 127);
  assert.equal(velocityFromY(50, 100), 79);
  assert.equal(velocityFromY(-20, 100), 30);
  assert.equal(velocityFromY(400, 100), 127);
  assert.equal(velocityFromY(10, 0), 127, 'a keybed without height is full velocity');
});

test('windowRange follows the octave and clamps it to −1…+2', () => {
  assert.deepEqual(windowRange(0), [36, 84]);
  assert.deepEqual(windowRange(-1), [24, 72]);
  assert.deepEqual(windowRange(3), [60, 108]);
  assert.deepEqual(windowRange(-5), [24, 72]);
});

test('rangeLabel', () => {
  assert.equal(rangeLabel(0), 'C2–C6');
  assert.equal(rangeLabel(-1), 'C1–C5');
  assert.equal(rangeLabel(2), 'C4–C8');
});

test('outputLabel: the master or channel number and name (8 characters)', () => {
  const groups = createEmptySet().mixer.groups;
  groups[7].name = 'VOX';
  groups[0].name = 'Overheads long';
  assert.equal(outputLabel(null, groups), '→ MASTER');
  assert.equal(outputLabel(7, groups), '→ 8 VOX');
  assert.equal(outputLabel(0, groups), '→ 1 OVERHEAD');
});

test('sounds, labels and colours cover the six sounds', () => {
  assert.deepEqual([...SOUNDS], ['piano', 'ePiano', 'organ', 'synthPad', 'synthLead', 'synthBass']);
  assert.equal(SOUND_LABELS.ePiano, 'E.PIANO');
  assert.equal(SOUND_LABELS.synthLead, 'SYNTH LEAD');
  assert.deepEqual(SOUND_COLORS.organ, [63, 20, 0]);
  for (const s of SOUNDS) assert.equal(SOUND_COLORS[s].length, 3);
});
