import assert from 'node:assert/strict';

import {
  LEGACY_MAP_TEXTURE_SCALE,
  MAP_HEX_HALF,
  MAP_HEX_SIZE,
  MAP_HEX_THREE_QUARTERS,
  MAP_RESOURCE_ICON_INSET,
} from './mapGeometry';
import { Util } from './util';

assert.equal(MAP_HEX_SIZE, 144);
assert.equal(MAP_HEX_HALF, 72);
assert.equal(MAP_HEX_THREE_QUARTERS, 108);
assert.equal(MAP_RESOURCE_ICON_INSET, 24);
assert.equal(LEGACY_MAP_TEXTURE_SCALE, 2);

assert.deepEqual(Util.hex_to_pixel(0, 0), { x: 0, y: 0 });
assert.deepEqual(
  Util.hex_to_pixel(1, 0),
  { x: 108, y: 72 },
  'odd columns use the 144px staggered hex spacing',
);
assert.deepEqual(Util.hex_to_pixel(2, 3), { x: 216, y: 432 });

console.log('144px map geometry checks passed');
