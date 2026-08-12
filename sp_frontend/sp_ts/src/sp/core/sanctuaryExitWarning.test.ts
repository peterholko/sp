import assert from 'node:assert/strict';

import { ObjectState } from './objectState';
import { SanctuaryZoneLookup } from './sanctuaryState';
import {
  isInsideSanctuary,
  shouldConfirmSanctuaryExit,
} from './sanctuaryExitWarning';

const zones: SanctuaryZoneLookup = {
  '200': { monolith_id: 200, radius: 3 },
};
const monolith = {
  id: '200',
  subclass: 'monolith',
  x: 10,
  y: 10,
  op: 'updated',
} as ObjectState;
const objectStates: Record<string, ObjectState> = { '200': monolith };

assert.equal(isInsideSanctuary({ q: 10, r: 10 }, zones, objectStates), true);
assert.equal(isInsideSanctuary({ q: 12, r: 10 }, zones, objectStates), true);
assert.equal(
  isInsideSanctuary({ q: 13, r: 10 }, zones, objectStates),
  false,
  'distance equal to radius is outside, matching the server rule',
);

assert.equal(
  shouldConfirmSanctuaryExit(
    { q: 12, r: 10 },
    { q: 13, r: 10 },
    zones,
    objectStates,
    false,
  ),
  true,
  'the first outward boundary crossing requires confirmation',
);
assert.equal(
  shouldConfirmSanctuaryExit(
    { q: 12, r: 10 },
    { q: 11, r: 10 },
    zones,
    objectStates,
    false,
  ),
  false,
  'movement within Sanctuary is never interrupted',
);
assert.equal(
  shouldConfirmSanctuaryExit(
    { q: 12, r: 10 },
    { q: 13, r: 10 },
    zones,
    objectStates,
    true,
  ),
  false,
  'a confirmed hero is not warned again during the client session',
);
assert.equal(
  shouldConfirmSanctuaryExit(
    { q: 13, r: 10 },
    { q: 14, r: 10 },
    zones,
    objectStates,
    false,
  ),
  false,
  'movement that starts outside Sanctuary does not warn',
);
assert.equal(
  shouldConfirmSanctuaryExit(
    { q: 12, r: 10 },
    { q: 13, r: 10 },
    zones,
    {
      '200': { ...monolith, op: 'deleted', eventType: 'obj_delete' },
    },
    false,
  ),
  false,
  'an authoritatively removed Monolith cannot trigger a stale warning',
);

console.log('sanctuary exit warning policy checks passed');
