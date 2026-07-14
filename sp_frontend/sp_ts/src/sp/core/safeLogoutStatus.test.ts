import assert from 'node:assert/strict';
import { NetworkEvent } from './networkEvent';
import {
  SAFE_LOGOUT_ACTIVE_ASSAULT_WARNING,
  SAFE_LOGOUT_COMPLETION_MESSAGE,
  SAFE_LOGOUT_CONDITIONS,
  SafeLogoutCloseGuard,
  SafeLogoutStatusPacket,
  SafeLogoutUiState,
  beginSafeLogoutCancellation,
  beginSafeLogoutRequest,
  cancelSafeLogoutPacket,
  clearSafeLogoutReconnectSuppression,
  clearSafeLogoutStatus,
  consumeSafeLogoutCompletion,
  dispatchSafeLogoutStatus,
  hasSafeLogoutReconnectSuppression,
  receiveSafeLogoutStatus,
  rememberSafeLogoutCompletion,
  requestSafeLogoutPacket,
  safeLogoutReasonMessage,
  safeLogoutStatusView,
  shouldRenderSafeLogout,
} from './safeLogoutStatus';

function status(overrides: Partial<SafeLogoutStatusPacket> = {}): SafeLogoutStatusPacket {
  return {
    packet: 'safe_logout_status',
    version: 1,
    state: 'online',
    can_request: false,
    can_cancel: false,
    message: '',
    in_own_sanctuary: false,
    active_assault: false,
    protected: false,
    ...overrides,
  };
}

assert.equal(NetworkEvent.SAFE_LOGOUT_STATUS, 'SAFE_LOGOUT_STATUS');
assert.deepEqual(requestSafeLogoutPacket(), { cmd: 'request_safe_logout' });
assert.deepEqual(cancelSafeLogoutPacket(), { cmd: 'cancel_safe_logout' });
assert.equal('player_id' in requestSafeLogoutPacket(), false);
assert.equal('player_id' in cancelSafeLogoutPacket(), false);

const eligible = status({
  can_request: true,
  in_own_sanctuary: true,
  message: 'You can safely end your session from this sanctuary.',
});
const eligibleView = safeLogoutStatusView(eligible);
assert.equal(eligibleView?.canRequest, true, 'eligible server state enables Begin');
assert.equal(eligibleView?.inOwnSanctuary, true);
assert.match(SAFE_LOGOUT_CONDITIONS, /Closing the game before it completes will not protect you/);

const outside = safeLogoutStatusView(status({ reason: 'outside_sanctuary' }));
assert.equal(outside?.canRequest, false, 'ineligible state cannot request');
assert.equal(outside?.message, 'Return to your own sanctuary to use Safe Logout.');
assert.equal(
  safeLogoutStatusView(status({
    reason: 'moved',
    message: 'Authoritative server cancellation copy.',
  }))?.message,
  'Authoritative server cancellation copy.',
  'non-empty server copy remains authoritative',
);

const assault = safeLogoutStatusView(status({
  reason: 'assault_active',
  active_assault: true,
}));
assert.equal(assault?.message, SAFE_LOGOUT_ACTIVE_ASSAULT_WARNING);
assert.match(assault?.message || '', /Disconnecting will not stop the assault/);
assert.equal(
  safeLogoutStatusView(status({
    reason: 'assault_active',
    active_assault: true,
    message: 'Authoritative assault warning from the server.',
  }))?.message,
  'Authoritative assault warning from the server.',
);

const pendingPacket = status({
  state: 'pending',
  can_cancel: true,
  in_own_sanctuary: true,
  countdown_total_seconds: 10,
  countdown_remaining_seconds: 7,
  message: 'Remain still and avoid combat until Safe Logout completes.',
});
const pending = safeLogoutStatusView(pendingPacket);
assert.equal(pending?.pending, true);
assert.equal(pending?.canCancel, true);
assert.equal(pending?.countdownSeconds, 7);
assert.equal(pending?.countdownLabel, 'Safe in 7 seconds');

const missingOptional = safeLogoutStatusView(status());
assert.equal(missingOptional?.countdownSeconds, null, 'missing optionals do not crash');
assert.equal(safeLogoutStatusView(status({
  state: 'pending',
  countdown_remaining_seconds: -4,
}))?.countdownSeconds, 0, 'invalid negative presentation is clamped');

assert.equal(
  safeLogoutReasonMessage('moved'),
  'Safe Logout was cancelled because you moved.',
);
assert.equal(
  safeLogoutReasonMessage('took_damage'),
  'Safe Logout was cancelled because you took damage.',
);
assert.equal(
  safeLogoutReasonMessage('hostile_nearby'),
  'Safe Logout is unavailable while enemies are nearby.',
);
assert.equal(
  safeLogoutReasonMessage('future_reason'),
  'Safe Logout is unavailable right now.',
  'unknown future reasons fail safely',
);

