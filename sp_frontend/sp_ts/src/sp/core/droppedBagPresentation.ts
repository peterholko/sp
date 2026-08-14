export const DROPPED_BAG_RENDER_SCALE = 0.5;
export const SELECT_TARGET_ICON_SIZE = 72;

export function isDroppedBagObject(objectState): boolean {
  return objectState?.template == 'Dropped Bag' || objectState?.image == 'droppedbag';
}

export function droppedBagTargetImageStyle(baseStyle) {
  if (!baseStyle) return baseStyle;

  const inset = SELECT_TARGET_ICON_SIZE * (1 - DROPPED_BAG_RENDER_SCALE) / 2;
  const top = Number.parseFloat(String(baseStyle.top || 0));
  const right = Number.parseFloat(String(baseStyle.right || 0));
  const size = SELECT_TARGET_ICON_SIZE * DROPPED_BAG_RENDER_SCALE;

  return {
    ...baseStyle,
    top: (top + inset) + 'px',
    right: (right + inset) + 'px',
    width: size + 'px',
    height: size + 'px',
    objectFit: 'contain',
  };
}
