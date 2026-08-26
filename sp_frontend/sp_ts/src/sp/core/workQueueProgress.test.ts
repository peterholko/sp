import assert from 'node:assert/strict';
import {
  reconcileWorkQueueProgress,
  workQueueProgressFraction,
} from './workQueueProgress';

const progress = reconcileWorkQueueProgress(null, {
  action_id: 41,
  action_duration_ms: 15_000,
  action_elapsed_ms: 4_000,
}, 10_000);

assert.deepEqual(progress, {
  actionId: 41,
  durationMs: 15_000,
  elapsedMs: 4_000,
  startTimeMs: 6_000,
});
assert.equal(workQueueProgressFraction(progress, 13_500), 0.5);

const reconciled = reconcileWorkQueueProgress(progress, {
  action_id: 41,
  action_duration_ms: 15_000,
  action_elapsed_ms: 5_000,
}, 13_000);

assert.equal(reconciled?.elapsedMs, 7_000, 'same-action corrections never move backward');
assert.equal(reconciled?.startTimeMs, 6_000);
assert.ok(Math.abs(workQueueProgressFraction(reconciled, 13_000) - (7 / 15)) < 0.0001);

const next = reconcileWorkQueueProgress(reconciled, {
  action_id: 42,
  action_duration_ms: 15_000,
  action_elapsed_ms: 0,
}, 21_000);

assert.equal(next?.actionId, 42, 'a new server action starts a new cycle');
assert.equal(next?.startTimeMs, 21_000);
assert.equal(workQueueProgressFraction(next, 21_000), 0);

const complete = reconcileWorkQueueProgress(null, {
  action_id: 43,
  action_duration_ms: 15_000,
  action_elapsed_ms: 15_000,
}, 20_000);

assert.equal(workQueueProgressFraction(complete, 30_000), 1);
assert.equal(reconcileWorkQueueProgress(null, {}, 20_000), null);

console.log('Authoritative work queue progress checks passed');
