import { test } from 'node:test';
import assert from 'node:assert/strict';
import { flatten, keyNav, type TreeNode } from './tree.ts';
const tree: TreeNode[] = [
  { name: 'A', path: 'A', depth: 0, isDir: true, open: true, children: [
    { name: 'a', path: 'A/a', depth: 1, isDir: false },
    { name: 'B', path: 'A/B', depth: 1, isDir: true, children: [{ name: 'hidden', path: 'A/B/hidden', depth: 2, isDir: false }] },
  ] }, { name: 'z', path: 'z', depth: 0, isDir: false },
];
test('flatten shows open descendants only', () => assert.deepEqual(flatten(tree).map(n=>n.path), ['A','A/a','A/B','z']));
test('tree navigation clamps and distinguishes folder and file actions', () => {
  const rows=flatten(tree);
  for (const [index,key,result] of [
    [0,'ArrowUp',{index:0}], [3,'ArrowDown',{index:3}], [0,'ArrowDown',{index:1}], [3,'ArrowUp',{index:2}],
    [0,'ArrowRight',{index:1}], [2,'ArrowRight',{index:2,action:'open'}], [1,'ArrowRight',{index:1}],
    [0,'ArrowLeft',{index:0,action:'close'}], [1,'ArrowLeft',{index:0,action:'parent'}],
    [2,' ',{index:2,action:'toggle'}], [1,' ',{index:1,action:'audition'}],
    [0,'Enter',{index:0,action:'toggle'}], [1,'Enter',{index:1,action:'load'}], [1,'q',{index:1}],
  ] as const) assert.deepEqual(keyNav(rows,index,key),result);
});

import { clampIndex, OVERSCAN, revealScrollTop, ROW_H, visibleRange } from './tree.ts';

test('an empty list renders nothing', () => {
  assert.deepEqual(visibleRange(0, 0, 400), { start: 0, end: 0 });
  assert.deepEqual(visibleRange(0, 999, 0), { start: 0, end: 0 });
});

test('the range is the viewport plus ten rows each side, clamped to the list', () => {
  // 400 px = 18.2 rows -> rows 0..18 visible, +10 overscan below, none above.
  assert.deepEqual(visibleRange(5000, 0, 400), { start: 0, end: Math.ceil(400 / ROW_H) + OVERSCAN });
  const mid = visibleRange(5000, 22 * 1000, 400);
  assert.deepEqual(mid, { start: 1000 - OVERSCAN, end: 1000 + Math.ceil(400 / ROW_H) + OVERSCAN });
  assert.ok(mid.end - mid.start < 50, 'a few dozen rows, not 5000');
});

test('the end of the list clamps the range', () => {
  const r = visibleRange(5000, 22 * 4990, 400);
  assert.equal(r.end, 5000);
  assert.ok(r.start >= 4900 && r.start < 5000);
  assert.deepEqual(visibleRange(3, 0, 400), { start: 0, end: 3 });
  assert.deepEqual(visibleRange(5, 22 * 99, 400), { start: 0, end: 5 }, 'scrolled past a list that shrank');
});

test('the top padding shifts the range', () => {
  assert.equal(visibleRange(100, 4 + 22 * 50, 220, ROW_H, 0, 4).start, 50);
});

test('keyboard navigation scrolls the target row into view with the least movement', () => {
  assert.equal(revealScrollTop(5, 0, 400), 0, 'already visible');
  assert.equal(revealScrollTop(30, 0, 400), 30 * ROW_H + ROW_H - 400, 'below the viewport: its bottom edge lines up');
  assert.equal(revealScrollTop(2, 500, 400), 2 * ROW_H, 'above the viewport: its top edge lines up');
  assert.equal(revealScrollTop(0, 500, 400, ROW_H, 4), 0);
  // After revealing, the row is inside the rendered range.
  const top = revealScrollTop(4999, 0, 400);
  const r = visibleRange(5000, top, 400);
  assert.ok(4999 >= r.start && 4999 < r.end);
});

test('collapsing a folder cannot leave the selection past the end', () => {
  assert.equal(clampIndex(40, 12), 11);
  assert.equal(clampIndex(3, 12), 3);
  assert.equal(clampIndex(5, 0), 0);
});
