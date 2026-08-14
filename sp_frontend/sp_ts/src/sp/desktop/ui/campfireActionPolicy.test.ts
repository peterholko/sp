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
  canLightCampfireTarget(foreignUnlitCampfire, false, '1'),
  true,
  'ownership does not hide the action for a valid unlit Campfire',
);
assert.equal(
  canLightCampfireTarget({ ...foreignUnlitCampfire, image: 'campfirelit' }, false, '1'),
  false,
  'a lit Campfire has no duplicate light action',
);
assert.equal(
  canLightCampfireTarget({
    ...foreignUnlitCampfire,
    player: '1',
    template: 'Shelter Tent',
    subclass: 'shelter',
    image: 'tent',
  }, false, '1'),
  true,
  'an owned Shelter Tent exposes its retained Campfire action',
);
assert.equal(
  canLightCampfireTarget({
    ...foreignUnlitCampfire,
    template: 'Shelter Tent',
    subclass: 'shelter',
    image: 'tent',
  }, false, '1'),
  false,
  'another player cannot tend an owned Shelter Tent',
);
assert.equal(
  canLightCampfireTarget({ ...foreignUnlitCampfire, state: 'founded' }, false, '1'),
  false,
  'an unfinished Campfire cannot be lit',
);
assert.equal(
  canLightCampfireTarget(foreignUnlitCampfire, true, '1'),
  false,
  'Safe Logout protection suppresses the mutation action',
);
assert.equal(canLightCampfireTarget(undefined, false, '1'), false);

console.log('Campfire target action policy checks passed');
