import assert from 'node:assert/strict';
import * as React from 'react';
import AttacksPanel from './attacksPanel';

function visibleText(node: any): string {
  if (node === null || node === undefined || typeof node === 'boolean') return '';
  if (typeof node === 'string' || typeof node === 'number') return String(node);
  if (Array.isArray(node)) return node.map(visibleText).join(' ');
  return React.Children.toArray(node.props?.children).map(visibleText).join(' ');
}

const panel = new AttacksPanel({
  attacks: [],
  combatState: {
    enemy_intent: 'Fast creature looking for an opening',
    counter_hint: 'Fast enemies reward control: quick chains toward Hamstring, while block protects low stamina.',
    available_finisher: 'Hamstring',
    attack_history: [],
    matching_combos: [],
    target_effects: [],
  },
});

const renderedText = visibleText(panel.render()).replace(/\s+/g, ' ').trim();
assert.match(renderedText, /⚔ Fast opener/);
assert.match(renderedText, /Try: Quick → Hamstring • Block low/);
assert.match(renderedText, /Ready: Hamstring/);
assert.doesNotMatch(renderedText, /Fast enemies reward control/);
assert.doesNotMatch(renderedText, /Combo ready/);

console.log('Mobile combat panel compact rendering checks passed');
