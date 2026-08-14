import assert from 'node:assert/strict';

import {
  SanctuaryStatePacket,
  sanctuaryZoneLookup,
} from './sanctuaryState';

const packet: SanctuaryStatePacket = {
  packet: 'sanctuary_state',
  version: 1,
  zones: [
    { monolith_id: 200, radius: 5 },
    { monolith_id: 900, radius: 7 },
  ],
};

const lookup = sanctuaryZoneLookup(packet);
assert.deepEqual(Object.keys(lookup).sort(), ['200', '900']);
assert.deepEqual(lookup['200'], packet.zones[0]);

assert.deepEqual(
  sanctuaryZoneLookup({ ...packet, zones: [] }),
  {},
  'an empty full snapshot clears every live sanctuary boundary',
);
assert.deepEqual(
  sanctuaryZoneLookup({ ...packet, version: 2 }),
  {},
  'unknown versions fail closed instead of retaining stale boundaries',
);
assert.deepEqual(
  sanctuaryZoneLookup({
    ...packet,
    zones: [
      { monolith_id: -1, radius: 5 },
      { monolith_id: 201, radius: 0 },
      { monolith_id: 202, radius: 4.5 },
    ],
  }),
  {},
  'malformed ids and radii are ignored',
);

console.log('live sanctuary state snapshot checks passed');
