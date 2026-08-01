import assert from 'node:assert/strict';

import { ObjectState } from './objectState';
import {
  ProtectedSettlementsPacket,
  isSafeLogoutProtectedObject,
  protectedSettlementForObject,
  protectedSettlementLookup,
} from './protectedSettlements';

const packet: ProtectedSettlementsPacket = {
  packet: 'protected_settlements',
  version: 1,
  settlements: [
    { player_id: 12, monolith_id: 200, sanctuary_radius: 3 },
    { player_id: 47, monolith_id: 900, sanctuary_radius: 5 },
  ],
};

const lookup = protectedSettlementLookup(packet);
assert.deepEqual(Object.keys(lookup).sort(), ['12', '47']);
assert.deepEqual(lookup['12'], packet.settlements[0]);

const playerStructure = {
  id: '301',
  player: '12',
  subclass: 'burrow',
} as ObjectState;
const neutralMonolith = {
  id: '200',
  player: '2000',
  subclass: 'monolith',
} as ObjectState;
const unrelatedObject = {
  id: '302',
  player: '99',
  subclass: 'villager',
} as ObjectState;

assert.equal(isSafeLogoutProtectedObject(playerStructure, lookup), true);
assert.equal(isSafeLogoutProtectedObject(neutralMonolith, lookup), true);
assert.equal(isSafeLogoutProtectedObject(unrelatedObject, lookup), false);
assert.equal(protectedSettlementForObject(neutralMonolith, lookup)?.player_id, 12);

const cleared = protectedSettlementLookup({ ...packet, settlements: [] });
assert.deepEqual(cleared, {}, 'an empty full snapshot clears every protected settlement');
assert.equal(isSafeLogoutProtectedObject(playerStructure, cleared), false);

assert.deepEqual(
  protectedSettlementLookup({ ...packet, version: 2 }),
  {},
  'unknown protocol versions fail closed instead of retaining stale wards',
);

assert.deepEqual(
  protectedSettlementLookup({
    ...packet,
    settlements: [
      { player_id: -1, monolith_id: 1, sanctuary_radius: 3 },
      { player_id: 1, monolith_id: 2, sanctuary_radius: 0 },
    ],
  }),
  {},
  'malformed settlement anchors are ignored',
);

console.log('protected settlement snapshot checks passed');
