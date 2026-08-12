import { shouldHighlightBurrowLogs } from './tutorialInventoryHighlight';

describe('tutorial inventory highlight policy', () => {
  const shipwreck = { template: 'Shipwreck' };
  const logs = { name: 'Log', quantity: 5 };

  test('highlights Shipwreck Logs only while Build Burrow is current', () => {
    expect(shouldHighlightBurrowLogs('build_burrow', shipwreck, logs)).toBe(true);
    expect(shouldHighlightBurrowLogs('defeat_opening_threat', shipwreck, logs)).toBe(false);
    expect(shouldHighlightBurrowLogs('transfer_supplies', shipwreck, logs)).toBe(false);
  });

  test('does not highlight other items, owners, or empty Log stacks', () => {
    expect(shouldHighlightBurrowLogs('build_burrow', shipwreck, { name: 'Skin', quantity: 1 })).toBe(false);
    expect(shouldHighlightBurrowLogs('build_burrow', { template: 'Burrow' }, logs)).toBe(false);
    expect(shouldHighlightBurrowLogs('build_burrow', shipwreck, { name: 'Log', quantity: 0 })).toBe(false);
  });
});
