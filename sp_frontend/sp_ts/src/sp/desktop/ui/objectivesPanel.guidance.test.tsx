import assert from 'node:assert/strict';
import * as React from 'react';
import ObjectivesPanel from './objectivesPanel';

function textContent(node: unknown): string {
  if (typeof node === 'string' || typeof node === 'number') {
    return String(node);
  }
  if (node === null || node === undefined || typeof node !== 'object') {
    return '';
  }

  const element = node as React.ReactElement;
  return React.Children.toArray(element.props?.children)
    .map((child) => textContent(child))
    .join(' ');
}

function descendants(node: unknown): React.ReactElement[] {
  if (node === null || node === undefined || typeof node !== 'object') {
    return [];
  }

  const element = node as React.ReactElement;
  return [
    element,
    ...React.Children.toArray(element.props?.children).flatMap((child) => descendants(child)),
  ];
}

(globalThis as any).window = {
  innerWidth: 1440,
  innerHeight: 900,
  screen: { width: 1920, height: 1080 },
  __SP_DESKTOP__: true,
};

const panel: any = new ObjectivesPanel({});
panel.setState = (update) => {
  const next = typeof update === 'function' ? update(panel.state, panel.props) : update;
  panel.state = { ...panel.state, ...next };
};
panel.state.objectiveState = {
  packet: 'objective_state',
  version: 1,
  current_id: 'scavenge_shipwreck',
  objectives: [
    {
      id: 'scavenge_shipwreck',
      title: 'Inspect the shipwreck',
      state: 'active',
      category: 'Introduction',
      action_hint: 'Move beside the wreck and inspect it.',
      lesson: 'The wreck holds the supplies needed for the opening encounter.',
      blocker: 'Finish the current action before inspecting the wreck.',
      reward: 'Salvaged supplies.',
    },
    {
      id: 'win_first_fight',
      title: 'Defeat the opening threat',
      state: 'locked',
      category: 'Combat',
      action_hint: 'Defeat both creatures near the wreck.',
      lesson: 'Clearing the immediate danger opens the next encounter.',
      reward: '',
    },
  ],
};

const blockedRender = panel.render();
const blockedText = textContent(blockedRender);
assert.match(
  blockedText,
  /Why:\s+The wreck holds the supplies needed for the opening encounter/,
);
assert.match(blockedText, /Next:\s+Move beside the wreck and inspect it/);
assert.match(
  blockedText,
  /Blocked:\s+Finish the current action before inspecting the wreck/,
);
assert.match(
  blockedText,
  /Defeat the opening threat/,
  'later objective rows remain visible while one recommendation is active',
);

const blockedRow = descendants(blockedRender)
  .find((node) => typeof node.props?.title === 'string'
    && node.props.title.includes('Finish the current action before inspecting the wreck.'));
assert.ok(blockedRow, 'the objective row tooltip includes the authoritative blocker');
assert.match(blockedRow.props.title, /Blocked: Finish the current action before inspecting the wreck/);

delete panel.state.objectiveState.objectives[0].blocker;
const availableRender = panel.render();
assert.doesNotMatch(textContent(availableRender), /Blocked:/);
assert.ok(
  descendants(availableRender)
    .every((node) => typeof node.props?.title !== 'string' || !node.props.title.includes('Blocked:')),
  'available objectives do not render an empty blocker label or tooltip entry',
);

panel.state.build_campfire = true;
panel.state.build_3_structures = true;
panel.state.recruit_villager = true;
panel.state.explore_poi = true;
panel.state.survive_5_nights = true;
panel.state.threatState = { pressure_level: 'high' };
panel.state.discoveryEvent = { title: 'Old run discovery' };
panel.handleRunReset();
assert.equal(panel.state.objectiveState, null, 'run reset clears the prior recommendation');
assert.equal(panel.state.threatState, null, 'run reset clears prior-run threat presentation');
assert.equal(panel.state.discoveryEvent, null, 'run reset clears prior-run discovery presentation');
assert.equal(panel.state.build_campfire, false);
assert.equal(panel.state.build_3_structures, false);
assert.equal(panel.state.recruit_villager, false);
assert.equal(panel.state.explore_poi, false);
assert.equal(panel.state.survive_5_nights, false);

console.log('ObjectivesPanel actionable guidance checks passed');
