export interface TerrainFeatureSummary {
  name?: string;
}

export interface TerrainFeatureTileData {
  terrain_features?: TerrainFeatureSummary[];
}

export function terrainFeatureButtonTitle(
  tileData?: TerrainFeatureTileData | null,
): string | null {
  const features = tileData?.terrain_features;
  if (!Array.isArray(features) || features.length === 0) {
    return null;
  }

  const featureName = features[0]?.name?.trim();
  return featureName ? `View ${featureName}` : 'View Special Property';
}
