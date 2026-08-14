/**
 * Place a centered badge immediately above a centered progress bar while
 * preserving an edge-to-edge gap between them.
 */
export function badgeCenterAboveProgressBar(
  progressCenterOffset: number,
  progressHeight: number,
  badgeHeight: number,
  gap: number,
): number {
  const progressTop = progressCenterOffset - (progressHeight / 2);
  return progressTop - gap - (badgeHeight / 2);
}
