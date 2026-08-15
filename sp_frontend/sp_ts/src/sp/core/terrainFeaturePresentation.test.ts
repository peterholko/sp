import assert from 'node:assert/strict';
import { terrainFeatureButtonTitle } from './terrainFeaturePresentation';

assert.equal(terrainFeatureButtonTitle(undefined), null);
assert.equal(terrainFeatureButtonTitle({ terrain_features: [] }), null);
assert.equal(
  terrainFeatureButtonTitle({
    terrain_features: [{ name: 'Old Growth Groves' }],
  }),
  'View Old Growth Groves',
);

console.log('Terrain feature presentation checks passed');
