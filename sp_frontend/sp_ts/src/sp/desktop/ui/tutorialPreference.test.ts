import assert from 'node:assert/strict';
import {
  loadTutorialEnabled,
  saveTutorialEnabled,
  tutorialPreferenceStorageKey,
  TutorialPreferenceStorage,
} from './tutorialPreference';

class MemoryStorage implements TutorialPreferenceStorage {
  values = new Map<string, string>();

  getItem(key: string): string | null {
    return this.values.has(key) ? this.values.get(key)! : null;
  }

  setItem(key: string, value: string): void {
    this.values.set(key, value);
  }
}

const storage = new MemoryStorage();
assert.equal(loadTutorialEnabled('player-a', storage), true, 'missing preference defaults on');

assert.equal(saveTutorialEnabled('player-a', false, storage), true);
assert.equal(loadTutorialEnabled('player-a', storage), false, 'saved off preference is restored');
assert.equal(loadTutorialEnabled('player-b', storage), true, 'preferences are scoped by player');
assert.notEqual(
  tutorialPreferenceStorageKey('player-a'),
  tutorialPreferenceStorageKey('player-b'),
  'different players receive different storage keys',
);
assert.match(
  tutorialPreferenceStorageKey('player-a'),
  /\.v1\./,
  'the storage key carries the preference schema version',
);

assert.equal(saveTutorialEnabled('player-b', true, storage), true);
assert.equal(loadTutorialEnabled('player-b', storage), true);

storage.values.set(tutorialPreferenceStorageKey('malformed'), '{"enabled":false}');
assert.equal(loadTutorialEnabled('malformed', storage), true, 'malformed data safely defaults on');

const readFailure: TutorialPreferenceStorage = {
  getItem: () => { throw new Error('storage read denied'); },
  setItem: () => {},
};
assert.equal(loadTutorialEnabled('player-a', readFailure), true, 'read errors default on');
assert.equal(loadTutorialEnabled('player-a', null), true, 'unavailable storage defaults on');

const writeFailure: TutorialPreferenceStorage = {
  getItem: () => null,
  setItem: () => { throw new Error('storage write denied'); },
};
assert.equal(saveTutorialEnabled('player-a', false, writeFailure), false, 'write errors are safe');
assert.equal(saveTutorialEnabled('player-a', false, null), false, 'unavailable storage is safe');

console.log('Desktop tutorial preference checks passed');
