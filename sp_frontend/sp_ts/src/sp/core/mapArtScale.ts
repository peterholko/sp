export interface MapArtDefinition {
  /**
   * Converts source-art pixels into the logical 144px map footprint.
   * Native 144px art uses 1. Legacy 72px art omits this field and is enlarged
   * 2x so unmigrated mid/late-game content remains usable on the larger grid.
   */
  map_scale?: unknown;
}

export interface MapArtPresentation {
  /** Final Phaser scale after asset and presentation scaling are composed. */
  scale: number;
  /** World-space inset that keeps a deliberately smaller presentation centred. */
  insetX: number;
  insetY: number;
}

export const LEGACY_MAP_ART_SCALE = 2;
export const NATIVE_144_MAP_ART_SCALE = 1;

const NATIVE_144_CORE_MAP_IMAGES = new Set([
  'foundation',
  'gravestone',
]);

function positiveFiniteNumber(value: unknown, fallback: number): number {
  const parsed = typeof value === 'number' ? value : Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

/**
 * Reads the explicit authoring scale without inferring intent from dimensions.
 * Existing 72px definitions generally omit map_scale and are enlarged 2x.
 */
export function mapArtScale(definition?: MapArtDefinition | null): number {
  return positiveFiniteNumber(definition?.map_scale, LEGACY_MAP_ART_SCALE);
}

/**
 * Preloaded fallback textures do not receive an image-definition packet from
 * the server. Resolve their native-art scale locally while continuing to
 * prefer authoritative definitions for normal units and structures.
 */
export function mapArtDefinitionForImage(
  imageName: string,
  definition?: MapArtDefinition | null,
): MapArtDefinition | null | undefined {
  if (definition?.map_scale != null) {
    return definition;
  }

  if (NATIVE_144_CORE_MAP_IMAGES.has(imageName)) {
    return { map_scale: NATIVE_144_MAP_ART_SCALE };
  }

  return definition;
}

/**
 * Composes source-art scaling with an object's gameplay presentation scale.
 * The inset is based on the logical footprint after map_scale is applied. This
 * keeps special presentations such as dropped bags centred in a 144px tile.
 */
export function mapArtPresentation(
  definition: MapArtDefinition | null | undefined,
  sourceWidth: number,
  sourceHeight: number,
  presentationScale = 1,
): MapArtPresentation {
  const assetScale = mapArtScale(definition);
  const objectScale = positiveFiniteNumber(presentationScale, 1);
  const logicalWidth = Math.max(0, sourceWidth) * assetScale;
  const logicalHeight = Math.max(0, sourceHeight) * assetScale;

  return {
    scale: assetScale * objectScale,
    insetX: logicalWidth * (1 - objectScale) / 2,
    insetY: logicalHeight * (1 - objectScale) / 2,
  };
}
