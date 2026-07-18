import assert from 'node:assert/strict';
import {
  CrisisAssaultIntent,
  CrisisPreparationOption,
  CrisisStatusPacket,
  CrisisUiState,
  clearCrisisStatus,
  crisisAssaultIntentsView,
  crisisPreparationOptionsView,
  formatCrisisCountdown,
  crisisPressureView,
  crisisStatusView,
  receiveCrisisStatus,
  shouldRenderSurvivalThread,
} from './crisisStatus';

function status(overrides: Partial<CrisisStatusPacket> = {}): CrisisStatusPacket {
  return {
    packet: 'crisis_status',
    version: 1,
    exists: true,
    kind: 'goblin',
    phase: 'dormant',
    warning: false,
    assault_ready: false,
    assault_active: false,
    resolved: false,
    continues_while_disconnected: false,
    ...overrides,
  };
}

function preparationOption(
  overrides: Partial<CrisisPreparationOption> = {},
): CrisisPreparationOption {
  return {
    id: 'defences',
    label: 'Defences',
    state: 'needs_attention',
    detail: 'Two defensive structures are damaged.',
    action_hint: 'Repair walls before the raid begins.',
    ...overrides,
  };
}

function assaultIntent(
  overrides: Partial<CrisisAssaultIntent> = {},
): CrisisAssaultIntent {
  return {
    role: 'hunter',
    label: 'Hunter Rider',
    intent: 'Pressuring your hero.',
    ...overrides,
  };
}

const initialUiState: CrisisUiState = {
  crisisStatus: null,
  previousCrisisPhase: null,
  compactExpanded: false,
};

assert.equal(crisisStatusView(null), null, 'no crisis renders no crisis card');
assert.equal(crisisStatusView(status({ phase: 'dormant' }))?.tone, 'neutral');

const preparing = crisisStatusView(status({
  phase: 'preparing',
  warning: true,
  title: 'Raiders Gathering',
}));
assert.equal(preparing?.warning, true, 'preparing exposes its warning');
assert.equal(preparing?.phaseLabel, 'Preparing');
assert.equal(preparing?.compactLabel, 'Raiders Gathering');
assert.equal(preparing?.statusAriaLabel, 'Personal goblin crisis status');
assert.equal(preparing?.pressureLabel, 'Goblin crisis pressure');
assert.deepEqual(preparing?.preparationOptions, [], 'missing additive rows remain compatible');

const omittedV1Kind = crisisStatusView(status({
  kind: undefined,
  phase: 'preparing',
}));
assert.equal(omittedV1Kind?.compactLabel, 'Raiders Gathering');
assert.equal(omittedV1Kind?.statusAriaLabel, 'Personal goblin crisis status');
assert.equal(omittedV1Kind?.pressureLabel, 'Goblin crisis pressure');

const preparationRows = [
  preparationOption(),
  preparationOption({
    id: 'defenders',
    label: 'Defenders',
    state: 'ready',
    detail: 'One combat-capable villager is available.',
    action_hint: 'Keep the defender near the settlement.',
  }),
  preparationOption({
    id: 'equipment',
    label: 'Equipment',
    state: 'unavailable',
    detail: 'No carried armor can be equipped.',
    action_hint: '',
  }),
  preparationOption({
    id: 'recovery',
    label: 'Recovery',
    state: 'needs_attention',
    detail: 'Healing supplies are stored away from the hero.',
    action_hint: 'Carry one healing item before the raid.',
  }),
  preparationOption({ id: 'fifth', label: 'Fifth option' }),
];
const preparedView = crisisStatusView(status({
  phase: 'preparing',
  preparation_options: preparationRows,
}));
assert.equal(preparedView?.preparationOptions.length, 4, 'preparation rows are capped at four');
assert.deepEqual(
  preparedView?.preparationOptions.map((option) => option.stateLabel),
  ['Needs attention', 'Ready', 'Unavailable', 'Needs attention'],
  'every stable state has literal text',
);
assert.equal(preparedView?.preparationOptions[0].id, 'defences');
assert.equal(preparedView?.preparationOptions[0].actionHint, 'Repair walls before the raid begins.');

