import assert from 'node:assert/strict';
import { anchorActionProgress, requiresAuthoritativeActionProgress } from './actionProgress';
import { refiningProgressForItem } from './refiningProgress';

const hero = {
  id: '7',
  state: 'refining',
  action_id: 114,
  action_duration_ms: 8000,
  action_elapsed_ms: 2750,
} as any;

const snapshot = refiningProgressForItem(hero, 42, 42);
assert.deepEqual(snapshot, {
  action_id: 114,
  action_duration_ms: 8000,
  action_elapsed_ms: 2750,
});
assert.deepEqual(anchorActionProgress(snapshot!, 10000), {
  actionId: 114,
  durationMs: 8000,
  elapsedMs: 2750,
  startTimeMs: 7250,
});

assert.equal(refiningProgressForItem(hero, 43, 42), null);
assert.equal(refiningProgressForItem(hero, 42, null), null);
assert.equal(refiningProgressForItem({ ...hero, state: 'idle' }, 42, 42), null);
assert.equal(refiningProgressForItem({ ...hero, action_elapsed_ms: undefined }, 42, 42), null);
assert.equal(requiresAuthoritativeActionProgress('refining'), true);

console.log('Item-panel refining progress checks passed');
