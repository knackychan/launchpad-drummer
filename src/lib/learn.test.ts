import assert from 'node:assert/strict';
import test from 'node:test';
import { bindingLabel, boundIds, noteBoundTo, paramLabel, targetLabel } from './learn.ts';
import { createPreviewSet } from './model.ts';
import type { Binding } from './model.ts';

const set = createPreviewSet();
// The preview names the channels KICK, SNARE, HATS, TOMS, ...

test('paramLabel: groups, master and banks', () => {
  assert.equal(paramLabel('group.3.volume', set), '4 TOMS · VOLUME');
  assert.equal(paramLabel('group.0.lowpass', set), '1 KICK · LPF');
  assert.equal(paramLabel('group.1.mute', set), '2 SNARE · MUTE');
  assert.equal(paramLabel('master', set), 'MASTER');
  assert.equal(paramLabel('bank.1.humanize', set), 'BANK 2 · HUMANIZE');
  assert.equal(paramLabel('bank.0.playState', set), 'BANK 1 · PLAY');
  assert.equal(paramLabel('keys.volume', set), 'KEYS · VOLUME');
  assert.equal(paramLabel('seq.bpm', set), 'SEQ · BPM');
  assert.equal(paramLabel('stopAll', set), 'STOP ALL');
});

test('paramLabel: a default channel name reads GROUP n', () => {
  const plain = { ...set, mixer: { ...set.mixer, groups: set.mixer.groups.map((g, i) => ({ ...g, name: `Group ${i + 1}` })) } };
  assert.equal(paramLabel('group.2.pan', plain), 'GROUP 3 · PAN');
});

test('targetLabel names the pad by bank, position and sample', () => {
  const pad = set.banks[0].pads.find(p => p.sample.endsWith('kick_808.wav'))!;
  assert.equal(targetLabel({ pad: { bank: 0, x: pad.x, y: pad.y } }, set), `B1 R${pad.y + 1}·C${pad.x + 1} · KICK 808`);
  assert.equal(targetLabel({ pad: { bank: 3, x: 0, y: 0 } }, set), 'B4 R1·C1 · EMPTY');
  assert.equal(targetLabel({ param: 'master' }, set), 'MASTER');
});

test('bindingLabel for a CC binding and a pad binding', () => {
  const cc: Binding = { device: 'Keystation 49', kind: 'cc', number: 21, target: { param: 'group.3.volume' } };
  assert.equal(bindingLabel(cc, set), 'Keystation 49 · CC 21 → 4 TOMS · VOLUME');
  const note: Binding = { device: 'Keystation 49', kind: 'note', number: 36, target: { pad: { bank: 3, x: 0, y: 0 } } };
  assert.equal(bindingLabel(note, set), 'Keystation 49 · NOTE 36 → B4 R1·C1 · EMPTY');
});

test('boundIds lists params and pads, noteBoundTo finds a pad by note on any device', () => {
  const bound = {
    ...set,
    learn: [
      { device: 'A', kind: 'cc', number: 1, target: { param: 'master' } },
      { device: 'B', kind: 'note', number: 40, target: { pad: { bank: 1, x: 2, y: 3 } } },
      { device: 'B', kind: 'cc', number: 40, target: { param: 'group.0.pan' } },
    ] as Binding[],
  };
  assert.deepEqual([...boundIds(bound)].sort(), ['group.0.pan', 'master', 'pad:1:2:3']);
  assert.deepEqual(noteBoundTo(bound, 40), { bank: 1, x: 2, y: 3 });
  assert.equal(noteBoundTo(bound, 41), null);
  assert.equal(noteBoundTo(set, 40), null);
});
