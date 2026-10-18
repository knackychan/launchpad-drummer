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
