import { test } from 'node:test';
import assert from 'node:assert/strict';
import { armBomb, createHorde, drainEvents, freezeEnemies, gateFraction, hitCell, ledFrame, padPress, rankOf,
  resetHorde, startHorde, stepProgress, tickHorde, togglePause, type Enemy } from './horde.ts';

const grunt = (column: number, row: number, extra: Partial<Enemy> = {}): Enemy =>
  ({ id: 1, type: 'grunt', column, row, hp: 1, stepIn: 5, ...extra });

test('a shot hits a lit enemy and increments combo, kill count, and score', () => {
  const state = createHorde(1);
  startHorde(state);
  state.enemies = [{ id: 1, type: 'grunt', column: 3, row: 4, hp: 1, stepIn: 1 }];
  const result = hitCell(state, 3, 4);
  assert.equal(result.hit, true);
  assert.equal(state.enemies.length, 0);
  assert.equal(state.kills, 1);
  assert.equal(state.combo, 1);
  assert.equal(state.score, 30);
});

test('a game over pad restarts after 1.2 s, and survivor fire removes the survivor and costs 10 HP', () => {
  const state = createHorde(4);
  startHorde(state);
  state.mode = 'over'; state.modeT = 0;
  hitCell(state, 2, 3);
  assert.equal(state.mode, 'over', 'presses right after the loss are ignored');
  tickHorde(state, 1.3);
  hitCell(state, 2, 3);
  assert.equal(state.mode, 'play');
  assert.equal(state.wave, 1);
  state.enemies = [{ id: 1, type: 'survivor', column: 2, row: 3, hp: 1, stepIn: 1 }];
  hitCell(state, 2, 3);
  assert.equal(state.hp, 90);
  assert.equal(state.enemies.length, 0);
});

test('a survivor reaching base heals and scores instead of damaging it', () => {
  const state = createHorde(5);
  startHorde(state);
  state.hp = 80;
  state.queue = [];
  state.enemies = [{ id: 1, type: 'survivor', column: 1, row: 1, hp: 1, stepIn: .1 }];
  tickHorde(state, .2);
  assert.equal(state.saved, 1);
  assert.equal(state.hp, 95); // +5 save and +10 wave clear
  assert.equal(state.score, 100);
});

test('beat mode moves on beats, and bombs hit a 3 by 3 area', () => {
  const state = createHorde(6);
  state.playMode = 'beat';
  startHorde(state);
  state.queue = [];
  state.enemies = [{ id: 1, type: 'grunt', column: 3, row: 5, hp: 1, stepIn: 1, beats: 0 }];
  tickHorde(state, .3);
  assert.equal(state.enemies[0].row, 5);
  tickHorde(state, .34);
  assert.equal(state.enemies[0].row, 5);
  tickHorde(state, .63);
  assert.equal(state.enemies[0].row, 4);
  hitCell(state, 9, 8);
  assert.equal(state.armed, true);
  hitCell(state, 4, 4);
  assert.equal(state.enemies.length, 0);
  assert.equal(state.bombs, 0);
});

test('boss armour does not take damage; its weak point does and then moves to another cell', () => {
  const state = createHorde(7);
  startHorde(state);
  state.enemies = [{ id: 1, type: 'boss', column: 3, row: 4, hp: 12, stepIn: 9, weak: 0 }];
  hitCell(state, 4, 4);
  assert.equal(state.enemies[0].hp, 12);
  hitCell(state, 3, 4);
  assert.equal(state.enemies[0].hp, 11);
  assert.notEqual(state.enemies[0].weak, 0);
});

