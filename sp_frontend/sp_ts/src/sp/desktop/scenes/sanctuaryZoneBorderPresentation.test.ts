import assert from 'node:assert/strict';

import { ObjectState } from '../../core/objectState';
import { ProtectedSettlementLookup } from '../../core/protectedSettlements';
import { SanctuaryZoneLookup } from '../../core/sanctuaryState';
import { sanctuaryWardSegments } from './sanctuaryWardGeometry';
import { sanctuaryZoneBorderPresentation } from './sanctuaryZoneBorderPresentation';

const zones: SanctuaryZoneLookup = {
  '200': { monolith_id: 200, radius: 5 },
};
const visibleMonolith = {
  id: '200',
  player: '2000',
  x: 4,
  y: 6,
  subclass: 'monolith',
  op: 'updated',
} as ObjectState;

const presentation = sanctuaryZoneBorderPresentation(visibleMonolith, zones, {}, true);
assert.ok(presentation);
assert.equal(presentation?.zone.radius, 5);
assert.deepEqual(
  presentation?.segments,
  sanctuaryWardSegments(4, 6, 5),
  'the live border reuses the strict-radius Safe Logout perimeter geometry',
);

assert.equal(
  sanctuaryZoneBorderPresentation(
    { ...visibleMonolith, op: 'deleted', eventType: 'obj_delete' },
    zones,
    {},
    true,
  ),
  null,
  'an authoritatively removed Monolith cannot anchor the live boundary',
);
assert.ok(
  sanctuaryZoneBorderPresentation(
    { ...visibleMonolith, op: 'deleted', eventType: 'perception' },
    zones,
    {},
    true,
  ),
  'a learned Monolith keeps anchoring the border after leaving current perception',
);
assert.equal(
  sanctuaryZoneBorderPresentation({ ...visibleMonolith, id: '201' }, zones, {}, true),
  null,
  'the live boundary never guesses a Monolith anchor',
);

const sameMonolithProtected: ProtectedSettlementLookup = {
  '12': { player_id: 12, monolith_id: 200, sanctuary_radius: 5 },
};
assert.equal(
  sanctuaryZoneBorderPresentation(visibleMonolith, zones, sameMonolithProtected, true),
  null,
  'the blue Safe Logout ward supersedes the green live border',
);

const otherMonolithProtected: ProtectedSettlementLookup = {
  '12': { player_id: 12, monolith_id: 201, sanctuary_radius: 5 },
};
assert.ok(
  sanctuaryZoneBorderPresentation(visibleMonolith, zones, otherMonolithProtected, true),
  'an unrelated Safe Logout ward does not hide the live boundary',
);

assert.equal(
  sanctuaryZoneBorderPresentation(visibleMonolith, zones, {}, false),
  null,
  'the local shield toggle hides the sanctuary border without changing its state',
);
assert.equal(
  sanctuaryZoneBorderPresentation(visibleMonolith, zones, {}),
  null,
  'missing presentation state defaults to the initially hidden border',
);

console.log('desktop live Sanctuary Zone border presentation checks passed');
