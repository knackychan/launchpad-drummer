import assert from 'node:assert/strict';
import test from 'node:test';
import { ENGINE_PATCH_MAX, EngineLink, parseEngineMessage, rateCap, type EngineHost, type EngineStatus } from './engineBridge.ts';

const frame = (over: Record<string, unknown> = {}) => ({ lpd: 'engine-synth', op: 'frame', on: true, bend: .1, wheel: .5, gain: .8, ...over });
const start = { lpd: 'engine-synth', op: 'start', patch: '{"patchVersion":1}' };

function host(native = true) {
  const calls: string[] = [];
  let release: (() => void) | null = null, fail: Error | null = null, hold = false;
  const h: EngineHost & { calls: string[]; replies: EngineStatus[]; hold(): void; release(): void; fail(e: Error): void } = {
    native, calls, replies: [],
    start(patch) {
      calls.push('start ' + patch.length);
      if (fail) return Promise.reject(fail);
      return hold ? new Promise<void>(r => { release = r; }) : Promise.resolve();
    },
    frame(on, bend, wheel, gain) { calls.push(`frame ${on} ${bend} ${wheel} ${gain}`); },
    stop() { calls.push('stop'); },
    reply(state) { h.replies.push(state); },
    hold() { hold = true; },
    release() { hold = false; release?.(); },
    fail(e) { fail = e; },
  };
  return h;
}
const clock = () => { let t = 0; const now = () => t; return { now, add(ms: number) { t += ms; } }; };

test('parse: well-formed messages pass, values are clamped', () => {
  assert.deepEqual(parseEngineMessage(start), { op: 'start', patch: start.patch });
  assert.deepEqual(parseEngineMessage({ lpd: 'engine-synth', op: 'stop' }), { op: 'stop' });
  assert.deepEqual(parseEngineMessage(frame({ bend: -3, wheel: 2, gain: -1 })), { op: 'frame', on: true, bend: -1, wheel: 1, gain: 0 });
});

test('parse: malformed messages are dropped', () => {
  for (const bad of [null, 7, 'x', [], { lpd: 'leds' }, { lpd: 'engine-synth' }, { lpd: 'engine-synth', op: 'boom' },
    frame({ on: 1 }), frame({ bend: NaN }), frame({ wheel: Infinity }), frame({ gain: '1' }), frame({ bend: undefined }),
    { ...start, patch: '' }, { ...start, patch: 42 }, { ...start, patch: 'x'.repeat(ENGINE_PATCH_MAX + 1) }]) {
    assert.equal(parseEngineMessage(bad), null, JSON.stringify(bad));
  }
  assert.notEqual(parseEngineMessage({ ...start, patch: 'x'.repeat(ENGINE_PATCH_MAX) }), null);
});

test('rate cap: a burst of 10, then 60 a second; 40 Hz frames always pass', () => {
  const c = clock(), allow = rateCap(60, 10, c.now);
  for (let i = 0; i < 10; i++) assert.equal(allow(), true);
  assert.equal(allow(), false);
  c.add(100);
  let passed = 0;
  for (let i = 0; i < 10; i++) if (allow()) passed++;
  assert.equal(passed, 6);
  const d = clock(), steady = rateCap(60, 10, d.now);
  for (let i = 0; i < 400; i++) { d.add(25); assert.equal(steady(), true); }
});

test('messages from anywhere but the Dustline frame are ignored', async () => {
  const h = host(), link = new EngineLink(h);
  await link.receive(start, false);
  await link.receive(frame(), false);
  assert.deepEqual(h.calls, []);
  assert.deepEqual(h.replies, []);
});

test('start builds the voice, replies ready, then frames pass; frames before ready are dropped', async () => {
  const h = host(), link = new EngineLink(h);
  await link.receive(frame(), true);
  assert.deepEqual(h.calls, [], 'no voice yet');
  await link.receive(start, true);
  assert.deepEqual(h.replies, ['ready']);
  assert.equal(link.active, true);
  await link.receive(frame({ bend: 2 }), true);
  await link.receive(frame({ gain: NaN }), true);
  assert.deepEqual(h.calls, ['start 18', 'frame true 1 0.5 0.8']);
});

test('outside the app the game hears unavailable; a failed start replies error', async () => {
  const h = host(false), link = new EngineLink(h);
  await link.receive(start, true);
  assert.deepEqual(h.replies, ['unavailable']);
  assert.deepEqual(h.calls, []);
  const f = host(), broken = new EngineLink(f);
  f.fail(new Error('no audio output'));
  await broken.receive(start, true);
  assert.deepEqual(f.replies, ['error']);
  await broken.receive(frame(), true);
  assert.equal(broken.active, false);
});

test('auto-stop: leaving Dustline forgets the voice; a hidden window stops it until the next sounding frame', async () => {
  const h = host(), link = new EngineLink(h);
  await link.receive(start, true);
  link.stop(false);
  assert.equal(h.calls.at(-1), 'stop');
  await link.receive(frame({ on: false }), true);
  assert.equal(h.calls.at(-1), 'stop', 'a silent frame does not restart');
  await link.receive(frame(), true);
  assert.deepEqual(h.calls.slice(-1), ['start 18'], 'a sounding frame restarts the voice');
  assert.deepEqual(h.replies, ['ready', 'ready']);
  link.stop(true);
  await link.receive(frame(), true);
  assert.equal(h.calls.at(-1), 'stop', 'after leaving, frames need a new start');
  await link.receive({ lpd: 'engine-synth', op: 'stop' }, true);
  assert.equal(h.calls.filter(c => c === 'stop').length, 2, 'stop is sent once per running voice');
});

test('a stop while the voice is being built stops it as soon as it exists, with no ready reply', async () => {
  const h = host(), link = new EngineLink(h);
  h.hold();
  const pending = link.receive(start, true);
  link.stop(true);
  h.release();
  await pending;
  assert.deepEqual(h.calls, ['start 18', 'stop']);
  assert.deepEqual(h.replies, []);
  assert.equal(link.active, false);
});

test('a flood is capped before it reaches the app', async () => {
  const c = clock(), h = host(), link = new EngineLink(h, c.now);
  await link.receive(start, true);
  for (let i = 0; i < 100; i++) await link.receive(frame(), true);
  assert.equal(h.calls.filter(x => x.startsWith('frame')).length, 9, 'the burst, less the start');
});
