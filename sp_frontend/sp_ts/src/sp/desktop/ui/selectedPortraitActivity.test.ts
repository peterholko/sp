import assert from 'node:assert/strict';

import { ObjectState } from '../../core/objectState';
import {
  SELECTED_PORTRAIT_ACTIVITY_BADGE_BORDER,
  SELECTED_PORTRAIT_ACTIVITY_BADGE_SIZE,
  SELECTED_PORTRAIT_ACTIVITY_ICON_SIZE,
  selectedPortraitActivityBadge,
} from './selectedPortraitActivity';

assert.equal(SELECTED_PORTRAIT_ACTIVITY_BADGE_SIZE, 28);
assert.equal(SELECTED_PORTRAIT_ACTIVITY_BADGE_BORDER, 2);
assert.equal(SELECTED_PORTRAIT_ACTIVITY_ICON_SIZE, 24);
assert.equal(
  SELECTED_PORTRAIT_ACTIVITY_BADGE_SIZE
    - (SELECTED_PORTRAIT_ACTIVITY_BADGE_BORDER * 2),
  SELECTED_PORTRAIT_ACTIVITY_ICON_SIZE,
  'the activity artwork fills the badge interior',
);

const villager = {
  id: '17',
  subclass: 'villager',
  state: 'gathering',
  activity: 'Logging',
} as ObjectState;

assert.deepEqual(selectedPortraitActivityBadge(villager), {
  icon: 'logging',
  label: 'Activity: Logging',
});
assert.equal(
  selectedPortraitActivityBadge({ ...villager, state: 'none' }, ''),
  null,
  'an idle villager does not retain a stale activity badge',
);
assert.equal(
  selectedPortraitActivityBadge({ ...villager, subclass: 'hero' }),
  null,
  'the selected portrait badge remains villager-only',
);

console.log('Selected portrait activity checks passed');
