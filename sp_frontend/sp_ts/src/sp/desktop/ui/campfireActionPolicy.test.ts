import assert from 'node:assert/strict';
import { canLightCampfireTarget } from './campfireActionPolicy';

const foreignUnlitCampfire = {
  player: '9',
  subclass: 'campfire',
  template: 'Campfire',
  state: 'none',
  image: 'campfire',
};

assert.equal(
  canLightCampfireTarget(foreignUnlitCampfire, false),
  true,
  'ownership does not hide the action for a valid unlit Campfire',
);
assert.equal(
  canLightCampfireTarget({ ...foreignUnlitCampfire, image: 'campfirelit' }, false),
  false,
  'a lit Campfire has no duplicate light action',
);
assert.equal(
  canLightCampfireTarget({ ...foreignUnlitCampfire, template: 'Tent', subclass: 'shelter' }, false),
  false,
  'Campfire-capable shelters are not standalone Campfires',
);
assert.equal(
  canLightCampfireTarget({ ...foreignUnlitCampfire, state: 'founded' }, false),
  false,
  'an unfinished Campfire cannot be lit',
);
assert.equal(
  canLightCampfireTarget(foreignUnlitCampfire, true),
  false,
  'Safe Logout protection suppresses the mutation action',
);
assert.equal(canLightCampfireTarget(undefined, false), false);

console.log('Campfire target action policy checks passed');
