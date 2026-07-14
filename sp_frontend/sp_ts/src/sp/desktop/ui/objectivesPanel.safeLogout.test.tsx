import assert from 'node:assert/strict';
import * as React from 'react';
import { Global } from '../../core/global';
import { SafeLogoutStatusPacket } from '../../core/safeLogoutStatus';
import ObjectivesPanel from './objectivesPanel';

class FakeEmitter {
  onCount = 0;
  offCount = 0;

  on(): void {
    this.onCount += 1;
  }

  off(): void {
    this.offCount += 1;
  }

  emit(): void {}
}

function status(overrides: Partial<SafeLogoutStatusPacket> = {}): SafeLogoutStatusPacket {
  return {
    packet: 'safe_logout_status',
    version: 1,
    state: 'online',
    can_request: true,
    can_cancel: false,
    message: 'You can safely end your session from this sanctuary.',
    in_own_sanctuary: true,
    active_assault: false,
    protected: false,
    ...overrides,
  };
}

function descendants(node: any): any[] {
  if (node === null || node === undefined || typeof node !== 'object') {
    return [];
  }

  const children = React.Children.toArray(node.props && node.props.children);
  return [node].concat(children.flatMap((child) => descendants(child)));
}

let resizeAdds = 0;
let resizeRemoves = 0;
let timerCreates = 0;
(globalThis as any).window = {
  innerWidth: 1024,
  innerHeight: 900,
  screen: { width: 1920, height: 1080 },
  __SP_DESKTOP__: true,
  addEventListener: () => { resizeAdds += 1; },
  removeEventListener: () => { resizeRemoves += 1; },
  setTimeout: () => { timerCreates += 1; return timerCreates; },
};

const emitter = new FakeEmitter();
Global.gameEmitter = emitter;
let beginRequests = 0;
let cancelRequests = 0;
let latestStatus: SafeLogoutStatusPacket | null = status();
Global.network = {
  getLatestSafeLogoutStatus: () => latestStatus,
  clearLatestSafeLogoutStatus: () => { latestStatus = null; },
  sendRequestSafeLogout: () => { beginRequests += 1; return true; },
  sendCancelSafeLogout: () => { cancelRequests += 1; return true; },
};

const panel: any = new ObjectivesPanel({});
panel.setState = (update) => {
  const next = typeof update === 'function' ? update(panel.state, panel.props) : update;
  panel.state = { ...panel.state, ...next };
};
panel.componentDidMount();
assert.equal(panel.state.safeLogoutStatus?.can_request, true, 'mount replays an early cached status');
assert.equal(resizeAdds, 1);
assert.equal(timerCreates, 0, 'safe-logout component mount creates no interpolation timer');

const compactNodes = descendants(panel.render());
const compactRoot = compactNodes[0];
assert.equal(compactRoot.props.style.width, '280px', 'compact layout remains bounded and expanded');
assert.equal(compactRoot.props.style.overflowY, 'auto');
const beginButton = compactNodes.find((node) => node.type === 'button'
  && node.props['aria-label'] === 'Begin Safe Logout countdown');
assert.ok(beginButton, 'compact layout renders Begin control');
assert.equal(beginButton.props.type, 'button', 'native button preserves Enter/Space keyboard behavior');
beginButton.props.onClick();
beginButton.props.onClick();
assert.equal(beginRequests, 1, 'component action lock suppresses duplicate keyboard/click activation');

(globalThis as any).window.innerWidth = 1920;
panel.state.viewportWidth = 1920;
const wideRoot = descendants(panel.render())[0];
assert.equal(wideRoot.props.style.width, '290px', 'wide layout retains its fixed readable width');
assert.equal(wideRoot.props.style.overflowY, 'auto');

const pendingPanel: any = new ObjectivesPanel({});
pendingPanel.setState = (update) => {
  const next = typeof update === 'function' ? update(pendingPanel.state, pendingPanel.props) : update;
  pendingPanel.state = { ...pendingPanel.state, ...next };
};
pendingPanel.handleSafeLogoutStatus(status({
  state: 'pending',
  can_request: false,
  can_cancel: true,
  countdown_remaining_seconds: 6,
  message: 'Remain still and avoid combat until Safe Logout completes.',
}));
const pendingNodes = descendants(pendingPanel.render());
const liveRegions = pendingNodes.filter((node) => node.props && node.props['aria-live'] === 'polite');
assert.equal(liveRegions.length, 2, 'message and changing countdown use polite live regions');
assert.ok(liveRegions.every((node) => node.props['aria-atomic'] === 'true'));
const cancelButton = pendingNodes.find((node) => node.type === 'button'
  && node.props['aria-label'] === 'Cancel Safe Logout countdown');
assert.ok(cancelButton);
assert.equal(cancelButton.props.type, 'button');
cancelButton.props.onClick();
cancelButton.props.onClick();
assert.equal(cancelRequests, 1);

pendingPanel.handleHeroInit('hero-a');
pendingPanel.handleSafeLogoutStatus(status({ state: 'pending', can_cancel: true }));
pendingPanel.handleHeroInit('hero-b');
assert.equal(pendingPanel.state.safeLogoutStatus, null, 'hero recreation clears pending status');
pendingPanel.handleSafeLogoutStatus(status({ state: 'pending', can_cancel: true }));
pendingPanel.handleRunReset();
assert.equal(pendingPanel.state.safeLogoutStatus, null, 'True Death/fresh-run reset clears pending status');

panel.componentWillUnmount();
assert.equal(resizeRemoves, 1);
assert.equal(emitter.offCount, emitter.onCount, 'component unmount removes every lifecycle listener');
assert.equal(timerCreates, 0, 'component unmount leaves no Safe Logout timer');

console.log('ObjectivesPanel Safe Logout component checks passed');
