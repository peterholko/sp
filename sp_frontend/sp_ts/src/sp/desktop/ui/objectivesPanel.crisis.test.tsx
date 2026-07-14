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

const status: CrisisStatusPacket = {
  packet: 'crisis_status',
  version: 1,
  exists: true,
  phase: 'assault_ready',
  warning: true,
  assault_ready: true,
  assault_active: false,
  resolved: false,
  preparation_seconds_remaining: 0,
  continues_while_disconnected: false,
};
const crisis = crisisStatusView(status);
assert.ok(crisis);

const panel: any = new ObjectivesPanel({});
const rendered = panel.renderCrisisCard(crisis, {}, {});
const renderedText = textContent(rendered);

assert.match(renderedText, /Minimum warning\s+complete/);
assert.doesNotMatch(renderedText, /Preparation time|\b0s\b/);

console.log('ObjectivesPanel crisis countdown checks passed');
