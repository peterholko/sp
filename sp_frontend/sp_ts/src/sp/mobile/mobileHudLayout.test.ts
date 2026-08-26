import assert from 'node:assert/strict';

import {
  compassDirection,
  mobileViewport,
  shouldShowCombatControls,
} from './mobileHudLayout';

assert.deepEqual(mobileViewport(390, 844), {
  width: 390,
  height: 844,
  portrait: true,
  compact: false,
});
assert.equal(mobileViewport(320, 568).compact, true);
assert.equal(mobileViewport(360, 800).compact, true);
assert.equal(mobileViewport(430, 932).compact, false);
assert.equal(mobileViewport(844, 390).portrait, false);

const rect = { left: 10, top: 20, width: 120, height: 120 };
assert.equal(compassDirection(70, 22, rect), 'N');
assert.equal(compassDirection(20, 30, rect), 'NW');
assert.equal(compassDirection(20, 125, rect), 'SW');
assert.equal(compassDirection(70, 138, rect), 'S');
assert.equal(compassDirection(120, 125, rect), 'SE');
assert.equal(compassDirection(120, 30, rect), 'NE');
assert.equal(compassDirection(0, 0, { ...rect, width: 0 }), null);

const objects = {
  10: { subclass: 'npc', state: 'none', presence: 'perceived' },
  11: { subclass: 'npc', state: 'dead', presence: 'perceived' },
  12: { subclass: 'villager', state: 'none', presence: 'perceived' },
  13: { subclass: 'npc', state: 'none', presence: 'remembered' },
};
assert.equal(shouldShowCombatControls({ type: 'obj', id: 10 }, objects), true);
assert.equal(shouldShowCombatControls({ type: 'obj', id: 11 }, objects), false);
assert.equal(shouldShowCombatControls({ type: 'obj', id: 12 }, objects), false);
assert.equal(shouldShowCombatControls({ type: 'obj', id: 13 }, objects), false);
assert.equal(
  shouldShowCombatControls({ type: 'tile' }, objects, { target_id: 10 }),
  true,
);

console.log('mobile HUD layout checks passed');
