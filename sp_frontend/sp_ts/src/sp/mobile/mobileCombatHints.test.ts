import assert from 'node:assert/strict';
import { mobileCombatHints } from './mobileCombatHints';

assert.deepEqual(
  mobileCombatHints(
    'Fast creature looking for an opening',
    'Fast enemies reward control: quick chains toward Hamstring, while block protects low stamina.',
  ),
  {
    intent: 'Fast opener',
    counter: 'Quick → Hamstring • Block low',
  },
);

assert.deepEqual(
  mobileCombatHints(
    'Armored pest bracing through light attacks',
    'Armored enemies reward setup: use precise attacks before committing fierce damage.',
  ),
  {
    intent: 'Armored; resists light hits',
    counter: 'Precise setup → Fierce',
  },
);

assert.equal(
  mobileCombatHints('', 'Start with quick for control, precise for setup, fierce for damage, or block to buy time.').counter,
  'Quick / Precise / Fierce / Block',
);

const fallback = mobileCombatHints(
  'An unfamiliar enemy with an exceptionally long and previously unseen tactical description',
  'A new counter hint that is deliberately long enough to require compact fallback handling',
);
assert.ok(fallback.intent.length <= 30, 'unknown intents remain portrait-safe');
assert.ok(fallback.counter.length <= 38, 'unknown counters remain portrait-safe');
assert.match(fallback.intent, /…$/, 'truncated intent is visibly marked');
assert.match(fallback.counter, /…$/, 'truncated counter is visibly marked');

assert.deepEqual(mobileCombatHints(null, undefined), { intent: '', counter: '' });

console.log('Mobile combat hint presentation checks passed');