test('a boss in the sea gate waits for a clear 2 by 2 footprint, then walks in', () => {
  const state = createHorde(8);
  startHorde(state);
  state.queue = ['boss'];
  state.spawnIn = 0;
  tickHorde(state, .1);
  assert.equal(state.queue.length, 0);
  const boss = state.enemies[0];
  assert.equal(boss.type, 'boss');
  assert.equal(boss.state, 'gate');
  assert.equal(boss.row, 9);
  const blockers: Enemy[] = Array.from({ length: 8 }, (_, column) => grunt(column + 1, 8, { id: column + 10, stepIn: 9 }));
  state.enemies.push(...blockers);
  tickHorde(state, 2.3);
  assert.equal(boss.state, 'gate', 'a blocked gate holds the boss');
  state.enemies = [boss];
  tickHorde(state, .3);
  assert.equal(boss.state, 'walk');
  assert.equal(boss.row, 7);
  assert.equal(ledFrame(state).filter(rgb => rgb[0] === 30 && rgb[1] === 0).length >= 3, true);
});

test('freeze holds enemy movement; a killed splitter creates adjacent grunts and grows the wave', () => {
  const state = createHorde(9);
  startHorde(state);
  state.charge = 100;
  state.queue = [];
  state.waveTotal = 10;
  state.enemies = [{ id: 1, type: 'splitter', column: 4, row: 5, hp: 1, stepIn: .1 }];
  hitCell(state, 9, 7);
  tickHorde(state, .5);
  assert.equal(state.enemies[0].row, 5);
  assert.equal(state.freezeIn, 3.5);
  hitCell(state, 4, 5);
  assert.deepEqual(state.enemies.map(enemy => enemy.column).sort(), [3, 5]);
  assert.equal(state.waveTotal, 12);
});

test('misses reset combo without damaging base; enemies reaching row one cost HP', () => {
  const state = createHorde(2);
  startHorde(state);
  state.enemies = [{ id: 1, type: 'grunt', column: 1, row: 1, hp: 1, stepIn: 0.1 }];
  state.combo = 4;
  assert.equal(hitCell(state, 8, 8).hit, false);
  assert.equal(state.combo, 0);
  tickHorde(state, 0.2);
  assert.equal(state.hp, 94);
});

test('the frame maps game row one to the hardware bottom and encodes RGB pads', () => {
  const state = createHorde(3);
  startHorde(state);
  state.enemies = [{ id: 1, type: 'runner', column: 2, row: 1, hp: 1, stepIn: 1 }];
  const frame = ledFrame(state, 1 / 24); // the row-one pulse peaks at this time
  assert.equal(frame.length, 64);
  assert.deepEqual(frame[7 * 8 + 1], [63, 50, 0]);
});

test('base damage depends on the enemy: runner 5, splitter 8, brute 15, boss 35', () => {
  for (const [type, damage] of [['runner', 5], ['splitter', 8], ['brute', 15], ['boss', 35]] as const) {
    const state = createHorde(10);
    startHorde(state);
    state.enemies = [{ id: 1, type, column: 1, row: 1, hp: 3, stepIn: .1 }];
    tickHorde(state, .2);
    assert.equal(state.hp, 100 - damage, type);
  }
});

test('a spawned enemy surfaces in the sea gate and cannot be shot until it walks in', () => {
  const state = createHorde(11);
  startHorde(state);
  state.queue = ['grunt'];
  state.spawnIn = 0;
  tickHorde(state, .1);
  const enemy = state.enemies[0];
  assert.equal(enemy.state, 'gate');
  assert.equal(enemy.row, 9);
  assert.equal(hitCell(state, enemy.column, 8).hit, false);
  assert.equal(state.enemies.length, 1);
  tickHorde(state, .9);
  assert.equal(enemy.state, 'gate');
  assert.ok(gateFraction(state, enemy) > 0 && gateFraction(state, enemy) < .2);
  tickHorde(state, .2);
  assert.equal(enemy.state, 'walk');
  assert.equal(enemy.row, 8);
});

