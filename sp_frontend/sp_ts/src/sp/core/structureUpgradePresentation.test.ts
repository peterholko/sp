import assert from 'node:assert/strict';
import { structureUpgradePreviewImageName } from './structureUpgradePresentation';

assert.equal(
  structureUpgradePreviewImageName({ image: 'tent' }),
  'tent.png',
  'Shelter Tent uses its configured tent art key',
);
assert.equal(structureUpgradePreviewImageName({}), null);
assert.equal(structureUpgradePreviewImageName(null), null);

console.log('Structure upgrade presentation checks passed');
