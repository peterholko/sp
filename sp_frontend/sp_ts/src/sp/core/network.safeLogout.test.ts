import assert from 'node:assert/strict';
import { Global } from './global';
import { Network } from './network';
import { NetworkEvent } from './networkEvent';
import {
  SAFE_LOGOUT_COMPLETION_MESSAGE,
  SAFE_LOGOUT_RESUME_MESSAGE,
  SafeLogoutStatusPacket,
  clearSafeLogoutReconnectSuppression,
  hasSafeLogoutReconnectSuppression,
} from './safeLogoutStatus';

class MemoryStorage {
  private readonly values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.get(key) || null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }

  removeItem(key: string): void {
    this.values.delete(key);
  }
}

class FakeClock {
  private nextId = 1;
  readonly callbacks = new Map<number, () => void>();

  setTimeout(callback: () => void): number {
    const id = this.nextId;
    this.nextId += 1;
    this.callbacks.set(id, callback);
    return id;
  }

  clearTimeout(id: number): void {
    this.callbacks.delete(id);
  }

  runAll(): void {
    while (this.callbacks.size > 0) {
      const pending = Array.from(this.callbacks.entries());
      this.callbacks.clear();
      pending.forEach(([, callback]) => callback());
    }
  }
}

class FakeEmitter {
  readonly events: Array<{ event: string; args: any[] }> = [];

  emit(event: string, ...args: any[]): void {
    this.events.push({ event, args });
  }

  on(): void {}

  off(): void {}

  count(event: string): number {
    return this.events.filter((entry) => entry.event === event).length;
  }
}

class FakeWebSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  static readonly instances: FakeWebSocket[] = [];

  readyState = FakeWebSocket.CONNECTING;
  closeCount = 0;
  sent: string[] = [];
  onopen: ((event: any) => void) | null = null;
  onclose: ((event: any) => void) | null = null;
  onerror: ((event: any) => void) | null = null;
  onmessage: ((event: any) => void) | null = null;

  constructor(readonly url: string) {
    FakeWebSocket.instances.push(this);
  }

  send(message: string): void {
    this.sent.push(message);
  }

  close(): void {
    this.closeCount += 1;
    this.readyState = FakeWebSocket.CLOSING;
  }

  open(): void {
    this.readyState = FakeWebSocket.OPEN;
    this.onopen?.({});
  }

  message(packet: unknown): void {
    this.onmessage?.({ data: JSON.stringify(packet) });
  }

  error(): void {
    this.readyState = FakeWebSocket.CLOSED;
    this.onerror?.({});
  }

  closed(): void {
    this.readyState = FakeWebSocket.CLOSED;
    this.onclose?.({});
  }
}

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

const storage = new MemoryStorage();
const clock = new FakeClock();
const emitter = new FakeEmitter();
let reloadCount = 0;

(globalThis as any).window = {
  location: {
    hostname: 'example.test',
    reload: () => { reloadCount += 1; },
  },
  sessionStorage: storage,
  setTimeout: (callback: () => void) => clock.setTimeout(callback),
  clearTimeout: (id: number) => clock.clearTimeout(id),
};
(globalThis as any).document = { visibilityState: 'visible' };
(globalThis as any).WebSocket = FakeWebSocket;
Global.gameEmitter = emitter;

const network = new Network();
Global.network = network;
network.connect();
const firstSocket = FakeWebSocket.instances[0];
firstSocket.open();
assert.equal(clock.callbacks.size, 0, 'opening a socket creates no untracked keepalive timer');