assert.deepEqual(
  crisisPreparationOptionsView('preparing', [
    preparationOption({ id: 'duplicate' }),
    preparationOption({ id: 'duplicate', label: 'Duplicate' }),
    { ...preparationOption({ id: 'future' }), state: 'future_state' },
    null,
    preparationOption({ id: 'outside-bound', label: 'Outside bound' }),
  ]),
  [
    {
      id: 'duplicate',
      label: 'Defences',
      state: 'needs_attention',
      stateLabel: 'Needs attention',
      detail: 'Two defensive structures are damaged.',
      actionHint: 'Repair walls before the raid begins.',
    },
  ],
  'duplicate, unknown, malformed, and out-of-bound rows fail safely',
);
assert.deepEqual(
  crisisPreparationOptionsView('preparing', { malformed: true }),
  [],
  'a malformed optional field does not crash',
);

const ready = crisisStatusView(status({
  phase: 'assault_ready',
  assault_ready: true,
  preparation_seconds_remaining: 65,
}));
assert.equal(ready?.phaseLabel, 'Raid Imminent');
assert.equal(ready?.compactLabel, 'Raid Imminent');
assert.equal(ready?.preparationLabel, '1m 05s');
assert.equal(formatCrisisCountdown(0), 'complete');

const undeadPreparing = crisisStatusView(status({
  kind: 'undead',
  phase: 'preparing',
  title: 'Undead Gathering',
}));
assert.equal(undeadPreparing?.phaseLabel, 'Preparing');
assert.equal(undeadPreparing?.compactLabel, 'Undead Gathering');
assert.equal(undeadPreparing?.statusAriaLabel, 'Personal undead crisis status');
assert.equal(undeadPreparing?.pressureLabel, 'Undead crisis pressure');

const undeadReady = crisisStatusView(status({
  kind: 'undead',
  phase: 'assault_ready',
  assault_ready: true,
  title: 'Undead Incursion Imminent',
}));
assert.equal(undeadReady?.phaseLabel, 'Incursion Imminent');
assert.equal(undeadReady?.compactLabel, 'Undead Incursion Imminent');
assert.equal(undeadReady?.preparationLabel, null);

const active = crisisStatusView(status({
  phase: 'assault_active',
  assault_active: true,
  remaining_attackers: 2,
  total_attackers: 3,
  assault_intents: [
    assaultIntent(),
    assaultIntent({
      role: 'breacher',
      label: 'Breacher Rider',
      intent: 'Breaking settlement walls.',
    }),
    assaultIntent({
      role: 'pillager',
      label: 'Goblin Pillager',
      intent: 'Raiding completed structures.',
    }),
  ],
  continues_while_disconnected: true,
}));
assert.equal(active?.attackersLabel, '2 / 3');
assert.deepEqual(active?.assaultIntents, [
  { role: 'hunter', label: 'Hunter Rider', intent: 'Pressuring your hero.' },
  { role: 'breacher', label: 'Breacher Rider', intent: 'Breaking settlement walls.' },
  { role: 'pillager', label: 'Goblin Pillager', intent: 'Raiding completed structures.' },
]);
assert.equal(active?.disconnectedWarning, 'The assault continues while disconnected.');

assert.deepEqual(
  crisisAssaultIntentsView('assault_active', [
    assaultIntent(),
    assaultIntent({ role: 'HUNTER', label: 'Duplicate role' }),
    assaultIntent({ role: 'breacher', label: 'Hunter Rider' }),
    assaultIntent({ role: 'outside-bound', label: 'Outside bound' }),
  ]),
  [{ role: 'hunter', label: 'Hunter Rider', intent: 'Pressuring your hero.' }],
  'duplicate role/label and out-of-bound rows fail safely',
);
assert.deepEqual(
  crisisAssaultIntentsView('assault_active', [
    { role: '', label: 'Missing role', intent: 'Ignored.' },
    { role: 'missing-label', label: ' ', intent: 'Ignored.' },
    { role: 'missing-intent', label: 'Missing intent', intent: null },
  ]),
  [],
  'malformed assault-intent rows fail safely',
);
assert.deepEqual(
  crisisAssaultIntentsView('preparing', [assaultIntent()]),
  [],
  'assault intents are visible only during the active phase',
);
assert.deepEqual(
  crisisStatusView(status({
    phase: 'assault_active',
    assault_active: true,
    preparation_options: preparationRows,
  }))?.preparationOptions,
  [],
  'active combat hides stale preparation guidance',
);

