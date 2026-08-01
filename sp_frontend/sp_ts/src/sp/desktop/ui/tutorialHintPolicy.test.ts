import assert from 'node:assert/strict';
import {
  TUTORIAL_FIRST_HINT_DELAY_MS,
  TUTORIAL_HINT_VISIBLE_MS,
  TUTORIAL_OBJECTIVE_STALE_MS,
  TUTORIAL_REPEAT_HINT_DELAY_MS,
  advanceTutorialHintPolicy,
  createTutorialHintPolicy,
  dismissTutorialHint,
  receiveTutorialObjective,
  resetTutorialHintPolicy,
  setTutorialHintEnabled,
  tutorialHintVisible,
} from './tutorialHintPolicy';

function packet(overrides: Record<string, unknown> = {}) {
  const objective = {
    id: 'scavenge_shipwreck',
    title: 'Search the Shipwreck',
    state: 'active',
    action_hint: 'Move next to the Shipwreck and choose Investigate.',
    blocker: undefined,
    progress: undefined,
    goal: undefined,
    ...(overrides.objective as Record<string, unknown> || {}),
  };
  return {
    packet: 'objective_state',
    current_id: objective.id,
    objectives: [objective],
    ...overrides,
  };
}

let policy = createTutorialHintPolicy(true, 0);
policy = receiveTutorialObjective(policy, packet(), 0);
policy = advanceTutorialHintPolicy(policy, 0, false);

// Duplicate five-second snapshots keep the packet fresh without postponing the
// first hint.
for (let now = 5_000; now <= 55_000; now += 5_000) {
  policy = receiveTutorialObjective(policy, packet(), now);
  policy = advanceTutorialHintPolicy(policy, now, false);
}
assert.equal(tutorialHintVisible(policy, TUTORIAL_FIRST_HINT_DELAY_MS - 1), false);
policy = receiveTutorialObjective(policy, packet(), TUTORIAL_FIRST_HINT_DELAY_MS);
policy = advanceTutorialHintPolicy(policy, TUTORIAL_FIRST_HINT_DELAY_MS, false);
assert.equal(tutorialHintVisible(policy, TUTORIAL_FIRST_HINT_DELAY_MS), true);
assert.equal(policy.objective?.title, 'Search the Shipwreck');
assert.equal(
  tutorialHintVisible(policy, TUTORIAL_FIRST_HINT_DELAY_MS + TUTORIAL_HINT_VISIBLE_MS),
  false,
  'the callout has a bounded visible lifetime',
);

// Repeated hints use the longer cadence, measured from when the prior hint was
// displayed. Continuing packets do not alter it.
const repeatDue = TUTORIAL_FIRST_HINT_DELAY_MS + TUTORIAL_REPEAT_HINT_DELAY_MS;
policy = receiveTutorialObjective(policy, packet(), repeatDue - 1);
policy = advanceTutorialHintPolicy(policy, repeatDue - 1, false);
assert.equal(tutorialHintVisible(policy, repeatDue - 1), false);
policy = receiveTutorialObjective(policy, packet(), repeatDue);
policy = advanceTutorialHintPolicy(policy, repeatDue, false);
assert.equal(tutorialHintVisible(policy, repeatDue), true);

policy = dismissTutorialHint(policy, repeatDue + 1_000);
assert.equal(tutorialHintVisible(policy, repeatDue + 1_000), false);
policy = receiveTutorialObjective(
  policy,
  packet(),
  repeatDue + 1_000 + TUTORIAL_REPEAT_HINT_DELAY_MS - 1,
);
policy = advanceTutorialHintPolicy(
  policy,
  repeatDue + 1_000 + TUTORIAL_REPEAT_HINT_DELAY_MS - 1,
  false,
);
assert.equal(policy.visibleUntil, null, 'dismissal restarts the repeat interval');

