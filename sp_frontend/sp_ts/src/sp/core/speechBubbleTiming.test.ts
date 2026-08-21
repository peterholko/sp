import assert from 'node:assert/strict';

import {
  LONG_SPEECH_LIFETIME_MS,
  PerSpeakerSpeechQueue,
  SHORT_SPEECH_LIFETIME_MS,
  SPEAKER_MESSAGE_INTERVAL_MS,
  speechFadeStartOffset,
  speechLifetimeMs,
} from './speechBubbleTiming';

assert.equal(speechLifetimeMs('A short line.'), SHORT_SPEECH_LIFETIME_MS);
assert.equal(
  speechLifetimeMs('x'.repeat(60)),
  LONG_SPEECH_LIFETIME_MS,
  'messages at the threshold receive the longer reading time',
);
assert.equal(speechFadeStartOffset(SHORT_SPEECH_LIFETIME_MS), 2 / 3);
assert.equal(speechFadeStartOffset(LONG_SPEECH_LIFETIME_MS), 0.8);
assert.equal(SPEAKER_MESSAGE_INTERVAL_MS, 10000);

const queue = new PerSpeakerSpeechQueue();
assert.equal(queue.enqueue('villager-1', 'First'), 'First');
assert.equal(queue.enqueue('villager-1', 'Second'), null);
assert.equal(queue.enqueue('villager-1', 'Third'), null);
assert.equal(
  queue.enqueue('villager-2', 'Independent'),
  'Independent',
  'another villager can speak independently',
);
assert.equal(queue.advance('villager-1'), 'Second');
assert.equal(queue.advance('villager-1'), 'Third');
assert.equal(queue.advance('villager-1'), null);
assert.equal(queue.enqueue('villager-1', 'Fresh'), 'Fresh');

queue.clearSource('villager-1');
assert.equal(queue.enqueue('villager-1', 'After clear'), 'After clear');
queue.clear();
assert.equal(queue.enqueue('villager-2', 'After full clear'), 'After full clear');

console.log('Speech bubble timing and queue checks passed');
