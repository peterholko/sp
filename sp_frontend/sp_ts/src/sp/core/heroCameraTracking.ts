import { MAP_HEX_HALF } from './mapGeometry';

export interface HeroCameraTracking {
  followOffset: number;
}

interface HeroCameraObject {
  subclass: string;
  player: string | number;
}

/**
 * Camera tracking is based on the hero's logical tile footprint, not on
 * whether its current artwork is an animated sprite or a static image.
 */
export function ownedHeroCameraTracking(
  objectState: HeroCameraObject,
  localPlayerId: string | number,
): HeroCameraTracking | null {
  if (objectState.subclass !== 'hero' ||
      String(objectState.player) !== String(localPlayerId)) {
    return null;
  }

  return {
    followOffset: -MAP_HEX_HALF,
  };
}