test('the keyboard arena is 10 by 4, steps 1.55 times slower, and takes Launchpad rows 5 to 8', () => {
  const state = createHorde(12);
  resetHorde(state, { layout: 'keys' });
  startHorde(state);
  state.queue = [];
  state.enemies = [grunt(10, 4), grunt(2, 1, { id: 2 })];
  assert.equal(hitCell(state, 10, 4).killed, true);
  assert.equal(hitCell(state, 9, 8).hit, false, 'column nine is a key, not the bomb button');
  assert.equal(state.armed, false);
  padPress(state, 2, 5); // Launchpad row 5 is key row 1
  assert.equal(state.enemies.length, 0);
  state.queue = ['grunt'];
  state.spawnIn = 0;
  tickHorde(state, .1);
  tickHorde(state, 1.1);
  const walker = state.enemies[0];
  assert.equal(walker.state, 'walk');
  assert.equal(walker.row, 4);
  assert.ok(Math.abs(walker.stepIn - 1.4 * 1.55) < 1e-9);
  assert.equal(stepProgress(state, walker), 0);
  state.enemies = [{ id: 3, type: 'runner', column: 2, row: 1, hp: 1, stepIn: 1 }];
  const frame = ledFrame(state, 1 / 24);
  assert.deepEqual(frame[(8 - 5) * 8 + 1], [63, 50, 0]);
});

test('arm and freeze are explicit actions that deny politely and toggle the bomb', () => {
  const state = createHorde(13);
  startHorde(state);
  state.bombs = 0;
  armBomb(state);
  assert.equal(state.armed, false);
  assert.ok(drainEvents(state).some(event => event.kind === 'deny'));
  state.bombs = 1;
  armBomb(state);
  assert.equal(state.armed, true);
  armBomb(state);
  assert.equal(state.armed, false);
  freezeEnemies(state);
  assert.equal(state.freezeIn, 0);
  state.charge = 100;
  freezeEnemies(state);
  assert.equal(state.freezeIn, 4);
  assert.equal(state.charge, 0);
});

test('Free-mode speed scales enemy steps', () => {
  const state = createHorde(14);
  startHorde(state);
  state.queue = [];
  state.speed = 1.25;
  state.enemies = [grunt(3, 5, { stepIn: 1 })];
  tickHorde(state, .78);
  assert.equal(state.enemies[0].row, 5);
  tickHorde(state, .03);
  assert.equal(state.enemies[0].row, 4);
});

test('a blocked enemy sidesteps or retries after 45 per cent of a step instead of waiting a whole step', () => {
  const state = createHorde(15);
  startHorde(state);
  state.queue = [];
  state.enemies = [grunt(3, 4, { stepIn: .1 }), grunt(3, 3, { id: 2, stepIn: 9 })];
  tickHorde(state, .2);
  const blocked = state.enemies[0];
  assert.equal(blocked.row, 4);
  assert.ok(blocked.column !== 3 || Math.abs(blocked.stepIn - 1.4 * .45) < 1e-9);
});

test('a brute is only pushed back when the cell behind it is free', () => {
  const state = createHorde(16);
  startHorde(state);
  state.queue = [];
  state.enemies = [{ id: 1, type: 'brute', column: 3, row: 4, hp: 3, stepIn: 9 }, grunt(3, 5, { id: 2 })];
  hitCell(state, 3, 4);
  assert.equal(state.enemies[0].row, 4, 'blocked by the grunt behind it');
  state.enemies = [{ id: 1, type: 'brute', column: 3, row: 4, hp: 3, stepIn: 9 }];
  hitCell(state, 3, 4);
  assert.equal(state.enemies[0].row, 5);
  state.enemies = [{ id: 1, type: 'brute', column: 3, row: 8, hp: 3, stepIn: 9 }];
  hitCell(state, 3, 8);
  assert.equal(state.enemies[0].row, 8);
});

test('clearing a wave heals once, shows an intermission for 3 s, then starts the next wave', () => {
  const state = createHorde(17);
  startHorde(state);
  state.hp = 50;
  state.queue = [];
  state.enemies = [grunt(3, 4)];
  hitCell(state, 3, 4);
  tickHorde(state, .01);
  assert.equal(state.mode, 'inter');
  assert.equal(state.hp, 60);
  assert.equal(state.wave, 1);
  assert.equal(hitCell(state, 3, 4).hit, false);
  tickHorde(state, 2);
  assert.equal(state.mode, 'inter');
  tickHorde(state, 1.1);
  assert.equal(state.mode, 'play');
  assert.equal(state.wave, 2);
  assert.equal(state.hp, 60);
  assert.equal(state.queue.length, 14);
  assert.equal(state.waveTotal, 14);
  assert.equal(state.waveDone, 0);
});

