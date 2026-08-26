import assert from 'node:assert/strict';
import {
  operateWorkPresentation,
  workQueueWorkerPresentation,
} from './workQueuePresentation';

assert.deepEqual(operateWorkPresentation('Lumbercamp'), {
  name: 'Log',
  imageName: 'log.png',
});

assert.deepEqual(operateWorkPresentation('Mine'), {
  name: 'Valleyrun Copper Ore',
  imageName: 'valleyruncopperore.png',
});

assert.deepEqual(operateWorkPresentation('Unknown Structure'), {
  name: 'Operate',
  imageName: 'recipe.png',
});

const objectStates = {
  24: {
    name: 'Seren Oakvale',
    image: 'humanvillager',
  },
};

assert.deepEqual(workQueueWorkerPresentation(24, objectStates), {
  assigned: true,
  id: 24,
  name: 'Seren Oakvale',
  image: 'humanvillager',
});

assert.deepEqual(workQueueWorkerPresentation(-1, objectStates), {
  assigned: false,
  id: null,
  name: 'Unassigned',
  image: null,
});

assert.deepEqual(workQueueWorkerPresentation(31, objectStates), {
  assigned: true,
  id: 31,
  name: 'Worker 31',
  image: null,
});

console.log('Work queue presentation checks passed');
