import assert from 'node:assert/strict';
import {
  characterImageUrl,
  DEFAULT_HERO_PORTRAIT,
  HERO_PORTRAITS,
  portraitUrl,
  VILLAGER_PORTRAITS,
} from './portraitCatalog';

assert.equal(HERO_PORTRAITS.length, 5);
assert.equal(VILLAGER_PORTRAITS.length, 6);
assert.equal(DEFAULT_HERO_PORTRAIT, HERO_PORTRAITS[0]);
assert.equal(portraitUrl(HERO_PORTRAITS[1]), '/static/art/portraits/heroes/hero-02.png');
assert.equal(characterImageUrl(undefined, 'Novice Warrior'), '/static/art/novicewarrior_single.png');

console.log('Portrait catalog checks passed');
