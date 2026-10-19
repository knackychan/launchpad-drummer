import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CODE_TO_CELL, cellAt, makeLayout } from './hordeLayout.ts';

test('keyboard arena keys map by physical code to a 10 by 4 grid, number row on top', () => {
  assert.deepEqual(CODE_TO_CELL.Digit1, { column: 1, row: 4 });
  assert.deepEqual(CODE_TO_CELL.Digit0, { column: 10, row: 4 });
  assert.deepEqual(CODE_TO_CELL.KeyQ, { column: 1, row: 3 });
  assert.deepEqual(CODE_TO_CELL.Semicolon, { column: 10, row: 2 });
  assert.deepEqual(CODE_TO_CELL.KeyZ, { column: 1, row: 1 });
  assert.deepEqual(CODE_TO_CELL.Slash, { column: 10, row: 1 });
  assert.equal(Object.keys(CODE_TO_CELL).length, 40);
});

test('rows are staggered like a real keyboard and canvas points find their cell', () => {
  const keys = makeLayout('keys');
  assert.equal(keys.rect(1, 3).x - keys.rect(1, 4).x, 25);
  assert.equal(keys.rect(1, 1).x - keys.rect(1, 2).x, 25);
  const centre = (column: number, row: number) => { const r = keys.rect(column, row); return [r.x + r.w / 2, r.y + r.h / 2] as const; };
  assert.deepEqual(cellAt(keys, ...centre(7, 2)), { column: 7, row: 2 });
  assert.equal(cellAt(keys, 2, 2), null);
  const pad = makeLayout('pad');
  assert.equal(pad.rect(1, 8).y, 36);
  assert.equal(pad.rect(1, 9).y, 0, 'the sea gate sits above row 8');
  assert.deepEqual(cellAt(pad, 158 + 10, 36 * 8 + 5), { column: 1, row: 1 });
});
