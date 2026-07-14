export type SafeLogoutState =
  | 'online'
  | 'pending'
  | 'protected'
  | 'disconnected';

export type KnownSafeLogoutReason =
  | 'outside_sanctuary'
  | 'sanctuary_invalid'
  | 'hostile_nearby'
  | 'recent_combat'
  | 'recent_damage'
  | 'assault_active'
  | 'hero_invalid'
  | 'hero_dead'
  | 'true_death'
  | 'run_invalid'
  | 'already_pending'
  | 'already_protected'
  | 'moved'
  | 'entered_combat'
  | 'took_damage'
  | 'left_sanctuary'
  | 'assault_started'
  | 'hero_died'
  | 'disconnected_before_completion'
  | 'manually_cancelled'
  | 'run_ended'
  | 'unknown';

export interface RequestSafeLogoutPacket {
  cmd: 'request_safe_logout';
}

export interface CancelSafeLogoutPacket {
  cmd: 'cancel_safe_logout';
}

export function requestSafeLogoutPacket(): RequestSafeLogoutPacket {
  return { cmd: 'request_safe_logout' };
}

export function cancelSafeLogoutPacket(): CancelSafeLogoutPacket {
  return { cmd: 'cancel_safe_logout' };
}

/**
 * Authoritative safe-logout snapshot sent by the server. Optional values may
 * be omitted or encoded as null by serde. The index signature permits additive
 * protocol changes while stable fields remain typed.
 */
export interface SafeLogoutStatusPacket {
  packet: 'safe_logout_status';
  version: number;
  state: SafeLogoutState;
  can_request: boolean;
  can_cancel: boolean;
  countdown_total_seconds?: number | null;
  countdown_remaining_seconds?: number | null;
  reason?: string | null;
  message: string;
  in_own_sanctuary: boolean;
  active_assault: boolean;
  protected: boolean;
  [futureField: string]: unknown;
}

export interface SafeLogoutUiState {
  safeLogoutStatus: SafeLogoutStatusPacket | null;
  safeLogoutRequestInFlight: boolean;
  safeLogoutCancelInFlight: boolean;
}

export interface SafeLogoutStatusView {
  status: SafeLogoutStatusPacket;
  state: SafeLogoutState;
  pending: boolean;
  protected: boolean;
  canRequest: boolean;
  canCancel: boolean;
  inOwnSanctuary: boolean;
  activeAssault: boolean;
  countdownSeconds: number | null;
  countdownLabel: string | null;
  message: string;
  reason: string | null;
  reasonMessage: string | null;
}

export const SAFE_LOGOUT_CONDITIONS =
  'Safe Logout takes 10 seconds. Remain inside your sanctuary, stay still, and avoid combat. Closing the game before it completes will not protect you.';

export const SAFE_LOGOUT_ACTIVE_ASSAULT_WARNING =
  'Safe Logout is unavailable during an active assault. Disconnecting will not stop the assault.';

export const SAFE_LOGOUT_COMPLETION_MESSAGE =
  'Safe Logout complete. Your settlement is protected.';

export const SAFE_LOGOUT_COMPLETION_STORAGE_KEY =
  'siege_perilous.safe_logout.completed';

export const SAFE_LOGOUT_RECONNECT_SUPPRESSION_STORAGE_KEY =
  'siege_perilous.safe_logout.suppress_reconnect';

