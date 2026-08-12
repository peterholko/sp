import assert from 'node:assert/strict';

import {
  isPerceivedMapObject,
  isPresentMapObject,
  markMapObjectDestroyed,
  markMapObjectOutsidePerception,
  markMapObjectPerceived,
} from './mapObjectPresence';

assert.equal(
  isPresentMapObject({ class: 'unit', op: 'none', eventType: undefined }),
  true,
  'a currently perceived object remains present',
);
assert.equal(
  isPresentMapObject({ class: 'unit', op: 'deleted', eventType: 'perception' }),
  false,
  'a unit outside current perception is not a remembered target',
);
assert.equal(
  isPresentMapObject({
    class: 'structure',
    presence: 'remembered',
    op: 'deleted',
    eventType: 'perception',
  }),
  true,
  'a remembered structure stays selectable when night perception contracts',
);
assert.equal(
  isPresentMapObject({ class: 'poi', op: 'deleted', eventType: 'perception' }),
  true,
  'a remembered POI stays selectable with its rendered map marker',
);
assert.equal(
  isPresentMapObject({
    class: 'structure',
    presence: 'destroyed',
    op: 'deleted',
    eventType: 'obj_delete',
  }),
  false,
  'an explicitly removed structure cannot remain selectable',
);

const rememberedStructure: {
  class: string;
  presence?: 'perceived' | 'remembered' | 'destroyed';
  op?: string;
  eventType?: string;
} = {
  class: 'structure',
  presence: 'perceived',
  op: 'none',
  eventType: undefined,
};
markMapObjectOutsidePerception(rememberedStructure);
assert.equal(rememberedStructure.presence, 'remembered');
assert.equal(rememberedStructure.op, 'deleted');
assert.equal(rememberedStructure.eventType, 'perception');
assert.equal(
  isPresentMapObject(rememberedStructure),
  true,
  'perception loss preserves a remembered structure as a target',
);
assert.equal(
  isPerceivedMapObject(rememberedStructure),
  false,
  'a remembered structure does not contribute live perception',
);

const destroyedStructure: {
  class: string;
  presence?: 'perceived' | 'remembered' | 'destroyed';
  op?: string;
  eventType?: string;
} = {
  class: 'structure',
  presence: 'perceived',
  op: 'none',
  eventType: undefined,
};
markMapObjectDestroyed(destroyedStructure);
markMapObjectOutsidePerception(destroyedStructure);
markMapObjectPerceived(destroyedStructure);
assert.equal(destroyedStructure.presence, 'destroyed');
assert.equal(destroyedStructure.op, 'deleted');
assert.equal(destroyedStructure.eventType, 'obj_delete');
assert.equal(
  isPresentMapObject(destroyedStructure),
  false,
  'later perception transitions cannot resurrect an explicit deletion',
);
assert.equal(
  isPerceivedMapObject({
    ...destroyedStructure,
    op: 'updated',
    eventType: 'obj_update',
  }),
  false,
  'a later render operation cannot make a destroyed object a visibility source',
);

console.log('map object presence policy checks passed');
