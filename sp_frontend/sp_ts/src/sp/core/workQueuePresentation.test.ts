import assert from 'node:assert/strict';
import { operateWorkPresentation } from './workQueuePresentation';

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

console.log('Work queue presentation checks passed');
