import assert from 'node:assert/strict';
import {
  DESKTOP_TUTORIAL_NOTICE_MESSAGES,
  isDesktopTutorialNotice,
} from './tutorialNoticeRouting';

for (const message of DESKTOP_TUTORIAL_NOTICE_MESSAGES) {
  assert.equal(isDesktopTutorialNotice(message), true, `routes exact tutorial notice: ${message}`);
}

assert.equal(
  isDesktopTutorialNotice('A wolf howls somewhere beyond the firelight.'),
  false,
  'ordinary notices remain in the desktop notification stack',
);
assert.equal(
  isDesktopTutorialNotice(`${DESKTOP_TUTORIAL_NOTICE_MESSAGES[0]} `),
  false,
  'routing is exact and does not absorb similar messages',
);
assert.equal(isDesktopTutorialNotice(undefined), false);
assert.equal(isDesktopTutorialNotice(null), false);

console.log('Desktop tutorial notice routing checks passed');
