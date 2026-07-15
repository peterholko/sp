import assert from 'node:assert/strict';
import * as React from 'react';
import { CrisisStatusPacket, crisisStatusView } from '../../core/crisisStatus';
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

const status: CrisisStatusPacket = {
  packet: 'crisis_status',
  version: 1,
  exists: true,
  kind: 'goblin',
  phase: 'assault_ready',
  warning: true,
  assault_ready: true,
  assault_active: false,
  resolved: false,
  preparation_seconds_remaining: 0,
  continues_while_disconnected: false,
  preparation_options: [
    {
      id: 'defences',
      label: 'Defences',
      state: 'needs_attention',
      detail: 'Two defensive structures are damaged.',
      action_hint: 'Repair walls before the raid begins.',
    },
    {
      id: 'defenders',
      label: 'Defenders',
      state: 'ready',
      detail: 'One combat-capable villager is available.',
      action_hint: 'Keep the defender near the settlement.',
    },
    {
      id: 'equipment',
      label: 'Equipment',
      state: 'unavailable',
      detail: 'No carried armor can be equipped.',
      action_hint: '',
    },
  ],
};
const crisis = crisisStatusView(status);
assert.ok(crisis);

const panel: any = new ObjectivesPanel({});
const rendered = panel.renderCrisisCard(crisis, {}, {});
const renderedText = textContent(rendered);

assert.equal(rendered.props['aria-label'], 'Personal goblin crisis status');
assert.match(renderedText, /Minimum warning\s+complete/);
assert.doesNotMatch(renderedText, /Preparation time|\b0s\b/);
assert.match(renderedText, /Prepare your settlement/);
assert.match(renderedText, /Defences\s+Needs attention/);
assert.match(renderedText, /Two defensive structures are damaged/);
assert.match(renderedText, /Action:\s+Repair walls before the raid begins/);
assert.match(renderedText, /Defenders\s+Ready/);
assert.match(renderedText, /Equipment\s+Unavailable/);

const active = crisisStatusView({
  ...status,
  phase: 'assault_active',
  assault_ready: false,
  assault_active: true,
  preparation_seconds_remaining: undefined,
  remaining_attackers: 2,
  total_attackers: 3,
  continues_while_disconnected: true,
});
assert.ok(active);
const activeText = textContent(panel.renderCrisisCard(active, {}, {}));
assert.doesNotMatch(activeText, /Prepare your settlement|Two defensive structures are damaged/);
assert.match(activeText, /Attackers remaining\s+2 \/ 3/);
assert.match(activeText, /The assault continues while disconnected/);

const undead = crisisStatusView({
  ...status,
  kind: 'undead',
  phase: 'assault_ready',
  title: 'Undead Incursion Imminent',
  summary: 'The dead are gathering near your settlement.',
  action_hint: 'Return to the settlement and finish preparing.',
  pressure: 80,
  pressure_max: 100,
  preparation_seconds_remaining: undefined,
});
assert.ok(undead);
const undeadRendered = panel.renderCrisisCard(undead, {}, {});
const undeadText = textContent(undeadRendered);
const undeadPressure = descendants(undeadRendered)
  .find((node) => node.props?.role === 'progressbar');
assert.equal(undeadRendered.props['aria-label'], 'Personal undead crisis status');
assert.equal(undeadPressure?.props['aria-label'], 'Undead crisis pressure');
assert.equal(undeadPressure?.props.title, 'Undead crisis pressure: 80 of 100');
assert.match(undeadText, /Undead Incursion Imminent/);
assert.doesNotMatch(undeadText, /Goblin|goblin|Raiders/);

console.log('ObjectivesPanel crisis countdown checks passed');
