import assert from 'node:assert/strict';
import { canOfferItemTransfer } from './shipwreckTransferPolicy';

assert.equal(
  canOfferItemTransfer({ template: 'Shipwreck' }, false),
  false,
  'an unsearched Shipwreck must not offer Item Transfer',
);
assert.equal(
  canOfferItemTransfer({ template: 'Shipwreck' }, true),
  true,
  'a searched Shipwreck offers Item Transfer',
);
assert.equal(
  canOfferItemTransfer({ template: 'Supply Cache' }, false),
  true,
  'other POIs retain their existing transfer action',
);
assert.equal(canOfferItemTransfer(undefined, false), true);

console.log('Shipwreck transfer action policy checks passed');
