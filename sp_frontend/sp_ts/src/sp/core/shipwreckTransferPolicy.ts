type TransferTargetState = { template: string };

/**
 * The opening Shipwreck inventory stays sealed until its one-time search has
 * completed. Other transfer targets keep their existing behavior.
 */
export function canOfferItemTransfer(
  objectState: TransferTargetState | null | undefined,
  shipwreckSearched: boolean,
): boolean {
  return objectState?.template !== 'Shipwreck' || shipwreckSearched;
}
