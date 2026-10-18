import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHorde, hitCell, ledFrame, startHorde, tickHorde } from './horde.ts';

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

test('a game over pad restarts, and survivor fire removes the survivor and costs 10 HP', () => {
  const state = createHorde(4);
  startHorde(state);
  state.mode = 'over';
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

test('boss armour does not take damage; its weak point does', () => {
  const state = createHorde(7);
  startHorde(state);
  state.enemies = [{ id: 1, type: 'boss', column: 3, row: 4, hp: 12, stepIn: 1, weak: 0 }];
  hitCell(state, 4, 4);
  assert.equal(state.enemies[0].hp, 12);
  hitCell(state, 3, 4);
  assert.equal(state.enemies[0].hp, 11);
  assert.equal(state.enemies[0].weak, 1);
});

test('a blocked boss spawn waits for a clear 2 by 2 footprint', () => {
  const state = createHorde(8);
  startHorde(state);
  state.queue = ['boss'];
  state.spawnIn = 0;
  state.enemies = Array.from({ length: 8 }, (_, column) =>
    ({ id: column + 1, type: 'grunt' as const, column: column + 1, row: 8, hp: 1, stepIn: 3 }));
  tickHorde(state, .1);
  assert.deepEqual(state.queue, ['boss']);
  state.enemies = [];
  tickHorde(state, .3);
  assert.equal(state.queue.length, 0);
  assert.equal(state.enemies[0].type, 'boss');
  assert.equal(ledFrame(state).filter(rgb => rgb[0] === 63 && rgb[1] === 0).length, 4);
});

test('freeze holds enemy movement; a killed splitter creates adjacent grunts', () => {
  const state = createHorde(9);
  startHorde(state);
  state.charge = 100;
  state.queue = [];
  state.enemies = [{ id: 1, type: 'splitter', column: 4, row: 5, hp: 1, stepIn: .1 }];
  hitCell(state, 9, 7);
  tickHorde(state, .5);
  assert.equal(state.enemies[0].row, 5);
  assert.equal(state.freezeIn, 3.5);
  hitCell(state, 4, 5);
  assert.deepEqual(state.enemies.map(enemy => enemy.column).sort(), [3, 5]);
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
  const frame = ledFrame(state, 0);
  assert.equal(frame.length, 64);
  assert.deepEqual(frame[7 * 8 + 1], [63, 50, 0]);
});
