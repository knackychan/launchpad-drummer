import assert from 'node:assert/strict';
import test from 'node:test';
import { DB_FLOOR, dbToPos, fmtDb, fmtLp, knobToLp, lpToKnob, posToDb } from './mix.ts';

test('fader law: 0 dB at 75 %, +6 at the top, −∞ at the bottom', () => {
  assert.equal(dbToPos(0), 0.75);
  assert.equal(dbToPos(6), 1);
  assert.equal(dbToPos(-60), 0);
  assert.equal(posToDb(0), -60);
  assert.equal(posToDb(1), 6);
  for (const db of [-40, -12, -3, 0, 2.5, 6]) assert.ok(Math.abs(posToDb(dbToPos(db)) - db) < 1e-9);
});

test('dB and low-pass readouts', () => {
  assert.equal(fmtDb(-60), '−∞ dB');
  assert.equal(fmtDb(0), '0.0 dB');
  assert.equal(fmtDb(-12), '−12.0 dB');
  assert.equal(fmtLp(1), 'OFF');
  assert.equal(fmtLp(0), '200 Hz');
  assert.ok(Math.abs(knobToLp(lpToKnob(5000)) - 5000) < 1e-6);
});

test('the −∞ boundary is exactly the backend silence floor', () => {
  assert.equal(DB_FLOOR, -60); // lpd_core::audio::FADER_FLOOR_DB
  assert.equal(posToDb(0), DB_FLOOR);
  assert.equal(fmtDb(DB_FLOOR), '−∞ dB');
  assert.equal(fmtDb(DB_FLOOR - 10), '−∞ dB');
  assert.notEqual(fmtDb(DB_FLOOR + 0.5), '−∞ dB');
});
