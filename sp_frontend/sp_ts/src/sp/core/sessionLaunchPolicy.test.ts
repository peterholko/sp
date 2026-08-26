import assert from 'node:assert/strict';

import { sessionLaunchDestination } from './sessionLaunchPolicy';

assert.equal(sessionLaunchDestination(false, false), 'game');
assert.equal(sessionLaunchDestination(false, true), 'game');
assert.equal(sessionLaunchDestination(true, false), 'landing');
assert.equal(sessionLaunchDestination(true, true), 'hero-creation');

console.log('Session launch policy checks passed');
