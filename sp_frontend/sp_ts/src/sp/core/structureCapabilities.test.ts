import assert from 'node:assert/strict';
import {
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

console.log('Structure capability checks passed');
