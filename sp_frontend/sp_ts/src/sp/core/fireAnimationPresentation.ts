export type FireAnimationKind =
  | 'burning-object'
  | 'lit-campfire'
  | 'lit-shelter-tent';

export const FIRE_ANIMATION_FRAME_COUNT = 15;

export interface FireAnimationSource {
  state?: string;
  subclass?: string;
  image?: string;
}

export interface FireAnimationPresentation {
  kind: FireAnimationKind;
  offsetX: number;
  offsetY: number;
  depth: number;
  scale: number;
  alpha: number;
  additiveBlend: boolean;
}

const BURNING_OBJECT_PRESENTATION: FireAnimationPresentation = {
  kind: 'burning-object',
  offsetX: 36,
  offsetY: 36,
  depth: 10,
  scale: 1,
  alpha: 1,
  additiveBlend: false,
};

const LIT_CAMPFIRE_PRESENTATION: FireAnimationPresentation = {
  kind: 'lit-campfire',
  offsetX: 36,
  offsetY: 29,
  // Structures render at depth 1 and units at depth 3 or higher. Keep the
  // flame above the campfire stones without drawing it over a hero on the tile.
  depth: 2.5,
  scale: 0.48,
  alpha: 0.78,
  additiveBlend: true,
};

const LIT_SHELTER_TENT_PRESENTATION: FireAnimationPresentation = {
  kind: 'lit-shelter-tent',
  // The Shelter Tent's retained Campfire sits to the lower-right of the tent.
  // These values align the shared flame sheet with the fire in `tentlit.png`.
  offsetX: 53,
  offsetY: 42,
  depth: 2.5,
  scale: 0.44,
  alpha: 0.78,
  additiveBlend: true,
};

/**
 * Maps authoritative object presentation state to the shared fire animation.
 * Fire-capability `is_lit` is not part of MapObj, but the server atomically
 * swaps the image (`campfire`/`campfirelit` or `tent`/`tentlit`) whenever that
 * state changes.
 */
export function fireAnimationPresentation(
  objectState?: FireAnimationSource | null,
  litCampfireAnimationEnabled = true,
): FireAnimationPresentation | null {
  if (!objectState) {
    return null;
  }

  if (objectState.state === 'burning') {
    return BURNING_OBJECT_PRESENTATION;
  }

  if (
    litCampfireAnimationEnabled
    && objectState.subclass === 'campfire'
    && objectState.image === 'campfirelit'
  ) {
    return LIT_CAMPFIRE_PRESENTATION;
  }

  if (
    litCampfireAnimationEnabled
    && objectState.subclass === 'shelter'
    && objectState.image === 'tentlit'
  ) {
    return LIT_SHELTER_TENT_PRESENTATION;
  }

  return null;
}

/** Gives each object a stable frame offset so nearby fires do not flicker in lockstep. */
export function fireAnimationStartFrame(objectId: string | number): number {
  const key = String(objectId);
  let hash = 0;

  for (let index = 0; index < key.length; index += 1) {
    hash = ((hash * 31) + key.charCodeAt(index)) >>> 0;
  }

  return hash % FIRE_ANIMATION_FRAME_COUNT;
}
