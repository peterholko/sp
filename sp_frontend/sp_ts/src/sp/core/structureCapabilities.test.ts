import assert from 'node:assert/strict';
import {
  canAssignWorkersToStructure,
  isCampfireStation,
  isShelterStructure,
  isUnlitCampfireStation,
} from './structureCapabilities';

const shelterTent = {
  template: 'Shelter Tent',
  subclass: 'shelter',
  image: 'tent',
};

assert.equal(isShelterStructure(shelterTent), true);
assert.equal(isCampfireStation(shelterTent), true);
assert.equal(isUnlitCampfireStation(shelterTent), true);
assert.equal(isUnlitCampfireStation({ ...shelterTent, image: 'tentlit' }), false);
assert.equal(
  isCampfireStation({ template: 'Large Tent', subclass: 'shelter', image: 'well' }),
  false,
);
assert.equal(
  isShelterStructure({ template: 'Campfire', subclass: 'campfire', image: 'campfire' }),
  false,
);

assert.equal(
  canAssignWorkersToStructure({ ...shelterTent, state: 'none' }),
  false,
  'a completed shelter has no workplace assignment',
);
assert.equal(
  canAssignWorkersToStructure({ ...shelterTent, state: 'burning' }),
  false,
  'lighting a completed shelter does not make it a workplace',
);
assert.equal(
  canAssignWorkersToStructure({ ...shelterTent, state: 'building' }),
  true,
  'an unfinished shelter can accept builders',
);
assert.equal(
  canAssignWorkersToStructure({ state: 'none', workspaces: 1 }),
  true,
  'a completed workplace remains assignable',
);
assert.equal(
  canAssignWorkersToStructure({ state: 'none', subclass: 'craft' }),
  true,
  'legacy crafting stations remain worker-capable even without workspace metadata',
);
assert.equal(
  canAssignWorkersToStructure({ state: 'dead', workspaces: 1 }),
  false,
  'destroyed structures are never assignable',
);

console.log('Structure capability checks passed');