test('the Crab King weak point also rotates by itself every 1.8 s', () => {
  const state = createHorde(18);
  startHorde(state);
  state.queue = [];
  state.enemies = [{ id: 1, type: 'boss', column: 3, row: 4, hp: 12, stepIn: 99, weak: 0 }];
  tickHorde(state, 1.7);
  assert.equal(state.enemies[0].weak, 0);
  tickHorde(state, .2);
  assert.equal(state.enemies[0].weak, 1);
});

test('shots, hits and on-beat hits are counted; on-beat kills score double', () => {
  const state = createHorde(19);
  state.playMode = 'beat';
  startHorde(state);
  state.queue = [];
  state.enemies = [grunt(3, 4), { id: 2, type: 'boss', column: 5, row: 4, hp: 12, stepIn: 99, weak: 0 }];
  hitCell(state, 8, 8); // miss
  assert.equal(state.shots, 1);
  assert.equal(state.hits, 0);
  state.beatClock = .62; // 0.005 s before the next beat of 0.625 s
  hitCell(state, 3, 4);
  assert.equal(state.score, 60);
  assert.equal(state.perfect, 1);
  assert.equal(state.hits, 1);
  hitCell(state, 6, 4); // armour: a shot but not a hit
  assert.equal(state.shots, 3);
  assert.equal(state.hits, 1);
  assert.equal(state.combo, 1, 'armour leaves the combo alone');
});

test('a bomb or a shot on a survivor is friendly fire', () => {
  const state = createHorde(20);
  startHorde(state);
  state.queue = [];
  state.enemies = [{ id: 1, type: 'survivor', column: 3, row: 4, hp: 1, stepIn: 9 }, grunt(7, 7, { id: 2 })];
  armBomb(state);
  hitCell(state, 3, 4);
  assert.equal(state.hp, 90);
  assert.equal(state.enemies.length, 1);
  assert.equal(state.bombs, 0);
});

test('rank letters follow the combo and pause stops the game but not the clock', () => {
  assert.deepEqual([0, 5, 10, 20, 35].map(rankOf), ['D', 'C', 'B', 'A', 'S']);
  const state = createHorde(21);
  startHorde(state);
  state.queue = [];
  state.enemies = [grunt(3, 4, { stepIn: .1 })];
  togglePause(state);
  const time = state.time;
  tickHorde(state, .5);
  assert.equal(state.enemies[0].row, 4);
  assert.ok(state.time > time);
  assert.equal(hitCell(state, 3, 4).hit, false);
  togglePause(state);
  tickHorde(state, .2);
  assert.equal(state.enemies[0].row, 3);
});

test('events describe shots for the view and are drained once', () => {
  const state = createHorde(22);
  startHorde(state);
  drainEvents(state);
  state.queue = [];
  state.combo = 4;
  state.enemies = [grunt(3, 4)];
  hitCell(state, 3, 4);
  hitCell(state, 8, 8);
  const kinds = drainEvents(state).map(event => event.kind);
  assert.ok(['shot', 'hit', 'kill', 'rank', 'miss'].every(kind => kinds.includes(kind as never)), kinds.join());
  assert.equal(drainEvents(state).length, 0);
});

test('beat mode: gate enemies enter after two beats, the boss after four', () => {
  const state = createHorde(23);
  state.playMode = 'beat';
  startHorde(state);
  state.queue = ['boss'];
  state.spawnIn = 1;
  tickHorde(state, .63); // beat 1: the boss surfaces and counts its first gate beat
  const boss = state.enemies[0];
  assert.equal(boss.state, 'gate');
  tickHorde(state, 1.25); // beats 2 and 3
  assert.equal(boss.state, 'gate');
  tickHorde(state, .63); // beat 4
  assert.equal(boss.state, 'walk');
});
