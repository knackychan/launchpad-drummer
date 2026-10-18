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
  assert.equal(state.score, 20);
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
