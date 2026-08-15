import assert from 'node:assert/strict';
import { Global } from './global';
import { Network } from './network';

class FakeEmitter {
  emit(): void {}
}

const network = new Network();
Global.gameEmitter = new FakeEmitter();
Global.heroId = '7';

function prospectTimeline(actionId: number) {
  return {
    event: 'obj_update',
    obj_id: 7,
    attrs: [
      { attr: 'state', value: 'prospecting' },
      { attr: 'action_id', value: actionId.toString() },
      { attr: 'action_duration_ms', value: '5000' },
      { attr: 'action_elapsed_ms', value: '0' },
    ],
  };
}

const stateOnlyUpdate = {
  event: 'obj_update',
  obj_id: 7,
  attrs: [{ attr: 'state', value: 'prospecting' }],
};
const completionUpdate = {
  event: 'obj_update',
  obj_id: 7,
  attrs: [{ attr: 'state', value: 'none' }],
};

for (const secondProspectEvents of [
  [prospectTimeline(102), stateOnlyUpdate],
  [stateOnlyUpdate, prospectTimeline(102)],
]) {
  Global.objectStates = {
    '7': {
      id: '7',
      state: 'none',
      presence: 'perceived',
    } as any,
  };

  network.processUpdateObjStates([prospectTimeline(101)]);
  network.processUpdateObjStates([completionUpdate]);
  assert.equal(Global.objectStates['7'].action_id, undefined);

  network.processUpdateObjStates(secondProspectEvents);

  const hero = Global.objectStates['7'];
  assert.equal(hero.state, 'prospecting');
  assert.equal(hero.action_id, 102);
  assert.equal(hero.action_duration_ms, 5000);
  assert.equal(hero.action_elapsed_ms, 0);
}

network.processUpdateObjStates([completionUpdate]);

assert.equal(Global.objectStates['7'].action_id, undefined);
assert.equal(Global.objectStates['7'].action_duration_ms, undefined);
assert.equal(Global.objectStates['7'].action_elapsed_ms, undefined);
