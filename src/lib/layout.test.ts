import assert from 'node:assert/strict';
import test from 'node:test';
import { LP_CHROME_H, mixChrome, solvePlayLayout } from './layout.ts';

const base = { drawerOpen: true, drawerPinned: true, teleFolded: false, lanes: 5 };

test('1920x1080 with the editor docked matches the spec table', () => {
  // Bay of a 1920x1080 window: minus the CRT frame, rail, top and status strips.
  const l = solvePlayLayout({ ...base, width: 1920 - 24 - 80, height: 1080 - 24 - 68 });
  assert.equal(l.docked, true);
  assert.ok(l.pad >= 56 && l.pad <= 62, `pad ${l.pad}`);
  assert.equal(l.tw, 220);
  assert.ok(l.mix.cw >= 0.85 * l.pad, `columns ${l.mix.cw} keep up with pads ${l.pad}`);
});

test('the mixer plate is as tall as the Launchpad plate', () => {
  for (const [width, height] of [[1176, 628], [1816, 988], [2455, 1296]]) {
    const l = solvePlayLayout({ ...base, width, height });
    const lpH = LP_CHROME_H + 7 * 5 + 8 * l.pad;
    assert.equal(mixChrome(l.mix) + l.mix.fh, Math.max(lpH, mixChrome(l.mix) + 110));
  }
});

test('a small window floats the drawer instead of crushing the mixer', () => {
  const l = solvePlayLayout({ ...base, width: 900, height: 628 });
  assert.equal(l.docked, false);
  assert.equal(l.pad, 36);
});

test('pads stay within 36..112 and the folded telemetry is a strip', () => {
  const big = solvePlayLayout({ ...base, drawerOpen: false, width: 5000, height: 3000 });
  assert.equal(big.pad, 112);
  const folded = solvePlayLayout({ ...base, teleFolded: true, width: 1816, height: 988 });
  assert.equal(folded.tw, 30);
  assert.equal(folded.mini, true);
});
