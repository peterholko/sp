import assert from 'node:assert/strict';
import { SafeLogoutStatusPacket } from '../../core/safeLogoutStatus';
import ObjectivesPanel from './objectivesPanel';

function status(overrides: Partial<SafeLogoutStatusPacket> = {}): SafeLogoutStatusPacket {
  return {
    packet: 'safe_logout_status',
    version: 1,
    state: 'online',
    can_request: false,
    can_cancel: false,
    message: 'Return to your own sanctuary to use Safe Logout.',
    in_own_sanctuary: false,
    active_assault: false,
    protected: false,
    reason: 'outside_sanctuary',
    ...overrides,
  };
}

const panel: any = new ObjectivesPanel({});
panel.setState = (update) => {
  const next = typeof update === 'function' ? update(panel.state, panel.props) : update;
  panel.state = { ...panel.state, ...next };
};

panel.handleSafeLogoutStatus(status());
assert.equal(
  panel.state.expanded,
  false,
  'an ordinary Safe Logout status must not open the Survival drawer',
);

panel.toggleExpanded();
assert.equal(panel.state.expanded, true, 'the player can open the Survival drawer');
panel.toggleExpanded();
assert.equal(panel.state.expanded, false, 'the player can close the Survival drawer');

panel.handleSafeLogoutStatus(status({
  state: 'pending',
  can_cancel: true,
  reason: undefined,
  countdown_remaining_seconds: 9,
}));
assert.equal(
  panel.state.expanded,
  false,
  'a pending status must respect a drawer the player closed',
);

panel.state.expanded = true;
panel.handleSafeLogoutStatus(status({
  state: 'pending',
  can_cancel: true,
  reason: undefined,
  countdown_remaining_seconds: 8,
}));
assert.equal(
  panel.state.expanded,
  true,
  'status updates must also preserve an open drawer',
);

panel.handleRunReset();
assert.equal(panel.state.expanded, false, 'a fresh run starts with the drawer closed');

console.log('Mobile Survival drawer state checks passed');