const protectedPacket = status({
  state: 'protected',
  protected: true,
  countdown_remaining_seconds: 0,
  message: 'Your settlement is protected. It is now safe to leave.',
});
firstSocket.message(protectedPacket);
assert.equal(firstSocket.closeCount, 1, 'protected confirmation closes exactly once');
assert.equal(emitter.count(NetworkEvent.SAFE_LOGOUT_COMPLETE), 1);
assert.equal(hasSafeLogoutReconnectSuppression(storage), true);
firstSocket.message({ ...protectedPacket });
assert.equal(firstSocket.closeCount, 1, 'duplicate protected packet cannot close twice');
assert.equal(emitter.count(NetworkEvent.SAFE_LOGOUT_COMPLETE), 1);
firstSocket.closed();
assert.equal(reloadCount, 1, 'intentional-close suppression survives the close callback');
assert.equal(emitter.count(NetworkEvent.NETWORK_ERROR), 0, 'intentional close remains silent');
assert.equal(emitter.count(NetworkEvent.SERVER_OFFLINE), 0, 'intentional close remains silent');
assert.equal(hasSafeLogoutReconnectSuppression(storage), true, 'close callback retains suppression');

network.connect();
const secondSocket = FakeWebSocket.instances[1];
secondSocket.open();
assert.equal(hasSafeLogoutReconnectSuppression(storage), false, 'new login clears suppression');
assert.equal(network.getLatestSafeLogoutStatus(), null, 'new connection clears cached status');
secondSocket.message(status({ can_request: true, in_own_sanctuary: true, message: 'Online.' }));
assert.equal(
  network.getLatestSafeLogoutStatus()?.state,
  'online',
  'reconnect online snapshot replaces any protected presentation',
);

const completionCountBeforeStalePacket = emitter.count(NetworkEvent.SAFE_LOGOUT_COMPLETE);
firstSocket.message({ ...protectedPacket, message: 'Delayed old-socket packet.' });
assert.equal(secondSocket.closeCount, 0, 'old socket cannot close the current connection');
assert.equal(
  emitter.count(NetworkEvent.SAFE_LOGOUT_COMPLETE),
  completionCountBeforeStalePacket,
  'old socket cannot navigate or mutate current safe-logout state',
);

secondSocket.error();
clock.runAll();
assert.equal(emitter.count(NetworkEvent.NETWORK_ERROR), 1, 'later ordinary failure retains reconnect UI');
secondSocket.closed();
clock.runAll();
assert.equal(emitter.count(NetworkEvent.SERVER_OFFLINE), 1, 'ordinary close reaches the health/reconnect surface');

network.connect();
const thirdSocket = FakeWebSocket.instances[2];
thirdSocket.open();
thirdSocket.message(protectedPacket);
assert.equal(hasSafeLogoutReconnectSuppression(storage), true);
clearSafeLogoutReconnectSuppression(storage);
thirdSocket.closed();
assert.equal(reloadCount, 1, 'authentication begun before close cancels only the pending reload');

network.connect();
const fourthSocket = FakeWebSocket.instances[3];
fourthSocket.open();
const pendingAtZero = status({
  state: 'pending',
  can_cancel: true,
  countdown_remaining_seconds: 0,
  message: 'Awaiting server completion.',
});
fourthSocket.message(pendingAtZero);
assert.equal(fourthSocket.closeCount, 0, 'local countdown zero never grants protection');
fourthSocket.message(status({ reason: 'manually_cancelled', message: 'Safe Logout was cancelled.' }));
assert.equal(fourthSocket.closeCount, 0, 'cancellation never schedules local completion');

const resumePacket = status({
  resumed_from_protection: true,
  message: SAFE_LOGOUT_RESUME_MESSAGE,
});
fourthSocket.message(resumePacket);
assert.equal(emitter.count(NetworkEvent.SAFE_LOGOUT_RESUMED), 1, 'resume notice emits once');
assert.equal(
  emitter.events.find((entry) => entry.event === NetworkEvent.SAFE_LOGOUT_RESUMED)?.args[0].message,
  SAFE_LOGOUT_RESUME_MESSAGE,
);
fourthSocket.message({ ...resumePacket });
assert.equal(emitter.count(NetworkEvent.SAFE_LOGOUT_RESUMED), 1, 'duplicate resume snapshot is silent');
assert.deepEqual(network.getLatestSafeLogoutStatus(), resumePacket, 'latest snapshot can replay after UI mount');