assert.equal(
  crisisStatusView(status({
    phase: 'assault_ready',
    preparation_options: [preparationOption()],
  }))?.preparationOptions.length,
  1,
  'AssaultReady retains preparation guidance',
);
assert.deepEqual(
  crisisStatusView(status({
    phase: 'resolved',
    resolved: true,
    preparation_options: preparationRows,
  }))?.preparationOptions,
  [],
  'resolved crises hide preparation guidance',
);
assert.deepEqual(
  crisisStatusView(status({
    phase: 'resolved',
    resolved: true,
    assault_intents: [assaultIntent()],
  }))?.assaultIntents,
  [],
  'resolved crises hide stale raid intents',
);

assert.equal(
  crisisStatusView(status({ phase: 'resolved', resolved: true }))?.tone,
  'resolved',
);
assert.deepEqual(crisisPressureView(130, 120), { value: 120, max: 120, percent: 100 });
assert.deepEqual(crisisPressureView(-4, 120), { value: 0, max: 120, percent: 0 });
assert.equal(crisisPressureView(undefined, undefined), null);

const missingOptionalFields = crisisStatusView(status({ phase: undefined }));
assert.equal(missingOptionalFields?.phaseLabel, 'Unknown Status');
assert.equal(missingOptionalFields?.summary, '');

const cleared = receiveCrisisStatus(
  { ...initialUiState, crisisStatus: status(), previousCrisisPhase: 'dormant' },
  status({ exists: false }),
);
assert.equal(cleared.crisisStatus, null, 'exists:false clears previous state');
assert.equal(cleared.previousCrisisPhase, null);

const firstPreparing = receiveCrisisStatus(initialUiState, status({ phase: 'preparing' }));
assert.equal(firstPreparing.compactExpanded, true, 'entering preparing auto-expands');

const manuallyCollapsed: CrisisUiState = { ...firstPreparing, compactExpanded: false };
const repeatedPreparing = receiveCrisisStatus(manuallyCollapsed, status({ phase: 'preparing' }));
assert.equal(repeatedPreparing.compactExpanded, false, 'same phase respects manual collapse');

const enteringReady = receiveCrisisStatus(repeatedPreparing, status({ phase: 'assault_ready' }));
assert.equal(enteringReady.compactExpanded, true, 'a new urgent phase expands once');

assert.equal(shouldRenderSurvivalThread(false, status()), true, 'crisis renders without an objective');
assert.equal(shouldRenderSurvivalThread(false, status({ exists: false })), false);

const futurePhase = crisisStatusView(status({ phase: 'future_crisis_phase', future_value: 42 }));
assert.equal(futurePhase?.tone, 'neutral', 'unknown future phases fail safely');
assert.equal(futurePhase?.phaseLabel, 'Unknown Status');

const futureKind = crisisStatusView(status({ kind: 'future_crisis', phase: 'assault_ready' }));
assert.equal(futureKind?.phaseLabel, 'Crisis Imminent');
assert.equal(futureKind?.compactLabel, 'Crisis Imminent');
assert.equal(futureKind?.statusAriaLabel, 'Personal crisis status');
assert.equal(futureKind?.pressureLabel, 'Personal crisis pressure');

const reset = clearCrisisStatus({ ...enteringReady, compactExpanded: false });
assert.equal(reset.crisisStatus, null);
assert.equal(reset.previousCrisisPhase, null);
assert.equal(reset.compactExpanded, false, 'run reset preserves the player collapse preference');

console.log('crisisStatus helper checks passed');
