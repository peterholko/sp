import assert from 'node:assert/strict';

import { MAP_RENDER_EVENTS } from './mapRenderEvents';
import { NetworkEvent } from './networkEvent';

assert.deepEqual(MAP_RENDER_EVENTS, [
  NetworkEvent.PERCEPTION,
  NetworkEvent.NEW_PERCEPTION,
  NetworkEvent.OBJ_PERCEPTION,
]);
assert.ok(
  MAP_RENDER_EVENTS.includes(NetworkEvent.NEW_PERCEPTION),
  'an in-place vision increase must redraw newly explored map tiles',
);

console.log('map render perception event checks passed');
