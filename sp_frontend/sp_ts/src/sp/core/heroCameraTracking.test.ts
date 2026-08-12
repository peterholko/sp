import assert from 'node:assert/strict';
import { ownedHeroCameraTracking } from './heroCameraTracking';

assert.deepEqual(
  ownedHeroCameraTracking({ subclass: 'hero', player: '250' }, 250),
  { followOffset: -72 },
  'the owned hero uses its logical 144px footprint regardless of render type',
);

assert.equal(
  ownedHeroCameraTracking({ subclass: 'hero', player: '251' }, 250),
  null,
  'another player hero must not take over the local camera',
);

assert.equal(
  ownedHeroCameraTracking({ subclass: 'villager', player: '250' }, 250),
  null,
  'owned non-hero units must not take over the local camera',
);
