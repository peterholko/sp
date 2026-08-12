import assert from 'node:assert/strict';
import {
  LEGACY_MAP_ART_SCALE,
  NATIVE_144_MAP_ART_SCALE,
  mapArtDefinitionForImage,
  mapArtPresentation,
  mapArtScale,
} from './mapArtScale';

assert.equal(mapArtScale(undefined), LEGACY_MAP_ART_SCALE);
assert.equal(mapArtScale({}), LEGACY_MAP_ART_SCALE);
assert.equal(mapArtScale({ map_scale: 0 }), LEGACY_MAP_ART_SCALE);
assert.equal(mapArtScale({ map_scale: -1 }), LEGACY_MAP_ART_SCALE);
assert.equal(mapArtScale({ map_scale: 'not-a-number' }), LEGACY_MAP_ART_SCALE);
assert.equal(mapArtScale({ map_scale: '1' }), NATIVE_144_MAP_ART_SCALE);
assert.equal(mapArtScale({ map_scale: '0.5' }), 0.5);
assert.equal(
  mapArtScale(mapArtDefinitionForImage('foundation')),
  NATIVE_144_MAP_ART_SCALE,
);
assert.equal(
  mapArtScale(mapArtDefinitionForImage('gravestone')),
  NATIVE_144_MAP_ART_SCALE,
);
assert.equal(
  mapArtScale(mapArtDefinitionForImage('rubble')),
  LEGACY_MAP_ART_SCALE,
);
assert.equal(
  mapArtScale(mapArtDefinitionForImage('foundation', { map_scale: 0.25 })),
  0.25,
);

assert.deepEqual(
  mapArtPresentation({ map_scale: NATIVE_144_MAP_ART_SCALE }, 144, 144),
  { scale: 1, insetX: 0, insetY: 0 },
  'native 144px art occupies the full 144px logical footprint',
);

assert.deepEqual(
  mapArtPresentation({ map_scale: NATIVE_144_MAP_ART_SCALE }, 144, 144, 0.5),
  { scale: 0.5, insetX: 36, insetY: 36 },
  'special presentation scaling is centred within the native-art logical footprint',
);

assert.deepEqual(
  mapArtPresentation(undefined, 72, 72, 0.5),
  { scale: 1, insetX: 36, insetY: 36 },
  'legacy art is enlarged before special presentation scaling is applied',
);
