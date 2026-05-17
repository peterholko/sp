export const HALF_PANEL_HEIGHT_BREAKPOINT = 880;
export const HALF_PANEL_SMALL_MARGIN_TOP = -180;
export const HALF_PANEL_LARGE_MARGIN_TOP = 80;

export function getHalfPanelMarginTop(windowHeight: number = window.innerHeight): number {
  return windowHeight > HALF_PANEL_HEIGHT_BREAKPOINT
    ? HALF_PANEL_LARGE_MARGIN_TOP
    : HALF_PANEL_SMALL_MARGIN_TOP;
}

export function getHalfPanelOffsetMarginTop(offset: number, windowHeight?: number): string {
  return `${getHalfPanelMarginTop(windowHeight) + offset}px`;
}
