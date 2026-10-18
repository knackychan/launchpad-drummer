import { test } from 'node:test';
import assert from 'node:assert/strict';
import { activeNotices, type NoticeInputs } from './notices.ts';

const idle: NoticeInputs = { learnArmed: null, audioLost: false, backup: null, saveFailed: null,
  launchpad: 'connected', lcxl: 'connected', missing: 0, asioFallback: false, dropoutRecent: false,
  confirmations: [], keysAbsent: false, dismissed: new Set() };
test('active notices follow priority, with stable confirmation ordering', () => {
  const input: NoticeInputs = { ...idle, learnArmed: 'cc', audioLost: true, backup: { savedAt: '14:32' },
    saveFailed: 'disk full', launchpad: 'bootloader', lcxl: 'busy', missing: 3, asioFallback: true,
    dropoutRecent: true, confirmations: [{ id: 'a', text: 'A' }, { id: 'b', text: 'B' }], keysAbsent: true };
  assert.deepEqual(activeNotices(input).map(n => n.id), ['learn', 'audio-lost', 'backup', 'save-failed',
    'lp-bootloader', 'missing', 'asio', 'dropout', 'lcxl-busy', 'ok:a', 'ok:b', 'keys-off']);
  assert.equal(activeNotices({ ...idle, launchpad: 'busy' })[0].prio, 6);
  assert.deepEqual(activeNotices({ ...idle, launchpad: 'disconnected', lcxl: 'disconnected' }).map(n => n.prio), [12, 13]);
  assert.ok(!activeNotices({ ...input, dismissed: new Set(['backup']) }).some(n => n.id === 'backup'));
});
test('notice text handles missing counts and backup times', () => {
  assert.equal(activeNotices({ ...idle, missing: 1 })[0].text, '1 sample missing');
  assert.equal(activeNotices({ ...idle, missing: 3 })[0].text, '3 samples missing');
  assert.equal(activeNotices({ ...idle, backup: { savedAt: '14:32' } })[0].text, 'set.json unreadable — loaded backup (saved 14:32)');
  assert.equal(activeNotices({ ...idle, backup: { savedAt: null } })[0].text, 'set.json unreadable — loaded backup (saved earlier)');
  assert.deepEqual(activeNotices(idle), []);
});