const REASON_MESSAGES: Record<KnownSafeLogoutReason, string> = {
  outside_sanctuary: 'Return to your own sanctuary to use Safe Logout.',
  sanctuary_invalid: 'Your sanctuary could not be verified. Safe Logout is unavailable.',
  hostile_nearby: 'Safe Logout is unavailable while enemies are nearby.',
  recent_combat: 'Wait until you have been out of combat.',
  recent_damage: 'Wait until you have been safe from damage.',
  assault_active: SAFE_LOGOUT_ACTIVE_ASSAULT_WARNING,
  hero_invalid: 'Your hero could not be verified. Safe Logout is unavailable.',
  hero_dead: 'A dead hero cannot begin Safe Logout.',
  true_death: 'Safe Logout is unavailable after True Death.',
  run_invalid: 'Your current settlement could not be verified.',
  already_pending: 'Safe Logout is already counting down.',
  already_protected: 'Your settlement is already protected.',
  moved: 'Safe Logout was cancelled because you moved.',
  entered_combat: 'Safe Logout was cancelled because you entered combat.',
  took_damage: 'Safe Logout was cancelled because you took damage.',
  left_sanctuary: 'Safe Logout was cancelled because you left your sanctuary.',
  assault_started: SAFE_LOGOUT_ACTIVE_ASSAULT_WARNING,
  hero_died: 'Safe Logout was cancelled because your hero died.',
  disconnected_before_completion:
    'Safe Logout did not complete before the connection closed. Your settlement is not protected.',
  manually_cancelled: 'Safe Logout was cancelled.',
  run_ended: 'Safe Logout ended with the previous settlement run.',
  unknown: 'Safe Logout is unavailable right now.',
};

const INTERACTION_FEEDBACK_REASONS = new Set<string>([
  'already_pending',
  'already_protected',
  'moved',
  'entered_combat',
  'took_damage',
  'left_sanctuary',
  'assault_started',
  'hero_died',
  'disconnected_before_completion',
  'manually_cancelled',
  'run_ended',
]);

function finiteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function nonEmptyString(value: unknown): string | null {
  return typeof value === 'string' && value.trim() !== '' ? value : null;
}

export function safeLogoutReasonMessage(reason?: string | null): string | null {
  if (!reason) {
    return null;
  }

  if (Object.prototype.hasOwnProperty.call(REASON_MESSAGES, reason)) {
    return REASON_MESSAGES[reason as KnownSafeLogoutReason];
  }

  return REASON_MESSAGES.unknown;
}

export function safeLogoutCountdownSeconds(value?: number | null): number | null {
  if (!finiteNumber(value)) {
    return null;
  }

  return Math.max(0, Math.floor(value));
}

export function safeLogoutStatusView(
  packet?: SafeLogoutStatusPacket | null,
): SafeLogoutStatusView | null {
  if (!packet || packet.packet !== 'safe_logout_status') {
    return null;
  }

  const pending = packet.state === 'pending';
  const protectedStatus = packet.state === 'protected' && packet.protected === true;
  const activeAssault = packet.active_assault === true;
  const reason = nonEmptyString(packet.reason);
  const reasonMessage = activeAssault
    ? SAFE_LOGOUT_ACTIVE_ASSAULT_WARNING
    : safeLogoutReasonMessage(reason);
  const countdownSeconds = pending
    ? safeLogoutCountdownSeconds(packet.countdown_remaining_seconds)
    : null;

  let fallbackMessage = 'Return to your own sanctuary to use Safe Logout.';
  if (packet.can_request) {
    fallbackMessage = 'You can safely end your session from this sanctuary.';
  } else if (pending) {
    fallbackMessage = 'Remain still and avoid combat until Safe Logout completes.';
  } else if (protectedStatus) {
    fallbackMessage = 'Your settlement is protected. It is now safe to leave.';
  }

  return {
    status: packet,
    state: packet.state,
    pending,
    protected: protectedStatus,
    canRequest: packet.state === 'online' && packet.can_request === true,
    canCancel: pending && packet.can_cancel === true,
    inOwnSanctuary: packet.in_own_sanctuary === true,
    activeAssault,
    countdownSeconds,
    countdownLabel: countdownSeconds === null
      ? null
      : `Safe in ${countdownSeconds} ${countdownSeconds === 1 ? 'second' : 'seconds'}`,
    message: nonEmptyString(packet.message) || reasonMessage || fallbackMessage,
    reason,
    reasonMessage,
  };
}

export function receiveSafeLogoutStatus(
  packet?: SafeLogoutStatusPacket | null,
): SafeLogoutUiState {
  return {
    safeLogoutStatus: packet && packet.packet === 'safe_logout_status' ? packet : null,
    safeLogoutRequestInFlight: false,
    safeLogoutCancelInFlight: false,
  };
}

