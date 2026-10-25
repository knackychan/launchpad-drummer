import assert from 'node:assert/strict';
import test from 'node:test';
import { freqToX, xToFreq, dbToY, envPoints, envDotX, lfoValue, lfoPoints } from './synthDraw.ts';
import { pushSynthFrame, synthFeed, synthLive } from './synthFeed.ts';
import { SYNTH_META } from './synth-meta.ts';
import type { SynthTelemetry } from './synth-types.ts';
const e = { attack: 0.1, decay: 0.2, sustain: 0.5, release: 0.3 };
test('filter axes map log frequency and decibels', () => {
  assert.equal(freqToX(20,100),0);assert.equal(freqToX(20000,100),100);
  assert.ok(Math.abs(xToFreq(50,100)-632.46)<0.01);
  assert.equal(dbToY(24,60),0);assert.equal(dbToY(-36,60),60);assert.equal(dbToY(0,60),24);
});
test('envelope display uses engine attack, decay, hold and release curves', () => {
  const points=envPoints(e,101);assert.deepEqual(points[0],[0,0]);
  assert.ok(Math.abs(Math.max(...points.map(p=>p[1]))-1)<0.02);
  assert.equal(points[50][1],0.5);assert.ok(points.at(-1)![1]<=0.0050001);
});
test('envelope dots map stages to the drawn time model', () => {
  assert.ok(Math.abs(envDotX(3,0.5,e)!-0.5)<1e-12);assert.ok(Math.abs(envDotX(1,1,e)!-0.1333)<0.002);assert.equal(envDotX(0,0,e),null);
});
test('LFO display follows shape, phase and polarity', () => {
  assert.equal(lfoValue('triangle',0.25),1);assert.equal(lfoValue('sawDown',0),1);assert.equal(lfoValue('square',0.75),-1);
  const l={...SYNTH_META.init.lfo[0],shape:'sine' as const,phase:0.25};assert.equal(lfoPoints(l,4)[0],1);
  assert.ok(lfoPoints({...l,polarity:'uni'},96).every(v=>v>=0&&v<=1));
});
test('synth spectrum rises immediately, decays by twelve percent and expires', () => {
  const frame: SynthTelemetry={scope:[],spectrum:Array(128).fill(0),curve:[],sources:[],live:[],env:[],lfo:[],cutoffHz:1000,voices:1,cpu:0,resets:0};
  synthFeed.spectrum.fill(0);pushSynthFrame(frame,1000);assert.equal(synthFeed.spectrum[0],1);
  pushSynthFrame({...frame,spectrum:Array(128).fill(-100)},1016);assert.ok(Math.abs(synthFeed.spectrum[0]-0.88)<0.000001);
  assert.equal(synthLive(1515),true);assert.equal(synthLive(1516),false);
});
