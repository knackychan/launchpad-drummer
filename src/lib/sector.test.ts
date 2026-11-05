import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import vm from 'node:vm';

const folder = new URL('../../public/games/sector/', import.meta.url);
const context = vm.createContext({});
vm.runInContext(readFileSync(new URL('rules.js', folder), 'utf8'), context);
const R = context.SectorRules;
const playing = () => { const g = R.create(7); R.start(g); return g; };
const enemy = (kind = 'scout', x = 2.5, y = 1.5) => ({ id: 1, kind, x, y, hp: R.TYPES[kind].hp, speed: .3, angle: 0 });
const advance = (g: any, seconds: number) => { for (let i = 0; i < seconds * 20; i++) R.tick(g, .05); };

test('all 64 cells map to MKII notes, with visual top-left = 81 and bottom-left = 11', () => {
  assert.equal(R.cellNote(0, 0), 81); assert.equal(R.cellNote(7, 0), 88);
  assert.equal(R.cellNote(0, 7), 11); assert.equal(R.cellNote(7, 7), 18);
  for (let y = 0; y < 8; y++) for (let x = 0; x < 8; x++) {
    const cell = R.noteCell(R.cellNote(x, y)); assert.equal(cell.x, x); assert.equal(cell.y, y);
  }
  for (const n of [0, 10, 19, 79, 89, 90, 108, 81.5, NaN, '81']) assert.equal(R.noteCell(n), null);
});
test('shots damage armor, destroy targets, score and charge once per kill', () => {
  const g = playing(); g.enemies = [enemy('rock')];
  assert.equal(R.hit(g, 2, 1), true); assert.equal(g.enemies[0].hp, 1); assert.equal(g.score, 0);
  assert.equal(R.hit(g, 2, 1), false, 'cooldown rejects duplicate presses');
  advance(g, .15); R.hit(g, 2, 1);
  assert.equal(g.enemies.length, 0); assert.equal(g.score, 180); assert.equal(g.charge, 10); assert.equal(g.kills, 1);
});
test('empty shots reset combos; invalid and paused shots have no effects', () => {
  const g = playing(); g.combo = 12;
  for (const [x, y] of [[-1, 0], [8, 0], [0, NaN], [1.2, 2]]) R.hit(g, x, y);
  assert.equal(g.shots, 0);
  R.hit(g, 0, 0); assert.equal(g.combo, 0); assert.equal(g.shots, 1);
  R.pause(g); const before = JSON.stringify(g); R.hit(g, 2, 1); advance(g, 2);
  assert.equal(JSON.stringify(g), before);
});
test('repair restores shield up to 100 and never damages the station when missed', () => {
  const g = playing(); g.hp = 95; g.enemies = [enemy('repair')]; R.hit(g, 2, 1);
  assert.equal(g.hp, 100); assert.equal(g.score, 50); assert.equal(g.charge, 0);
  g.enemies = [enemy('repair', 4, 4)]; R.tick(g, .05); assert.equal(g.hp, 100);
});
test('pulse needs full charge, clears only threats, and cannot fire while paused', () => {
  const g = playing(); g.enemies = [enemy(), enemy('armored'), enemy('repair')];
  g.charge = 99; assert.equal(R.firePulse(g), false); assert.equal(g.enemies.length, 3);
  g.charge = 100; g.paused = true; assert.equal(R.firePulse(g), false);
  g.paused = false; assert.equal(R.firePulse(g), true);
  assert.equal(g.enemies.length, 1); assert.equal(g.enemies[0].kind, 'repair'); assert.equal(g.charge, 0); assert.equal(g.score, 400);
});
test('center collisions deplete shields and loss stops the simulation until restart', () => {
  const g = playing(); g.hp = 10; g.enemies = [enemy('rock', 4, 4)];
  R.tick(g, .05); assert.equal(g.hp, 0); assert.equal(g.mode, 'over');
  const time = g.time; advance(g, 2); assert.equal(g.time, time); assert.ok(g.overTime > 1);
  assert.equal(R.hit(g, 2, 1), false);
  R.start(g); assert.equal(g.hp, 100); assert.equal(g.mode, 'play'); assert.equal(g.wave, 1); assert.equal(g.score, 0);
});
test('waves spawn in bounds and advance only after every target clears and the break ends', () => {
  const g = playing(); advance(g, 5);
  assert.ok(g.enemies.length > 0);
  for (const e of g.enemies) assert.ok(e.x >= 0 && e.x < 8 && e.y >= 0 && e.y < 8);
  const quota = g.remaining; g.paused = true; advance(g, 5); assert.equal(g.remaining, quota);
  g.paused = false; g.enemies = []; g.remaining = 0;
  advance(g, 1); assert.equal(g.wave, 1); advance(g, 2); assert.equal(g.wave, 2);
});
test('LED frames contain exactly 80 valid MKII addresses and 6-bit colors, targets use their input cell', () => {
  const g = playing(); g.enemies = [enemy('scout', .3, .4), enemy('rock', 7.3, 7.2)];
  const frame = R.leds(g); assert.equal(frame.length, 80); assert.equal(new Set(frame.map((v: number[]) => v[0])).size, 80);
  for (const [note, ...c] of frame) {
    assert.ok(R.noteCell(note) || note % 10 === 9 && note >= 19 && note <= 89 || note >= 104 && note <= 111);
    assert.ok(c.every((v: number) => Number.isInteger(v) && v >= 0 && v <= 63));
  }
  assert.deepEqual(Array.from(frame.find((v: number[]) => v[0] === 81).slice(1)), [10, 39, 63]);
  assert.deepEqual(Array.from(frame.find((v: number[]) => v[0] === 18).slice(1)), [46, 37, 20]);
  R.hit(g, 0, 0); assert.equal(g.enemies.length, 1);
});
test('non-finite and suspended frame deltas cannot advance gameplay unexpectedly', () => {
  const g = playing(); R.tick(g, NaN); R.tick(g, Infinity); R.tick(g, -1); assert.equal(g.time, 0);
  R.tick(g, 100); assert.equal(g.time, .05);
});
test('desktop, browser launcher and shared game switcher register Sector Defense', () => {
  assert.match(readFileSync(new URL('../App.svelte', import.meta.url), 'utf8'), /id: 'sector', title: 'SECTOR DEFENSE'/);
  assert.match(readFileSync(new URL('../../public/games/index.html', import.meta.url), 'utf8'), /data-launch="sector"/);
  assert.match(readFileSync(new URL('../../public/games/shared/launchband-menu.js', import.meta.url), 'utf8'), /hostPath: '\/games\/sector\/index.html'/);
});