fourthSocket.message({
  packet: 'init_perception',
  data: { map: [], visible_objs: [], observers: [], weather: [] },
});
assert.equal(clock.callbacks.size, 1, 'delayed perception callback is tracked');
network.connect();
assert.equal(clock.callbacks.size, 0, 'reconnect clears delayed callbacks');
const fifthSocket = FakeWebSocket.instances[4];
fifthSocket.open();

const replacementNetwork = new Network();
Global.network = replacementNetwork;
replacementNetwork.connect();
const replacementSocket = FakeWebSocket.instances[5];
replacementSocket.open();
assert.equal(replacementNetwork.getLatestSafeLogoutStatus(), null, 'account replacement starts without prior status');
const replacementCompletionCount = emitter.count(NetworkEvent.SAFE_LOGOUT_COMPLETE);
fifthSocket.message({ ...protectedPacket, message: 'Obsolete Network instance.' });
fifthSocket.closed();
assert.equal(replacementSocket.closeCount, 0, 'obsolete Network instance cannot close replacement socket');
assert.equal(emitter.count(NetworkEvent.SAFE_LOGOUT_COMPLETE), replacementCompletionCount);

replacementSocket.message(resumePacket);
assert.deepEqual(replacementNetwork.getLatestSafeLogoutStatus(), resumePacket);
replacementSocket.message(status({
  state: 'pending',
  can_cancel: true,
  countdown_remaining_seconds: 4,
  message: 'Pending before True Death.',
}));
assert.equal(replacementNetwork.getLatestSafeLogoutStatus()?.state, 'pending');
replacementSocket.message({ packet: 'info_true_death' });
assert.equal(replacementNetwork.getLatestSafeLogoutStatus(), null, 'True Death clears pending status');
replacementSocket.message(status({ can_request: true, message: 'Prior run.' }));
replacementSocket.message({ packet: 'select_class', player: 7 });
assert.equal(replacementNetwork.getLatestSafeLogoutStatus(), null, 'fresh hero selection clears prior-run status');

replacementSocket.message(status({ can_request: true, message: 'Prior account.' }));
replacementSocket.message({
  packet: 'init_perception',
  data: { map: [], visible_objs: [], observers: [], weather: [] },
});
assert.equal(clock.callbacks.size, 1, 'old account has one tracked delayed callback before reset');
const completionEventsBeforeAuthReset = emitter.count(NetworkEvent.SAFE_LOGOUT_COMPLETE);
const networkErrorsBeforeAuthReset = emitter.count(NetworkEvent.NETWORK_ERROR);
const offlineEventsBeforeAuthReset = emitter.count(NetworkEvent.SERVER_OFFLINE);
replacementNetwork.resetForAuthentication();
assert.equal(replacementNetwork.getLatestSafeLogoutStatus(), null, 'authentication start clears prior-account status');
assert.equal(clock.callbacks.size, 0, 'authentication start cancels prior-account callbacks');
assert.equal(replacementSocket.closeCount, 1, 'authentication start closes the superseded socket once');
replacementSocket.message(protectedPacket);
replacementSocket.error();
replacementSocket.closed();
clock.runAll();
assert.equal(
  emitter.count(NetworkEvent.SAFE_LOGOUT_COMPLETE),
  completionEventsBeforeAuthReset,
  'superseded account cannot deliver a protected callback',
);
assert.equal(
  emitter.count(NetworkEvent.NETWORK_ERROR),
  networkErrorsBeforeAuthReset,
  'superseded account cannot deliver a delayed network error',
);
assert.equal(
  emitter.count(NetworkEvent.SERVER_OFFLINE),
  offlineEventsBeforeAuthReset,
  'superseded account cannot deliver a delayed offline callback',
);

assert.equal(SAFE_LOGOUT_COMPLETION_MESSAGE, 'Safe Logout complete. Your settlement is protected.');
console.log('network Safe Logout lifecycle checks passed');
