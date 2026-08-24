import assert from 'node:assert/strict';
import { selectedObjectMovementPresentation } from './selectedObjectMovement';

const presentation = selectedObjectMovementPresentation(24, 17, 32, ['6', '24', '31']);
assert.deepEqual(presentation, {
  selectedTile: { hexX: 17, hexY: 32 },
  objIdsOnTile: ['6', '24', '31'],
  selectedBoxPos: 2,
});

const sourceIds = [6, 31];
const fallback = selectedObjectMovementPresentation(24, 18, 33, sourceIds);
assert.deepEqual(fallback, {
  selectedTile: { hexX: 18, hexY: 33 },
  objIdsOnTile: [6, 31],
  selectedBoxPos: 1,
});
assert.deepEqual(sourceIds, [6, 31], 'the source tile list must not be mutated');

console.log('Selected-object movement presentation checks passed');