// Numerical progress, a changed blocker, and an objective transition each
// establish a new grace period.
let progressPolicy = createTutorialHintPolicy(true, 0);
progressPolicy = receiveTutorialObjective(
  progressPolicy,
  packet({ objective: { progress: 0, goal: 3 } }),
  0,
);
progressPolicy = advanceTutorialHintPolicy(progressPolicy, 0, false);
progressPolicy = receiveTutorialObjective(
  progressPolicy,
  packet({ objective: { progress: 1, goal: 3 } }),
  59_000,
);
assert.equal(progressPolicy.progressAt, 59_000);
progressPolicy = receiveTutorialObjective(
  progressPolicy,
  packet({ objective: { progress: 1, goal: 3, blocker: 'Finish the current action.' } }),
  60_000,
);
assert.equal(progressPolicy.progressAt, 60_000);
progressPolicy = receiveTutorialObjective(
  progressPolicy,
  packet({
    current_id: 'build_burrow',
    objective: {
      id: 'build_burrow',
      title: 'Build your Burrow',
      action_hint: 'Place and build the Burrow.',
      progress: 0,
      goal: 1,
    },
  }),
  61_000,
);
assert.equal(progressPolicy.progressAt, 61_000);
assert.equal(progressPolicy.objective?.id, 'build_burrow');

// Stale/disconnected objective data clears a hint. A fresh packet resumes with
// a full grace period instead of catching up immediately.
let pausedPolicy = createTutorialHintPolicy(true, 0);
pausedPolicy = receiveTutorialObjective(pausedPolicy, packet(), 0);
pausedPolicy = advanceTutorialHintPolicy(pausedPolicy, 0, false);
pausedPolicy = advanceTutorialHintPolicy(
  pausedPolicy,
  TUTORIAL_OBJECTIVE_STALE_MS + 1,
  false,
);
assert.equal(pausedPolicy.eligible, false);
pausedPolicy = receiveTutorialObjective(pausedPolicy, packet(), 50_000);
pausedPolicy = advanceTutorialHintPolicy(pausedPolicy, 50_000, false);
assert.equal(pausedPolicy.progressAt, 50_000);
pausedPolicy = advanceTutorialHintPolicy(pausedPolicy, 60_000, true);
assert.equal(pausedPolicy.eligible, false, 'combat/death/crisis/visibility pauses share one gate');
pausedPolicy = receiveTutorialObjective(pausedPolicy, packet(), 70_000);
pausedPolicy = advanceTutorialHintPolicy(pausedPolicy, 70_000, false);
assert.equal(pausedPolicy.progressAt, 70_000, 'resuming starts a fresh grace period');

// Turning the tutorial off always clears hints. Re-enabling preserves the
// authoritative objective but starts a new first-hint grace period.
let toggledPolicy = createTutorialHintPolicy(true, 0);
toggledPolicy = receiveTutorialObjective(toggledPolicy, packet(), 0);
toggledPolicy = advanceTutorialHintPolicy(toggledPolicy, 0, false);
toggledPolicy = setTutorialHintEnabled(toggledPolicy, false, 30_000);
toggledPolicy = advanceTutorialHintPolicy(toggledPolicy, 30_000, false);
assert.equal(toggledPolicy.enabled, false);
assert.equal(tutorialHintVisible(toggledPolicy, 30_000), false);
toggledPolicy = receiveTutorialObjective(toggledPolicy, packet(), 31_000);
toggledPolicy = setTutorialHintEnabled(toggledPolicy, true, 31_000);
toggledPolicy = advanceTutorialHintPolicy(toggledPolicy, 31_000, false);
assert.equal(toggledPolicy.progressAt, 31_000);
assert.equal(toggledPolicy.eligible, true);

toggledPolicy = receiveTutorialObjective(
  toggledPolicy,
  { packet: 'objective_state', current_id: 'complete', objectives: [] },
  40_000,
);
assert.equal(toggledPolicy.objective, null, 'completed threads cannot emit hints');
toggledPolicy = resetTutorialHintPolicy(toggledPolicy, 41_000);
assert.equal(toggledPolicy.lastPacketAt, null, 'run/hero reset drops the prior snapshot');

console.log('Tutorial hint policy checks passed');
