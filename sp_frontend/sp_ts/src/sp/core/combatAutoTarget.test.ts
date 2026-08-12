import assert from 'node:assert/strict';

import { chooseCombatAutoTarget } from './combatAutoTarget';

const damage = {
  source_id: 10,
  target_id: 100,
  state: 'dead',
};
const selectedKey = { type: 'obj', id: 100 };
const objectStates = {
  100: {
    player: 1000,
    class: 'unit',
    subclass: 'npc',
    state: 'dead',
    x: 4,
    y: 7,
    presence: 'perceived' as const,
  },
  101: {
    player: 1001,
    class: 'unit',
    subclass: 'npc',
    state: 'none',
    x: 4,
    y: 7,
    presence: 'perceived' as const,
  },
  102: {
    player: 1002,
    class: 'unit',
    subclass: 'npc',
    state: 'attacking',
    x: 4,
    y: 7,
    presence: 'perceived' as const,
  },
  103: {
    player: 1003,
    class: 'unit',
    subclass: 'npc',
    state: 'dead',
    x: 4,
    y: 7,
    presence: 'perceived' as const,
  },
  104: {
    player: 1004,
    class: 'unit',
    subclass: 'npc',
    state: 'none',
    x: 5,
    y: 7,
    presence: 'perceived' as const,
  },
  105: {
    player: 1005,
    class: 'unit',
    subclass: 'npc',
    state: 'none',
    x: 4,
    y: 7,
    presence: 'remembered' as const,
  },
  106: {
    player: 7,
    class: 'unit',
    subclass: 'villager',
    state: 'none',
    x: 4,
    y: 7,
    presence: 'perceived' as const,
  },
  107: {
    player: 1006,
    class: 'unit',
    subclass: 'merchant',
    state: 'none',
    x: 4,
    y: 7,
    presence: 'perceived' as const,
  },
};

assert.equal(
  chooseCombatAutoTarget(damage, 10, 7, selectedKey, objectStates, () => 0),
  101,
  'the first end of the random range selects the first living enemy',
);
assert.equal(
  chooseCombatAutoTarget(damage, 10, 7, selectedKey, objectStates, () => 0.999),
  102,
  'the other end of the random range selects the other living enemy',
);
assert.equal(
  chooseCombatAutoTarget(damage, 11, 7, selectedKey, objectStates, () => 0),
  null,
  'damage from someone other than this hero does not change selection',
);
assert.equal(
  chooseCombatAutoTarget(damage, 10, 7, { type: 'obj', id: 102 }, objectStates, () => 0),
  null,
  'a player selection made before the kill packet is not overridden',
);
assert.equal(
  chooseCombatAutoTarget(
    damage,
    10,
    7,
    selectedKey,
    {
      ...objectStates,
      101: { ...objectStates[101], state: 'dead' },
      102: { ...objectStates[102], state: 'dead' },
    },
    () => 0,
  ),
  null,
  'the defeated target stays selected when no living enemy remains on its tile',
);

console.log('combat auto-target policy checks passed');
