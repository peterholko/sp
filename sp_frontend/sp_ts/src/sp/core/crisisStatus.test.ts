import assert from 'node:assert/strict';
import {
  CrisisStatusPacket,
  CrisisUiState,
  clearCrisisStatus,
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
    phase: 'dormant',
    warning: false,
    assault_ready: false,
    assault_active: false,
    resolved: false,
    continues_while_disconnected: false,
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

const ready = crisisStatusView(status({
  phase: 'assault_ready',
  assault_ready: true,
  preparation_seconds_remaining: 65,
}));
assert.equal(ready?.phaseLabel, 'Raid Imminent');
assert.equal(ready?.preparationLabel, '1m 05s');
assert.equal(formatCrisisCountdown(0), 'complete');

const active = crisisStatusView(status({
  phase: 'assault_active',
  assault_active: true,
  remaining_attackers: 2,
  total_attackers: 3,
  continues_while_disconnected: true,
}));
assert.equal(active?.attackersLabel, '2 / 3');
assert.equal(active?.disconnectedWarning, 'The assault continues while disconnected.');

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

const reset = clearCrisisStatus({ ...enteringReady, compactExpanded: false });
assert.equal(reset.crisisStatus, null);
assert.equal(reset.previousCrisisPhase, null);
assert.equal(reset.compactExpanded, false, 'run reset preserves the player collapse preference');

console.log('crisisStatus helper checks passed');
