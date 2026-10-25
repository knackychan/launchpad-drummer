import assert from 'node:assert/strict';
import test from 'node:test';
import { SYNTH_META } from './synth-meta.ts';
import { getParam, setParam, meta, fromNorm, toNorm, fmtParam, fmtAmount, arcsFor, arcPath, lcxlTag, categoryColor, sourceColor, throttle, withoutRoute } from './synth.ts';
const init = () => structuredClone(SYNTH_META.init);
test('every synth parameter reads its metadata default', () => {
  const p = init();
  assert.equal(SYNTH_META.params.length, 109);
  for (const m of SYNTH_META.params) assert.equal(getParam(p, m.id), m.default, m.id);
});
test('synth writes nested values, selector indices, booleans and clamped numbers', () => {
  const p = init();
  setParam(p, 'env1.attack', 0.1); assert.equal(p.env[0].attack, 0.1);
  setParam(p, 'fx.reverb.mix', 0.4); assert.equal(p.fx.reverb.mix, 0.4);
  assert.equal(setParam(p, 'filter.type', 9), 4); assert.equal(p.filter.type, 'notch');
  setParam(p, 'oscB.on', 0.7); assert.equal(p.oscB.on, true);
  assert.equal(setParam(p, 'oscA.unison', 0), 1);
  assert.equal(setParam(p, 'filter.cutoff', NaN), 20);
});
test('synth tapers match the engine at known positions', () => {
  assert.ok(Math.abs(fromNorm(meta('filter.cutoff'), 0.5) - 632.456) < 0.001);
  assert.ok(Math.abs(toNorm(meta('filter.cutoff'), 12000) - 0.92605) < 0.0001);
  assert.equal(fromNorm(meta('env1.attack'), 0.5), 1.25);
  assert.equal(fromNorm(meta('lfo1.syncDiv'), 1), 14);
});
test('synth values display their units', () => {
  const cases: [string, number, string][] = [
    ['filter.cutoff',12000,'12.0 kHz'], ['filter.cutoff',632.456,'632 Hz'], ['filter.cutoff',2500,'2.50 kHz'],
    ['lfo1.rate',0.70711,'0.71 Hz'], ['env1.attack',0.002,'2.0 ms'], ['env1.decay',0.5,'500 ms'], ['env1.release',1.25,'1.25 s'],
    ['oscA.fine',-7,'−7 ct'], ['oscA.semi',0,'0 st'], ['voice.volumeDb',-60,'−∞'], ['voice.volumeDb',-6,'−6.0 dB'],
    ['oscA.pan',-0.4,'L20'], ['filter.envAmt',0.45,'+3.6 oct'], ['filter.drive',0.5,'12.0 dB'], ['lfo1.phase',0.25,'90°'],
    ['oscA.oct',-1,'−1'], ['oscA.unison',7,'7'], ['filter.type',0,'LP 24'], ['oscB.on',1,'ON'], ['oscA.pos',0.667,'67 %'],
  ];
  for (const [id,v,want] of cases) assert.equal(fmtParam(id,v),want,id);
});
test('modulation amounts display destination units', () => {
  assert.equal(fmtAmount('osc.pitch',1/60),'+0.40 st');
  assert.equal(fmtAmount('filter.cutoff',0.15),'+1.5 oct');
  assert.equal(fmtAmount('oscA.pos',0.25),'+25 %');
  assert.equal(fmtAmount(null,-0.5),'−50 %');
});
test('knob arcs show the newest three routes with clamped ends', () => {
  const p=init();p.routes=[0.1,0.2,0.3,0.9].map(amount=>({source:'lfo1',dest:'filter.cutoff',amount,aux:null,bypass:false}));
  const {arcs,more}=arcsFor(p,'filter.cutoff',0.5);
  assert.deepEqual(arcs.map(a=>a.to),[1,0.8,0.7]); assert.equal(more,1); assert.deepEqual(arcs.map(a=>a.slot),[3,2,1]);
});
test('knob arc path spans the full 270 degree track', () => {
  assert.equal(arcPath(20,20,10,0,1),'M 12.93 27.07 A 10 10 0 1 1 27.07 27.07');
});
test('synth hardware tags and colours match the registry', () => {
  assert.equal(lcxlTag('filter.cutoff'),'LCXL row 2 · knob 1'); assert.equal(lcxlTag('oscA.pan'),null);
  assert.deepEqual(categoryColor('bass'),[0,30,63]); assert.equal(sourceColor('lfo2'),'#7f8cff');
});
test('removing a route clears its selection and shifts later slots', () => {
  assert.deepEqual(withoutRoute({ a: 0, b: 2, c: 3 }, 2), { a: 0, c: 2 });
});
test('throttle sends the leading call and the final arguments in a burst', t => {
  t.mock.timers.enable({ apis: ['Date', 'setTimeout'], now: 0 });
  const sent: number[] = [], send = throttle((value: number) => sent.push(value), 16);
  send(1); t.mock.timers.tick(1); send(2); t.mock.timers.tick(1); send(3);
  assert.deepEqual(sent, [1]); t.mock.timers.tick(14); assert.deepEqual(sent, [1, 3]);
});
test('keyed throttle retains edits to every route while coalescing the same slot', t => {
  t.mock.timers.enable({ apis: ['Date', 'setTimeout'], now: 1000 });
  const sent: [number, string][] = [];
  const send = throttle((slot: number, value: string) => sent.push([slot, value]), 16, slot => slot);
  send(0, 'A'); t.mock.timers.tick(1); send(1, 'B'); send(2, 'C'); send(1, 'B edited');
  t.mock.timers.tick(15);
  assert.deepEqual(sent, [[0, 'A'], [1, 'B edited'], [2, 'C']]);
});
test('flushing pending route edits before deletion prevents a stale-slot write', t => {
  t.mock.timers.enable({ apis: ['Date', 'setTimeout'], now: 1000 });
  const backend = ['LFO1', 'ENV2'];
  const send = throttle((slot: number, value: string) => { backend[slot] = value; }, 16, slot => slot);
  send(0, 'LFO1'); t.mock.timers.tick(1); send(0, 'LFO1 edited');
  send.flush(); backend.splice(0, 1); t.mock.timers.tick(32);
  assert.deepEqual(backend, ['ENV2']);
});
test('authoritative patch replacement cancels edits queued for the old patch', t => {
  t.mock.timers.enable({ apis: ['Date', 'setTimeout'], now: 1000 });
  const sent: number[] = [], send = throttle((value: number) => sent.push(value), 16);
  send(1); t.mock.timers.tick(1); send(2); send.cancel(); t.mock.timers.tick(32);
  assert.deepEqual(sent, [1]);
});
