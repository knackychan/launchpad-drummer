import assert from 'node:assert/strict';
import test from 'node:test';
import type { LoadStatus, SetModel, Startup } from './model.ts';
import { createEmptySet } from './model.ts';
import {
  acceptLoad, announcement, createGeneration, eventFirst, fetchReadyState, INITIAL_LOAD, isSlow, isTerminal, phaseLabel, progressOf, SLOW_AFTER_MS,
} from './startup.ts';
import type { ReadyCalls } from './startup.ts';

const status = (over: Partial<LoadStatus>): LoadStatus => ({ ...INITIAL_LOAD, ...over });

test('a newer revision replaces the current one, an older or equal one does not', () => {
  const at8 = status({ revision: 8, phase: 'loadingSamples', completed: 4, total: 9 });
  const at7 = status({ revision: 7, phase: 'readingSet' });
  assert.equal(acceptLoad(at8, at7), at8, 'a snapshot answered late cannot rewind progress');
  assert.equal(acceptLoad(at8, { ...at8 }), at8, 'a duplicate changes nothing');
  const at9 = status({ revision: 9, phase: 'installing' });
  assert.equal(acceptLoad(at8, at9), at9);
  assert.equal(acceptLoad(null, at7), at7, 'the first thing heard is always taken');
});

test('revisions keep counting across requests: a newer request with a higher revision wins', () => {
  const old = status({ requestId: 1, revision: 12, phase: 'failed', error: 'x' });
  const next = status({ requestId: 2, revision: 13, phase: 'readingSet' });
  assert.equal(acceptLoad(old, next), next);
});

test('an unknown, zero or broken total is indeterminate and never a percentage', () => {
  for (const total of [null, 0, -3, Number.NaN, Number.POSITIVE_INFINITY]) {
    const p = progressOf(status({ phase: 'loadingSamples', completed: 5, total }));
    assert.deepEqual(p, { kind: 'indeterminate' }, String(total));
  }
  assert.deepEqual(progressOf(status({ phase: 'readingSet', completed: 1, total: 4 })), { kind: 'indeterminate' }, 'only counted phases show a bar');
});

test('counted progress is clamped to its total', () => {
  assert.deepEqual(progressOf(status({ phase: 'loadingSamples', completed: 12, total: 32 })), { kind: 'determinate', value: 12, max: 32 });
  assert.deepEqual(progressOf(status({ phase: 'loadingSamples', completed: 40, total: 32 })), { kind: 'determinate', value: 32, max: 32 });
  assert.deepEqual(progressOf(status({ phase: 'preparingSamples', completed: -1, total: 32 })), { kind: 'determinate', value: 0, max: 32 });
});

test('labels use the phase and the real count, never a path', () => {
  assert.equal(phaseLabel(status({ phase: 'settings' })), 'Reading settings…');
  assert.equal(phaseLabel(status({ phase: 'loadingSamples', completed: 12, total: 32 })), 'Loading samples — 12 of 32');
  assert.equal(phaseLabel(status({ phase: 'loadingSamples', total: 0 })), 'Loading samples…');
  assert.equal(phaseLabel(status({ phase: 'preparingSamples', completed: 3, total: 33 })), 'Preparing default samples — 3 of 33');
  assert.equal(phaseLabel(status({ phase: 'failed', error: 'C:\\Users\\me\\Kit\\set.json' })), 'The set did not load');
});

test('terminal phases end a request', () => {
  assert.deepEqual((['ready', 'needsSet', 'failed', 'loadingSamples', 'settings'] as const).map(isTerminal), [true, true, true, false, false]);
});

test('announcements change per phase or per quarter of a counted phase, not per file', () => {
  const at = (completed: number) => announcement(status({ phase: 'loadingSamples', completed, total: 40 }));
  assert.equal(at(1), at(9), 'files inside one quarter sound the same');
  assert.notEqual(at(9), at(10));
  assert.equal(announcement(status({ phase: 'readingSet' })), 'Reading set…');
});

test('slow means ten seconds', () => {
  assert.equal(SLOW_AFTER_MS, 10_000);
  assert.equal(isSlow(9_999), false);
  assert.equal(isSlow(10_000), true);
});

test('only the newest generation token is current', () => {
  const gate = createGeneration();
  const a = gate.next(), b = gate.next();
  assert.equal(gate.isCurrent(a), false);
  assert.equal(gate.isCurrent(b), true);
  gate.bump();
  assert.equal(gate.isCurrent(b), false);
});

// ---- fetchReadyState --------------------------------------------------------------------------

const startupLoaded: Startup = { kind: 'loaded', path: 'P', name: 'N', fromBackup: false, backupSavedAt: null };
const ready = (requestId: number): LoadStatus => status({ requestId, revision: 5, phase: 'ready' });

