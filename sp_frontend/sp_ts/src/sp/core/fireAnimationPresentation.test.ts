import assert from 'node:assert/strict';
import {
  FIRE_ANIMATION_FRAME_COUNT,
  fireAnimationPresentation,
  fireAnimationStartFrame,
} from './fireAnimationPresentation';

const litCampfire = fireAnimationPresentation({
  state: 'none',
  subclass: 'campfire',
  image: 'campfirelit',
});
assert.equal(litCampfire?.kind, 'lit-campfire');
assert.ok((litCampfire?.scale || 0) < 1, 'campfire flames use a compact overlay');
assert.ok((litCampfire?.depth || 0) < 3, 'campfire flames remain behind units');
assert.equal(litCampfire?.additiveBlend, true);

assert.equal(
  fireAnimationPresentation({ state: 'none', subclass: 'campfire', image: 'campfire' }),
  null,
  'an unlit campfire has no flame animation',
);
assert.equal(
  fireAnimationPresentation(
    { state: 'none', subclass: 'campfire', image: 'campfirelit' },
    false,
  ),
  null,
  'the desktop-only campfire enhancement can be disabled without affecting state',
);
assert.equal(
  fireAnimationPresentation({ state: 'none', subclass: 'shelter', image: 'campfirelit' }),
  null,
  'an unrelated object cannot opt in by image alone',
);

const burningObject = fireAnimationPresentation({
  state: 'burning',
  subclass: 'campfire',
  image: 'campfirelit',
});
assert.equal(burningObject?.kind, 'burning-object', 'active burning state takes precedence');
assert.equal(burningObject?.scale, 1, 'existing full-object fire presentation is preserved');
assert.equal(burningObject?.additiveBlend, false);
assert.equal(
  fireAnimationPresentation({ state: 'burning', image: 'tree' }, false)?.kind,
  'burning-object',
  'desktop gating never disables the existing shared burning-object effect',
);

assert.equal(fireAnimationPresentation(null), null);

const firstStartFrame = fireAnimationStartFrame('campfire-101');
const secondStartFrame = fireAnimationStartFrame('campfire-102');
assert.ok(firstStartFrame >= 0 && firstStartFrame < FIRE_ANIMATION_FRAME_COUNT);
assert.ok(secondStartFrame >= 0 && secondStartFrame < FIRE_ANIMATION_FRAME_COUNT);
assert.equal(firstStartFrame, fireAnimationStartFrame('campfire-101'));
assert.notEqual(firstStartFrame, secondStartFrame, 'nearby campfires receive different offsets');

console.log('Fire animation presentation checks passed');
