import assert from 'node:assert/strict';
import test from 'node:test';
import { BPM, clickValue, foldSummary, laneLabel, paint, setLength, stepAngle, SWING, type StepValue } from './seq.ts';
import type { Pattern } from './model.ts';

test('clickValue: a click toggles, an accent click sets or clears the accent', () => {
  assert.deepEqual(([0, 1, 2] as StepValue[]).map(v => clickValue(v, false)), [1, 0, 0]);
  assert.deepEqual(([0, 1, 2] as StepValue[]).map(v => clickValue(v, true)), [2, 2, 0]);
});

test('paint returns a copy and does not mutate its input', () => {
  const steps: StepValue[] = [0, 0, 1, 0];
  const next = paint(steps, 1, 2);
  assert.deepEqual(next, [0, 2, 1, 0]);
  assert.deepEqual(steps, [0, 0, 1, 0]);
  assert.notEqual(next, steps);
});

test('foldSummary', () => {
  assert.equal(foldSummary(1, 8, 120), '1 lane · 8 steps · 120 BPM');
  assert.equal(foldSummary(5, 16, 100), '5 lanes · 16 steps · 100 BPM');
  assert.equal(foldSummary(0, 32, 60), '0 lanes · 32 steps · 60 BPM');
});

test('laneLabel: bank number and the sample name, EMPTY without a pad', () => {
  assert.equal(laneLabel(0, 'samples/kick_808.wav'), '1·KICK_808');
  assert.equal(laneLabel(2, String.raw`C:\kits\Snare 2.flac`), '3·SNARE 2');
  assert.equal(laneLabel(2, null), 'EMPTY');
  assert.equal(laneLabel(2, ''), 'EMPTY');
});

test('ranges', () => {
  assert.deepEqual(BPM, { min: 60, max: 200, def: 100 });
  assert.deepEqual(SWING, { min: 50, max: 75, def: 50 });
});

const lane = (steps: number[]) => ({ bank: 0, x: 0, y: 0, mute: false, steps: [...steps, ...Array(32 - steps.length).fill(0)] });
const pattern = (length: Pattern['length'], steps: number[]): Pattern => ({ lanes: [lane(steps)], length, bpm: 100, swing: 50 });

test('setLength growing copies an empty tail and keeps a used one; input is not mutated', () => {
  const p = pattern(16, [1, 0, 2, 0, 1]);
  const grown = setLength(p, 32);
  assert.equal(grown.length, 32);
  for (let i = 0; i < 16; i++) assert.equal(grown.lanes[0].steps[i + 16], grown.lanes[0].steps[i]);
  assert.equal(p.length, 16);
  assert.equal(p.lanes[0].steps[16], 0, 'the input keeps its empty tail');

  const used = pattern(16, [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
  const kept = setLength(used, 32);
  assert.equal(kept.lanes[0].steps[16], 0);
  assert.equal(kept.lanes[0].steps[19], 2);
});

test('setLength 32 → 16 → 32 keeps the hidden steps', () => {
  const p = pattern(32, [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
  const there = setLength(p, 16);
  assert.equal(there.length, 16);
  assert.equal(there.lanes[0].steps[20], 2);
  const back = setLength(there, 32);
  assert.equal(back.lanes[0].steps[20], 2);
  assert.equal(back.lanes[0].steps[16], 0);
});

test('setLength from 8 repeats the 8 steps', () => {
  const grown = setLength(pattern(8, [1, 0, 0, 2]), 16);
  assert.deepEqual(grown.lanes[0].steps.slice(8, 12), [1, 0, 0, 2]);
  assert.equal(grown.lanes[0].steps[16], 0);
});

test('stepAngle: the playhead planet starts at the top and goes clockwise', () => {
  assert.equal(stepAngle(0, 0, 16), -Math.PI / 2);
  assert.ok(Math.abs(stepAngle(4, 0, 16) - 0) < 1e-9, 'a quarter turn is 3 o clock');
  assert.ok(Math.abs(stepAngle(0, 0.5, 4) - (-Math.PI / 2 + Math.PI / 4)) < 1e-9);
});
