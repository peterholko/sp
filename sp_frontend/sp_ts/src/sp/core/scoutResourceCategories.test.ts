import assert from 'node:assert/strict';

import {
  groupScoutedResourceCategories,
  resourceLayerToggleAction,
  SCOUT_CATEGORY_ICON_ASSET_SIZE,
  SCOUT_CATEGORY_ICON_OVERLAP,
  SCOUT_CATEGORY_ICON_SIZE,
  scoutCategoryIconOffset,
} from './scoutResourceCategories';

assert.equal(resourceLayerToggleAction(true, true), 'hide');
assert.equal(resourceLayerToggleAction(false, true), 'show');
assert.equal(resourceLayerToggleAction(false, false), 'request');

const groups = groupScoutedResourceCategories([
  { category: 'Game', x: 4, y: 5 },
  { category: 'Ore', x: 3, y: 4 },
  { category: 'Timber', x: 3, y: 4 },
  { category: 'Ore', x: 3, y: 4 },
  { category: 'Unknown Future Category', x: 3, y: 4 },
]);

assert.deepEqual(groups, [
  { x: 3, y: 4, categories: ['Ore', 'Timber'] },
  { x: 4, y: 5, categories: ['Game'] },
]);

assert.deepEqual(scoutCategoryIconOffset(0, 1), { x: 0, y: 0 });
assert.equal(SCOUT_CATEGORY_ICON_ASSET_SIZE, 48);
assert.equal(SCOUT_CATEGORY_ICON_SIZE, 24);

const threeIconOffsets = Array.from({ length: 3 }, (_, index) =>
  scoutCategoryIconOffset(index, 3),
);
assert.ok(
  threeIconOffsets.every(
    ({ x }) => Math.abs(x) + SCOUT_CATEGORY_ICON_SIZE / 2 <= 36,
  ),
  'three stacked category icons fit within the logical hex width',
);
assert.equal(
  SCOUT_CATEGORY_ICON_SIZE
    - (threeIconOffsets[1].x - threeIconOffsets[0].x),
  SCOUT_CATEGORY_ICON_OVERLAP,
  'adjacent icons overlap by 2.5 logical pixels, or 5 pixels at default 2x zoom',
);

const sevenIconOffsets = Array.from({ length: 7 }, (_, index) =>
  scoutCategoryIconOffset(index, 7),
);
assert.deepEqual(
  [0, 1, 2].map(
    (row) => sevenIconOffsets.filter(
      ({ y }) => y === row
        * (SCOUT_CATEGORY_ICON_SIZE - SCOUT_CATEGORY_ICON_OVERLAP),
    ).length,
  ),
  [3, 3, 1],
);
assert.ok(
  sevenIconOffsets.every(
    ({ x, y }) => Math.abs(x) + SCOUT_CATEGORY_ICON_SIZE / 2 <= 36
      && 12 + y + SCOUT_CATEGORY_ICON_SIZE / 2 <= 72,
  ),
  'the complete seven-icon stack stays within the logical hex bounds',
);
assert.equal(
  new Set(sevenIconOffsets.map(({ x, y }) => `${x},${y}`)).size,
  7,
  'each category receives a distinct icon position',
);

console.log('Scout resource category presentation checks passed');
