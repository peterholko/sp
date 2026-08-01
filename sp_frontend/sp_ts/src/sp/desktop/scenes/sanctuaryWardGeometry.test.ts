import assert from 'node:assert/strict';

import { ObjectState } from '../../core/objectState';
import { ProtectedSettlementLookup } from '../../core/protectedSettlements';
import {
  sanctuaryWardPresentation,
  sanctuaryWardSegments,
} from './sanctuaryWardGeometry';

assert.equal(sanctuaryWardSegments(0, 0, 1).length, 6);
assert.equal(
  sanctuaryWardSegments(0, 0, 3).length,
  30,
  'base full radius 3 encloses exactly the distance < 3 cells',
);
assert.equal(sanctuaryWardSegments(0, 0, 8).length, 90);
assert.equal(
  sanctuaryWardSegments(1, 4, 5).length,
  sanctuaryWardSegments(2, 4, 5).length,
  'odd and even anchor columns produce the same perimeter topology',
);

const settlements: ProtectedSettlementLookup = {
  '12': { player_id: 12, monolith_id: 200, sanctuary_radius: 3 },
};
const visibleMonolith = {
  id: '200',
  player: '2000',
  x: 4,
  y: 6,
  subclass: 'monolith',
  op: 'updated',
} as ObjectState;

const presentation = sanctuaryWardPresentation(visibleMonolith, settlements);
assert.ok(presentation);
assert.equal(presentation?.settlement.player_id, 12);
assert.equal(presentation?.segments.length, 30);

assert.equal(
  sanctuaryWardPresentation({ ...visibleMonolith, op: 'deleted' }, settlements),
  null,
  'a remembered Monolith does not reveal live protection through shroud',
);
assert.equal(
  sanctuaryWardPresentation({ ...visibleMonolith, id: '201' }, settlements),
  null,
  'the ward never guesses a nearby Monolith anchor',
);

console.log('desktop Sanctuary Ward geometry checks passed');
