import assert from 'node:assert/strict';
import {
  NO_LEARNED_STRUCTURE_UPGRADES,
  structureUpgradePreviewImageName,
  structureUpgradeProgressImageName,
  structureUpgradeOptions,
} from './structureUpgradePresentation';

assert.equal(
  structureUpgradePreviewImageName({ image: 'tent' }),
  'tent.png',
  'Shelter Tent uses its configured tent art key',
);
assert.equal(structureUpgradePreviewImageName({}), null);
assert.equal(structureUpgradePreviewImageName(null), null);
assert.equal(
  structureUpgradeProgressImageName({ selected_upgrade_image: 'tent' }),
  'tent.png',
  'an active Shelter Tent upgrade keeps using its configured tent art key',
);
assert.equal(
  structureUpgradeProgressImageName({}),
  null,
  'an active upgrade never guesses an asset filename from its display name',
);
assert.deepEqual(structureUpgradeOptions(null), []);
assert.deepEqual(structureUpgradeOptions({}), []);
assert.deepEqual(
  structureUpgradeOptions({ upgrade_list: [{ name: 'Shelter Tent', image: 'tent' }] }),
  [{ name: 'Shelter Tent', image: 'tent' }],
);
assert.equal(
  NO_LEARNED_STRUCTURE_UPGRADES,
  'No learned upgrades are available for this structure. Use the required deed first.',
);

console.log('Structure upgrade presentation checks passed');
