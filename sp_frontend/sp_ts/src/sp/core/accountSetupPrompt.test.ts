import assert from 'node:assert/strict';

import {
  ACCOUNT_SETUP_PROMPT_DAY,
  shouldShowAccountSetupPrompt,
} from './accountSetupPrompt';

assert.equal(ACCOUNT_SETUP_PROMPT_DAY, 3);
assert.equal(shouldShowAccountSetupPrompt(1, false, false, false), false);
assert.equal(shouldShowAccountSetupPrompt(2, false, false, false), false);
assert.equal(shouldShowAccountSetupPrompt(3, false, false, false), true);
assert.equal(shouldShowAccountSetupPrompt(4, false, false, false), true);
assert.equal(shouldShowAccountSetupPrompt(3, true, false, false), false);
assert.equal(shouldShowAccountSetupPrompt(3, false, true, false), false);
assert.equal(shouldShowAccountSetupPrompt(3, false, false, true), false);
assert.equal(shouldShowAccountSetupPrompt('3', false, false, false), false);

console.log('Account setup prompt checks passed');