/** A promise the test settles by hand. */
function deferred<T>() {
  let resolve!: (v: T) => void, reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

function calls(over: Partial<ReadyCalls> = {}): ReadyCalls {
  const set: SetModel = createEmptySet();
  return {
    set: async () => set, startup: async () => startupLoaded, bank: async () => 2,
    save: async () => ({ state: 'saved', reason: null }), history: async () => ({ canUndo: true, canRedo: false }),
    missing: async () => [], status: async () => ready(4), ...over,
  };
}

test('a failing optional request does not stop the set from loading', async () => {
  const result = await fetchReadyState(calls({
    bank: async () => { throw new Error('no bank'); }, save: async () => { throw new Error('no save'); },
    history: () => Promise.reject(new Error('no history')), missing: async () => { throw new Error('no missing'); },
  }), 4, () => true);
  assert.equal(result.kind, 'ok');
  if (result.kind !== 'ok') return;
  assert.deepEqual([result.bank, result.save, result.history, result.missing], [null, null, null, null]);
});

test('a failing required request is an error with its message', async () => {
  const result = await fetchReadyState(calls({ set: async () => { throw new Error('engine said no'); } }), 4, () => true);
  assert.deepEqual(result, { kind: 'error', error: 'engine said no' });
  const noStartup = await fetchReadyState(calls({ startup: async () => undefined as unknown as Startup }), 4, () => true);
  assert.equal(noStartup.kind, 'error');
});

test('the independent requests are all started before any answer arrives', async () => {
  const started: string[] = [];
  const gate = deferred<void>();
  const slow = <T>(name: string, value: T) => async () => { started.push(name); await gate.promise; return value; };
  const pending = fetchReadyState(calls({
    set: slow('set', createEmptySet()), startup: slow('startup', startupLoaded), bank: slow('bank', 0),
    save: slow('save', { state: 'saved' as const, reason: null }), history: slow('history', { canUndo: false, canRedo: false }), missing: slow('missing', []),
  }), 4, () => true);
  assert.deepEqual(started, ['set', 'startup', 'bank', 'save', 'history', 'missing'], 'no waterfall');
  gate.resolve();
  assert.equal((await pending).kind, 'ok');
});

test('an older initial response that arrives after the user opened something else is dropped', async () => {
  const gen = createGeneration();
  const token = gen.next();
  const slowSet = deferred<SetModel>();
  const pending = fetchReadyState(calls({ set: () => slowSet.promise }), 4, () => gen.isCurrent(token));
  gen.next(); // the user opens another set while the first answer is on its way
  slowSet.resolve(createEmptySet());
  assert.deepEqual(await pending, { kind: 'stale' });
});

test('two rapid ready notifications: only the second one is applied', async () => {
  const gen = createGeneration();
  const applied: number[] = [];
  const run = async (requestId: number, set: Promise<SetModel>) => {
    const token = gen.next();
    const r = await fetchReadyState(calls({ set: () => set, status: async () => ready(requestId) }), requestId, () => gen.isCurrent(token));
    if (r.kind === 'ok') applied.push(requestId);
  };
  const first = deferred<SetModel>(), second = deferred<SetModel>();
  const a = run(4, first.promise);
  const b = run(5, second.promise);
  second.resolve(createEmptySet());
  first.resolve(createEmptySet()); // the older answer comes last
  await Promise.all([a, b]);
  assert.deepEqual(applied, [5]);
});

test('if the backend moved to another request meanwhile, the answer is stale', async () => {
  const newer = await fetchReadyState(calls({ status: async () => ready(9) }), 4, () => true);
  assert.deepEqual(newer, { kind: 'stale' });
  const loading = await fetchReadyState(calls({ status: async () => status({ requestId: 4, revision: 6, phase: 'loadingSamples', total: 3 }) }), 4, () => true);
  assert.deepEqual(loading, { kind: 'stale' });
  const unknown = await fetchReadyState(calls({ status: async () => { throw new Error('gone'); } }), 4, () => true);
  assert.equal(unknown.kind, 'ok', 'an unanswered ownership check does not discard good data');
});

test('a newer audio or midi event beats an older snapshot, in either order of arrival', async () => {
  let shown: string | null = null;
  const feed = eventFirst<string>(v => { shown = v; });
  const fetched = deferred<string>();
  const pending = fetched.promise.then(feed.snapshot);
  feed.event('running (event)');
  fetched.resolve('stopped (older snapshot)');
  await pending;
  assert.equal(shown, 'running (event)');

  const calm = eventFirst<string>(v => { shown = v; });
  calm.snapshot('first snapshot');
  assert.equal(shown, 'first snapshot', 'with no event yet the snapshot is taken');
  calm.event('later event');
  calm.snapshot('second snapshot');
  assert.equal(shown, 'later event');
});