export function clearSafeLogoutStatus(): SafeLogoutUiState {
  return {
    safeLogoutStatus: null,
    safeLogoutRequestInFlight: false,
    safeLogoutCancelInFlight: false,
  };
}

export function beginSafeLogoutRequest(state: SafeLogoutUiState): SafeLogoutUiState {
  const view = safeLogoutStatusView(state.safeLogoutStatus);
  if (!view || !view.canRequest || state.safeLogoutRequestInFlight) {
    return state;
  }

  return {
    ...state,
    safeLogoutRequestInFlight: true,
    safeLogoutCancelInFlight: false,
  };
}

export function beginSafeLogoutCancellation(state: SafeLogoutUiState): SafeLogoutUiState {
  const view = safeLogoutStatusView(state.safeLogoutStatus);
  if (!view || !view.canCancel || state.safeLogoutCancelInFlight) {
    return state;
  }

  return {
    ...state,
    safeLogoutRequestInFlight: false,
    safeLogoutCancelInFlight: true,
  };
}

export function shouldRenderSafeLogout(
  packet?: SafeLogoutStatusPacket | null,
): boolean {
  const view = safeLogoutStatusView(packet);
  if (!view) {
    return false;
  }

  return view.inOwnSanctuary
    || view.pending
    || view.protected
    || Boolean(view.reason && INTERACTION_FEEDBACK_REASONS.has(view.reason));
}

/** Emits the complete snapshot before invoking any protected-close reaction. */
export function dispatchSafeLogoutStatus(
  packet: SafeLogoutStatusPacket,
  emitStatus: (status: SafeLogoutStatusPacket) => void,
  handleProtected: (status: SafeLogoutStatusPacket) => void,
): void {
  emitStatus(packet);
  if (packet.state === 'protected' && packet.protected === true) {
    handleProtected(packet);
  }
}

/**
 * Keeps the socket close a one-shot transport reaction to server authority.
 * It does not infer protection and is reset only when a new connection starts.
 */
export class SafeLogoutCloseGuard {
  private intentional = false;
  private protectedPacketHandled = false;

  resetForLogin() {
    this.intentional = false;
    this.protectedPacketHandled = false;
  }

  acceptProtectedStatus(packet?: SafeLogoutStatusPacket | null): boolean {
    if (
      this.protectedPacketHandled
      || !packet
      || packet.state !== 'protected'
      || packet.protected !== true
    ) {
      return false;
    }

    this.protectedPacketHandled = true;
    this.intentional = true;
    return true;
  }

  suppressConnectionFailure(): boolean {
    return this.intentional;
  }
}

interface CompletionStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

export function rememberSafeLogoutCompletion(storage: CompletionStorage): void {
  storage.setItem(SAFE_LOGOUT_RECONNECT_SUPPRESSION_STORAGE_KEY, 'true');
  storage.setItem(SAFE_LOGOUT_COMPLETION_STORAGE_KEY, SAFE_LOGOUT_COMPLETION_MESSAGE);
}

export function consumeSafeLogoutCompletion(storage: CompletionStorage): string | null {
  const message = storage.getItem(SAFE_LOGOUT_COMPLETION_STORAGE_KEY);
  storage.removeItem(SAFE_LOGOUT_COMPLETION_STORAGE_KEY);
  return message;
}

export function hasSafeLogoutReconnectSuppression(storage: CompletionStorage): boolean {
  return storage.getItem(SAFE_LOGOUT_RECONNECT_SUPPRESSION_STORAGE_KEY) === 'true';
}

export function clearSafeLogoutReconnectSuppression(storage: CompletionStorage): void {
  storage.removeItem(SAFE_LOGOUT_RECONNECT_SUPPRESSION_STORAGE_KEY);
  storage.removeItem(SAFE_LOGOUT_COMPLETION_STORAGE_KEY);
}
