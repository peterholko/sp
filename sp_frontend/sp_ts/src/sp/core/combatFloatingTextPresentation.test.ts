import assert from 'node:assert/strict';
import {
  COMBO_ANNOUNCEMENT_DEDUP_MS,
  combatFloatingTextPresentation,
  shouldAnnounceCombo,
} from './combatFloatingTextPresentation';

assert.deepEqual(
  combatFloatingTextPresentation({
    dmg: 12,
    combo: 'Intimidating Shout',
  }),
  {
    targetText: '12',
    sourceText: 'Intimidating Shout!',
  },
  'combo names and target damage are presented independently',
);

assert.deepEqual(
  combatFloatingTextPresentation({ dmg: 0, combo: 'Intimidating Shout' }),
  {
    targetText: '0',
    sourceText: 'Intimidating Shout!',
  },
  'effect-only secondary targets retain their own zero-damage result',
);

assert.deepEqual(
  combatFloatingTextPresentation({ dmg: 0, missed: true }),
  { targetText: 'Miss', sourceText: undefined },
  'miss text remains anchored to the target',
);

const announcementTimes = new Map<string, number>();
assert.equal(
  shouldAnnounceCombo(announcementTimes, 7, 'Intimidating Shout', 1000),
  true,
  'the primary damage packet announces the combo',
);
assert.equal(
  shouldAnnounceCombo(announcementTimes, 7, 'Intimidating Shout', 1100),
  false,
  'secondary target packets do not repeat the same announcement',
);
assert.equal(
  shouldAnnounceCombo(announcementTimes, 8, 'Intimidating Shout', 1100),
  true,
  'another hero may announce the same combo independently',
);
assert.equal(
  shouldAnnounceCombo(
    announcementTimes,
    7,
    'Intimidating Shout',
    1000 + COMBO_ANNOUNCEMENT_DEDUP_MS,
  ),
  true,
  'a later execution of the combo is announced again',
);

console.log('Combat floating-text presentation checks passed');
