import assert from 'node:assert/strict';

import { badgeCenterAboveProgressBar } from './villagerActivityLayout';

const progressCenter = -20;
const progressHeight = 5;
const badgeHeight = 28;
const badgeCenter = badgeCenterAboveProgressBar(
  progressCenter,
  progressHeight,
  badgeHeight,
  2,
);
const progressTop = progressCenter - (progressHeight / 2);
const badgeBottom = badgeCenter + (badgeHeight / 2);

assert.equal(progressTop - badgeBottom, 2);
assert.equal(badgeCenter, -38.5);

console.log('Villager activity layout checks passed');
