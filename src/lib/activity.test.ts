import assert from 'node:assert/strict';
import test from 'node:test';
import { decay, energy, hitGroup, isResting, onActivity, REST, runActivity } from './activity.ts';

// A scheduler we drive by hand, standing in for the browser.
const queue = new Map<number, (t: number) => void>();
let nextId = 1;
let queries = 0;
Object.assign(globalThis, {
  requestAnimationFrame: (cb: (t: number) => void) => { queue.set(nextId, cb); return nextId++; },
  cancelAnimationFrame: (id: number) => { queue.delete(id); },
  document: { querySelectorAll: () => { queries++; return []; } },
});

function runFrames(stepMs: number, max = 1000): number {
  let t = performance.now();
  let frames = 0;
  while (queue.size && frames < max) {
    const [id, cb] = [...queue][0];
    queue.delete(id);
    t += stepMs;
    cb(t);
    frames++;
  }
  return frames;
}

test('decay halves every 110 ms and snaps to exactly zero below the rest level', () => {
  energy.fill(0);
  energy[0] = 1;
  decay(55); // half of a half-life
  assert.ok(Math.abs(energy[0] - Math.SQRT1_2) < 1e-6);
  energy[1] = REST * 1.5;
  decay(100);
  assert.equal(energy[1], 0);
  energy.fill(0);
});

test('the loop sleeps at rest, wakes on a hit and sleeps again', () => {
  energy.fill(0);
  const stop = runActivity();
  let notified = 0;
  const unsubscribe = onActivity(() => { notified++; });
  runFrames(16);
  assert.equal(queue.size, 0, 'nothing is scheduled while idle');

  const before = queries;
  hitGroup(2, 1);
  assert.equal(queue.size, 1, 'a hit wakes the loop');
  const frames = runFrames(16);
  assert.ok(frames > 5 && frames < 120, `decays over a bounded number of frames (${frames})`);
  assert.ok(isResting());
  assert.equal(queue.size, 0, 'asleep again');
  assert.ok(notified >= frames, 'listeners saw every step, including the resting one');
  assert.ok(queries > before);

  // Idle for a long time: no work at all.
  const idleQueries = queries;
  const idleNotified = notified;
  runFrames(16);
  assert.equal(queries, idleQueries);
  assert.equal(notified, idleNotified);

  unsubscribe();
  stop();
});

test('hits are ignored for unknown channels and clamped otherwise', () => {
  energy.fill(0);
  hitGroup(9, 1);
  hitGroup(-1, 1);
  assert.ok(isResting());
  hitGroup(3, 5);
  assert.equal(energy[3], 1);
  energy.fill(0);
  queue.clear();
});
