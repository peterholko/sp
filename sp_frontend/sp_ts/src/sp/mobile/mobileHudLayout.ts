import { DEAD, NPC, OBJ } from '../core/config';

export interface MobileViewport {
  width: number;
  height: number;
  portrait: boolean;
  compact: boolean;
}

export interface CompassRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export type HexDirection = 'N' | 'NW' | 'SW' | 'S' | 'SE' | 'NE';

export function mobileViewport(width: number, height: number): MobileViewport {
  const safeWidth = Math.max(0, Math.round(width));
  const safeHeight = Math.max(0, Math.round(height));

  return {
    width: safeWidth,
    height: safeHeight,
    portrait: safeHeight >= safeWidth,
    compact: safeWidth <= 360 || safeHeight <= 640,
  };
}

/**
 * Converts a pointer position into one of the six movement directions. The
 * calculation uses the rendered compass bounds, so a CSS-resized compass has
 * exactly the same hit regions as its source artwork.
 */
export function compassDirection(
  clientX: number,
  clientY: number,
  rect: CompassRect,
): HexDirection | null {
  if (rect.width <= 0 || rect.height <= 0) {
    return null;
  }

  const x = clientX - (rect.left + rect.width / 2);
  const y = clientY - (rect.top + rect.height / 2);
  const angle = ((Math.atan2(x, y) * 180) / Math.PI + 180 + 360) % 360;

  if (angle < 30 || angle >= 330) return 'N';
  if (angle < 90) return 'NW';
  if (angle < 150) return 'SW';
  if (angle < 210) return 'S';
  if (angle < 270) return 'SE';
  return 'NE';
}

/** Combat controls are useful only for a living, currently perceived NPC. */
export function shouldShowCombatControls(
  selectedKey: any,
  objectStates: Record<string | number, any>,
  combatState?: any,
): boolean {
  const selectedId = selectedKey?.type === OBJ ? selectedKey.id : undefined;
  const targetId = selectedId !== undefined ? selectedId : combatState?.target_id;
  const target = targetId !== undefined ? objectStates[targetId] : undefined;

  return Boolean(
    target
    && target.subclass === NPC
    && target.state !== DEAD
    && target.presence !== 'remembered',
  );
}
