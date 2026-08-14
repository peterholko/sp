import assert from 'node:assert/strict';

import {
  SANCTUARY_BORDER_VISIBLE_BY_DEFAULT,
  showSanctuaryBorderForMonolithSelection,
  toggleSanctuaryBorderVisibility,
} from './sanctuaryBorderVisibility';

assert.equal(
  SANCTUARY_BORDER_VISIBLE_BY_DEFAULT,
  false,
  'the sanctuary boundary starts hidden',
);

const emitted: boolean[] = [];
const visibleAtNotification: boolean[] = [];
const state = { sanctuaryBorderVisible: SANCTUARY_BORDER_VISIBLE_BY_DEFAULT };
const notify = (next: boolean) => {
  emitted.push(next);
  visibleAtNotification.push(state.sanctuaryBorderVisible);
};

let visible = toggleSanctuaryBorderVisibility(state, notify);
assert.equal(visible, true);
assert.equal(state.sanctuaryBorderVisible, true);

visible = toggleSanctuaryBorderVisibility(state, notify);
assert.equal(visible, false);
assert.equal(state.sanctuaryBorderVisible, false);

assert.deepEqual(emitted, [true, false]);
assert.deepEqual(
  visibleAtNotification,
  [true, false],
  'listeners must observe the committed visibility value immediately',
);

const objectStates = {
  '200': { subclass: 'monolith' },
  '201': { subclass: 'monolith' },
  '300': { subclass: 'structure' },
};
const sanctuaryZones = { '200': { monolith_id: 200, radius: 5 } };

visible = showSanctuaryBorderForMonolithSelection(
  state,
  { type: 'obj', id: 200 },
  objectStates,
  sanctuaryZones,
  notify,
);
assert.equal(visible, true);
assert.equal(state.sanctuaryBorderVisible, true);
assert.deepEqual(emitted, [true, false, true]);

showSanctuaryBorderForMonolithSelection(
  state,
  { type: 'obj', id: 200 },
  objectStates,
  sanctuaryZones,
  notify,
);
assert.deepEqual(
  emitted,
  [true, false, true],
  'clicking the Monolith while visible is idempotent rather than hiding the border',
);

toggleSanctuaryBorderVisibility(state, notify);
showSanctuaryBorderForMonolithSelection(
  state,
  { type: 'obj', id: 201 },
  objectStates,
  sanctuaryZones,
  notify,
);
showSanctuaryBorderForMonolithSelection(
  state,
  { type: 'obj', id: 300 },
  objectStates,
  sanctuaryZones,
  notify,
);
assert.equal(state.sanctuaryBorderVisible, false);
assert.deepEqual(
  emitted,
  [true, false, true, false],
  'unbound Monoliths and ordinary objects do not reveal the player boundary',
);

console.log('sanctuary border visibility toggle checks passed');