const initial: SafeLogoutUiState = {
  safeLogoutStatus: eligible,
  safeLogoutRequestInFlight: false,
  safeLogoutCancelInFlight: false,
};
const requesting = beginSafeLogoutRequest(initial);
assert.equal(requesting.safeLogoutRequestInFlight, true);
assert.equal(
  beginSafeLogoutRequest(requesting),
  requesting,
  'a repeated Begin click cannot create another local request',
);
const ineligibleUi: SafeLogoutUiState = {
  ...initial,
  safeLogoutStatus: status(),
};
assert.equal(
  beginSafeLogoutRequest(ineligibleUi),
  ineligibleUi,
  'an ineligible status cannot create a request',
);

const pendingUi: SafeLogoutUiState = {
  safeLogoutStatus: pendingPacket,
  safeLogoutRequestInFlight: false,
  safeLogoutCancelInFlight: false,
};
const cancelling = beginSafeLogoutCancellation(pendingUi);
assert.equal(cancelling.safeLogoutCancelInFlight, true);
assert.equal(
  beginSafeLogoutCancellation(cancelling),
  cancelling,
  'a repeated Cancel click cannot create another local request',
);

const cancelled = receiveSafeLogoutStatus(status({
  reason: 'moved',
  message: 'Safe Logout was cancelled because you moved.',
}));
assert.equal(cancelled.safeLogoutRequestInFlight, false);
assert.equal(cancelled.safeLogoutCancelInFlight, false);
assert.equal(safeLogoutStatusView(cancelled.safeLogoutStatus)?.pending, false);

const protectedPacket = status({
  state: 'protected',
  protected: true,
  countdown_remaining_seconds: 0,
  message: 'Your settlement is protected. It is now safe to leave.',
});
assert.equal(safeLogoutStatusView(protectedPacket)?.protected, true);
assert.equal(
  safeLogoutStatusView(pendingPacket)?.protected,
  false,
  'the client countdown never independently marks protection',
);

const deliveryOrder: string[] = [];
dispatchSafeLogoutStatus(
  protectedPacket,
  (packet) => deliveryOrder.push(`status:${packet.state}`),
  () => deliveryOrder.push('close'),
);
assert.deepEqual(
  deliveryOrder,
  ['status:protected', 'close'],
  'the complete status dispatches before the intentional close reaction',
);
dispatchSafeLogoutStatus(
  pendingPacket,
  () => deliveryOrder.push('pending'),
  () => deliveryOrder.push('unexpected-close'),
);
assert.equal(deliveryOrder.includes('unexpected-close'), false);

const closeGuard = new SafeLogoutCloseGuard();
assert.equal(closeGuard.suppressConnectionFailure(), false, 'ordinary failures stay ordinary');
assert.equal(closeGuard.acceptProtectedStatus(pendingPacket), false);
assert.equal(closeGuard.acceptProtectedStatus(protectedPacket), true);
assert.equal(closeGuard.suppressConnectionFailure(), true);
assert.equal(
  closeGuard.acceptProtectedStatus(protectedPacket),
  false,
  'duplicate protected packets cannot close twice',
);
closeGuard.resetForLogin();
assert.equal(closeGuard.suppressConnectionFailure(), false, 'new login clears close suppression');

const memory = new Map<string, string>();
const storage = {
  getItem: (key: string) => memory.get(key) || null,
  setItem: (key: string, value: string) => { memory.set(key, value); },
  removeItem: (key: string) => { memory.delete(key); },
};
rememberSafeLogoutCompletion(storage);
assert.equal(hasSafeLogoutReconnectSuppression(storage), true);
assert.equal(consumeSafeLogoutCompletion(storage), SAFE_LOGOUT_COMPLETION_MESSAGE);
assert.equal(consumeSafeLogoutCompletion(storage), null, 'completion feedback is one-time');
assert.equal(
  hasSafeLogoutReconnectSuppression(storage),
  true,
  'rendering completion does not clear reconnect suppression',
);
clearSafeLogoutReconnectSuppression(storage);
assert.equal(hasSafeLogoutReconnectSuppression(storage), false);

assert.equal(shouldRenderSafeLogout(eligible), true);
assert.equal(
  shouldRenderSafeLogout(status({ reason: 'outside_sanctuary' })),
  false,
  'ordinary far-away status does not clutter the Survival Thread',
);
assert.equal(
  shouldRenderSafeLogout(status({ reason: 'moved' })),
  true,
  'cancellation feedback remains visible after leaving the sanctuary',
);
assert.equal(shouldRenderSafeLogout(null), false);
assert.deepEqual(clearSafeLogoutStatus(), {
  safeLogoutStatus: null,
  safeLogoutRequestInFlight: false,
  safeLogoutCancelInFlight: false,
});

console.log('safeLogoutStatus helper checks passed');
