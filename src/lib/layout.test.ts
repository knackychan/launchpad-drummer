import assert from 'node:assert/strict';
import test from 'node:test';
import { solvePlayLayout, synthPageSet, synthKnob } from './layout.ts';
const base = { drawerOpen: true, drawerPinned: true, teleFolded: false, lanes: 5 };
const solve = (width: number, height: number, extra = {}) => solvePlayLayout({ ...base, width, height, ...extra });
test('2560x1440 with the drawer floating keeps 94 px pads and gives the synth one page', () => {
 const l=solve(2455,1296,{drawerPinned:false}); assert.deepEqual([l.pad,l.tw,l.synthW,l.mixMode,l.docked],[94,300,854,'slim',false]);
});
test('2560x1440 with the drawer docked', () => {
 const l=solve(2455,1296); assert.deepEqual([l.docked,l.tw,l.pad,l.synthW],[true,220,77,760]);
});
test('1920x1080: a docked drawer floats so the synth keeps its room', () => {
 const l=solve(1816,988); assert.deepEqual([l.docked,l.pad,l.tw,l.synthW],[false,56,220,599]);
});
test('1280x720: telemetry and mixer collapse', () => {
 const l=solve(1176,628); assert.deepEqual([l.tw,l.mini,l.mixMode,l.pad,l.synthW],[30,true,'strip',37,561]);
});
test('the keybed and sequencer follow the height alone', () => {
 for(const drawerPinned of [false,true]) {const l=solve(2455,1296,{drawerPinned});assert.deepEqual([l.kh,l.sq,l.rowH],[85,293,874]);}
 const l=solve(1176,628);assert.deepEqual([l.kh,l.sq],[44,122]);
});
test('folding the sequencer gives its height to the row', () => {
 const l=solve(1176,628,{seqFolded:true});assert.deepEqual([l.pH,l.rowH,l.sq,l.pad],[48,506,28,47]);
});
test('folding both plates', () => {const l=solve(1176,628,{seqFolded:true,keysFolded:true});assert.deepEqual([l.pH,l.kh,l.sq],[52,28,28]);});
test('pads never exceed 112 px', () => {for(const folded of [false,true]) assert.equal(solve(5000,3000,{keysFolded:folded,seqFolded:folded}).pad,112);});
test('above the minimum window the synth never drops under 480 px', () => {
 for(let width=1176;width<=2600;width+=50) for(const height of [628,988]) for(const drawerPinned of [false,true]) assert.ok(solve(width,height,{drawerPinned}).synthW>=480,`${width}×${height}`);
});
test('below the minimum window the synth may be narrower (K17)', () => {assert.equal(solve(900,628).synthW,293);});
test('page sets and knob sizes', () => {
 assert.deepEqual([[854,874],[760,874],[599,666],[561,418]].map(([w,h])=>synthPageSet(w,h)),['wide','wide','narrow','compact']);
 assert.deepEqual([synthKnob(874,'wide'),synthKnob(418,'compact'),synthKnob(300,'wide')],[44,44,24]);
});
