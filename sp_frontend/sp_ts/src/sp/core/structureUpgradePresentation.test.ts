import assert from 'node:assert/strict';
import {
  structureUpgradePreviewImageName,
  structureUpgradeProgressImageName,
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

console.log('Structure upgrade presentation checks passed');
