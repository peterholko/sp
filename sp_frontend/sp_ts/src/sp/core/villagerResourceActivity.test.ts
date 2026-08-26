import assert from 'node:assert/strict';

import { villagerActivityIcon } from './villagerResourceActivity';

const iconByState = {
  gathering: 'gathering',
  foraging: 'gathering',
  harvesting: 'gathering',
  planting: 'gathering',
  tending: 'gathering',
  logging: 'logging',
  lumberjacking: 'logging',
  mining: 'mining',
  hunting: 'hunting',
  fishing: 'fishing',
  building: 'building',
  upgrading: 'building',
  repairing: 'building',
  eating: 'eating',
  drinking: 'drinking',
  sleeping: 'sleeping',
} as const;

for (const [state, icon] of Object.entries(iconByState)) {
  assert.equal(
    villagerActivityIcon({ subclass: 'villager', state }),
    icon,
    `${state} uses the ${icon} icon`,
  );
}

const workflowIconByActivity = {
  Following: 'following',
  'Getting tool': 'fetching-tool',
  'Fetching tool': 'fetching-tool',
  'Fetching Mining tool': 'fetching-tool',
  'Fetching Logging tool': 'fetching-tool',
  Hauling: 'hauling',
  Unloading: 'hauling',
} as const;

for (const [activity, icon] of Object.entries(workflowIconByActivity)) {
  assert.equal(
    villagerActivityIcon({ subclass: 'villager', state: 'moving', activity }),
    icon,
    `${activity} uses the ${icon} icon while the villager moves`,
  );
}

for (const state of [
  'none',
  'moving',
  'crafting',
  'refining',
  'surveying',
  'prospecting',
]) {
  assert.equal(
    villagerActivityIcon({ subclass: 'villager', state }),
    null,
    `${state} has no matching activity icon`,
  );
}

assert.equal(
  villagerActivityIcon({ subclass: 'hero', state: 'gathering' }),
  null,
  'the activity badge is villager-only',
);
assert.equal(
  villagerActivityIcon({ subclass: 'hero', state: 'moving', activity: 'Following' }),
  null,
  'workflow activity badges remain villager-only',
);
assert.equal(
  villagerActivityIcon({ subclass: 'villager', state: 'gathering', activity: 'Hunting' }),
  'hunting',
  'hunting overrides the generic gathering state',
);
assert.equal(
  villagerActivityIcon({ subclass: 'villager', state: 'gathering', activity: 'Logging' }),
  'logging',
  'logging overrides the generic gathering state for tree gathering',
);
assert.equal(
  villagerActivityIcon({ subclass: 'villager', state: 'gathering', activity: 'Stonecutting' }),
  'mining',
  'stonecutting uses the closest matching extraction icon',
);
assert.equal(
  villagerActivityIcon({ subclass: 'villager', state: 'gathering', activity: 'Foraging' }),
  'gathering',
  'foraging keeps the general gathering icon while using an explicit activity label',
);
assert.equal(
  villagerActivityIcon({ subclass: 'villager', state: 'none', activity: 'Hunting' }),
  null,
  'stale activity text cannot keep an idle villager badge visible',
);
assert.equal(villagerActivityIcon(null), null);

console.log('Villager activity icon checks passed');
