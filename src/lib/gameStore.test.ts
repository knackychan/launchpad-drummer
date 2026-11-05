import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readGameStore, setGameStore } from './gameStore.ts';

function memory() {
  const values = new Map<string, string>();
  return { values, get length() { return values.size; }, key: (i: number) => [...values.keys()][i] ?? null,
    getItem: (k: string) => values.get(k) ?? null, setItem: (k: string, v: string) => { values.set(k, v); },
    removeItem: (k: string) => { values.delete(k); }, clear: () => values.clear() };
}

test('frame stores persist under separate game prefixes and survive a new read', () => {
  const storage = memory();
  storage.setItem('app.preference', 'untouched');
  setGameStore(storage, 'dustline', 'dustline-midi-v2', '{"bindings":{}}');
  setGameStore(storage, 'sector', 'dustline-midi-v2', 'different');
  assert.deepEqual({ ...readGameStore(storage, 'dustline') }, { 'dustline-midi-v2': '{"bindings":{}}' });
  assert.equal(storage.getItem('lpd.game.dustline.dustline-midi-v2'), '{"bindings":{}}');
  setGameStore(storage, 'dustline', 'dustline-midi-v2', null);
  assert.deepEqual({ ...readGameStore(storage, 'dustline') }, {});
  assert.equal(storage.getItem('app.preference'), 'untouched');
  assert.equal(readGameStore(storage, 'sector')['dustline-midi-v2'], 'different');
});

test('untrusted messages cannot escape a game namespace or store non-strings', () => {
  const storage = memory();
  for (const key of ['', '../app', '__proto__', {}, 4, 'a'.repeat(129)]) assert.throws(() => setGameStore(storage, 'dustline', key, 'x'));
  for (const value of [undefined, {}, [], 4]) assert.throws(() => setGameStore(storage, 'dustline', 'x', value));
  assert.throws(() => setGameStore(storage, '../sector', 'x', 'y'));
  assert.equal(storage.length, 0);
});

test('the 32 KB limit includes UTF-8, all keys and JSON overhead without losing old values', () => {
  const storage = memory();
  setGameStore(storage, 'dustline', 'a', 'x'.repeat(16300));
  setGameStore(storage, 'dustline', 'b', 'x'.repeat(16300));
  assert.throws(() => setGameStore(storage, 'dustline', 'c', 'x'.repeat(200)));
  assert.throws(() => setGameStore(storage, 'dustline', 'a', '海'.repeat(6000)));
  assert.equal(readGameStore(storage, 'dustline').a.length, 16300);
  storage.setItem('lpd.game.sector.huge', 'x'.repeat(40000));
  assert.deepEqual({ ...readGameStore(storage, 'sector') }, {});
});

test('storage errors reach the caller so failed persistence is reported', () => {
  const storage = memory();storage.setItem = () => { throw new Error('quota'); };
  assert.throws(() => setGameStore(storage, 'dustline', 'a', 'b'), /quota/);
});
