export const BUILD_BURROW_OBJECTIVE_ID = 'build_burrow';
export const SHIPWRECK_TEMPLATE = 'Shipwreck';
export const BURROW_LOG_ITEM = 'Log';

export function shouldHighlightBurrowLogs(
  currentObjectiveId: string,
  ownerState: any,
  item: any,
): boolean {
  return currentObjectiveId === BUILD_BURROW_OBJECTIVE_ID
    && ownerState?.template === SHIPWRECK_TEMPLATE
    && item?.name === BURROW_LOG_ITEM
    && Number(item?.quantity) > 0;
}
