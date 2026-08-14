import assert from 'node:assert/strict';

import {
  objectStateText,
  repeatingObjectStateText,
} from './objectStateText';

assert.equal(objectStateText('sleeping'), 'Zzzzz…');
assert.equal(
  repeatingObjectStateText('sleeping', true),
  'Zzzzz…',
  'sleep keeps its Zzz indicator even when a sleeping animation exists',
);
assert.equal(
  repeatingObjectStateText('sleeping', false),
  'Zzzzz…',
  'sleep keeps its Zzz indicator when the sprite has no sleeping animation',
);
assert.equal(
  repeatingObjectStateText('gathering', true),
  '* gathering *',
  'animated gathering retains its repeating activity label',
);
assert.equal(
  repeatingObjectStateText('gathering', true, 'Hunting'),
  '* Hunting *',
  'a hunting activity replaces the generic gathering label',
);
assert.equal(
  repeatingObjectStateText('gathering', true, 'Logging'),
  '* Logging *',
  'a logging activity replaces the generic gathering label',
);
assert.equal(
  repeatingObjectStateText('gathering', true, 'Mining'),
  '* gathering *',
  'unmapped gathering activities keep their existing label',
);
assert.equal(
  repeatingObjectStateText('drinking', false),
  '* drinking *',
  'missing animations retain the legacy state label',
);
assert.equal(repeatingObjectStateText('none', true), null);
assert.equal(repeatingObjectStateText('moving', false), null);
assert.equal(repeatingObjectStateText('dead', false), null);

console.log('Object state text checks passed');
