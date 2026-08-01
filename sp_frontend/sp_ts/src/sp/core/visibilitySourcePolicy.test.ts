import assert from 'node:assert/strict';
import {
  isVisibilitySource,
  mergedIncrementalVision,
  needsZeroVisionShroud,
  resetVisibilitySourceForInit,
} from './visibilitySourcePolicy';

assert.equal(
  isVisibilitySource({ player: '7', vision: 2 }, '7'),
  true,
  'owned observers preserve their existing visibility behavior',
);
assert.equal(
  isVisibilitySource({ player: '9', vision: 1, perceptionObserver: true }, '7'),
  true,
  'a foreign server-designated observer contributes visibility',
);
assert.equal(
  isVisibilitySource({ player: '9', vision: 1, perceptionObserver: false }, '7'),
  false,
  'a merely visible foreign object cannot reveal tiles',
);
assert.equal(
  isVisibilitySource({ player: '9', vision: 0, perceptionObserver: true }, '7'),
  false,
  'an observer with no vision does not reveal a tile',
);
assert.equal(isVisibilitySource(undefined, '7'), false);

assert.equal(
  mergedIncrementalVision({ vision: 1, perceptionObserver: true }, null),
  1,
  'an incremental visible-object update preserves active Campfire observer range',
);
assert.equal(
  mergedIncrementalVision({ vision: 1, perceptionObserver: true }, 2),
  2,
  'an explicit observer range remains authoritative',
);
assert.equal(
  mergedIncrementalVision({ vision: 1, perceptionObserver: false }, null),
  null,
  'ordinary visible objects retain the non-observer sentinel',
);

assert.equal(
  needsZeroVisionShroud({ vision: 0 }, false),
  true,
  'an isolated zero-range observer keeps the single-tile shroud edge',
);
assert.equal(
  needsZeroVisionShroud({ vision: 0 }, true),
  false,
  'a Campfire or other positive-range source suppresses overlapping shroud',
);
assert.equal(
  needsZeroVisionShroud({ vision: null }, false),
  false,
  'ordinary visible objects do not create single-tile shroud',
);

const staleCampfireObserver = { vision: 1, perceptionObserver: true };
resetVisibilitySourceForInit(staleCampfireObserver);
assert.deepEqual(
  staleCampfireObserver,
  { vision: null, perceptionObserver: false },
  'a reconnect cannot retain a Campfire observer omitted from the new init snapshot',
);

console.log('Visibility source policy checks passed');
