import assert from 'node:assert/strict';
import test from 'node:test';
import {
  capStep, createSpectrum, fmtPeakHz, fmtPk, holdUpdate, holdValue, meterColor, msPerDiv, silenceStep,
  specLevel, stepBands, vuPos,
} from './telemetry.ts';

test('spectrum maps -72..0 dBFS to 0..1 and clamps', () => {
  assert.equal(specLevel(-72), 0);
  assert.equal(specLevel(0), 1);
  assert.equal(specLevel(-36), 0.5);
  assert.equal(specLevel(-100), 0);
  assert.equal(specLevel(6), 1);
});

test('bars fall 12 % per frame and rise at once', () => {
  const s = createSpectrum();
  const loud = Array(28).fill(0); // 0 dBFS
  stepBands(s, loud, 0);
  assert.equal(s.bands[0], 1);
  const quiet = Array(28).fill(-100);
  stepBands(s, quiet, 16);
  assert.ok(Math.abs(s.bands[0] - 0.88) < 1e-9);
  stepBands(s, quiet, 32);
  assert.ok(Math.abs(s.bands[0] - 0.88 * 0.88) < 1e-9);
});

test('peak caps hold 400 ms then fall 1 % per frame, never below the bar', () => {
  const s = createSpectrum();
  stepBands(s, Array(28).fill(0), 1000);
  assert.equal(s.caps[3], 1);
  // Bar falls; the cap holds for 400 ms.
  stepBands(s, Array(28).fill(-100), 1100);
  stepBands(s, Array(28).fill(-100), 1300);
  assert.equal(s.caps[3], 1);
  stepBands(s, Array(28).fill(-100), 1500);
  assert.ok(Math.abs(s.caps[3] - 0.99) < 1e-9);
  stepBands(s, Array(28).fill(-100), 1516);
  assert.ok(Math.abs(s.caps[3] - 0.98) < 1e-9);
  assert.equal(capStep(0.5, 0.9, 5000), 0.9, 'a cap never sinks below its bar');
});

test('master meters: -48..+3 dB, green below -9, amber to 0, red above', () => {
  assert.equal(vuPos(-48), 0);
  assert.equal(vuPos(3), 1);
  assert.equal(vuPos(-100), 0);
  assert.equal(vuPos(-22.5), 0.5);
  assert.equal(meterColor(-20), '#5cff9d');
  assert.equal(meterColor(-9), '#5cff9d');
  assert.equal(meterColor(-8.9), '#ffb21a');
  assert.equal(meterColor(0), '#ffb21a');
  assert.equal(meterColor(0.1), '#ff3d4f');
});

test('peak hold: 1.2 s, then it falls 30 dB per second', () => {
  const h = { v: -100, t: 0 };
  holdUpdate(h, -6, 1000);
  assert.deepEqual(h, { v: -6, t: 1000 });
  holdUpdate(h, -20, 1500); // a lower reading does not replace it
  assert.equal(h.v, -6);
  assert.equal(holdValue(h, 2200), -6);
  assert.ok(Math.abs(holdValue(h, 3200) - -36) < 1e-9);
  assert.equal(holdValue(h, 60_000), -100);
  // A reading below the falling hold changes nothing; one above it takes over.
  holdUpdate(h, -50, 3200);
  assert.ok(Math.abs(holdValue(h, 3200) - -36) < 1e-9);
  holdUpdate(h, -30, 3200);
  assert.deepEqual(h, { v: -30, t: 3200 });
});

test('readouts', () => {
  assert.equal(fmtPeakHz(94.2), '94 Hz');
  assert.equal(fmtPeakHz(1195), '1.2 kHz');
  assert.equal(fmtPeakHz(0), '—');
  assert.equal(fmtPk(-100), 'PK −∞');
  assert.equal(fmtPk(-61), 'PK −∞');
  assert.equal(fmtPk(-6), 'PK −6.0 dB');
  assert.equal(fmtPk(1.25), 'PK +1.3 dB');
  assert.equal(fmtPk(0), 'PK 0.0 dB');
  assert.equal(msPerDiv(48000), '2.7 ms/div');
  assert.equal(msPerDiv(44100), '2.9 ms/div');
  assert.equal(msPerDiv(0), '— ms/div');
});

test('silence means master below -90 dBFS for 500 ms', () => {
  const s = { since: null as number | null };
  assert.equal(silenceStep(s, -95, 0), false);
  assert.equal(silenceStep(s, -95, 499), false);
  assert.equal(silenceStep(s, -95, 500), true);
  assert.equal(silenceStep(s, -20, 600), false, 'sound ends it at once');
  assert.equal(silenceStep(s, -95, 700), false, 'and the 500 ms start over');
  assert.equal(silenceStep(s, -95, 1200), true);
});
