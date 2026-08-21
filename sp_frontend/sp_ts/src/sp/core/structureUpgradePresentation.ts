export interface StructureUpgradePreviewSource {
  image?: string;
}

export interface StructureUpgradeProgressSource {
  selected_upgrade_image?: string;
}

/** Uses the template's configured art key instead of deriving one from its name. */
export function structureUpgradePreviewImageName(
  upgrade?: StructureUpgradePreviewSource | null,
): string | null {
  const image = upgrade?.image?.trim();
  return image ? image + '.png' : null;
}

/** Resolves an in-progress upgrade from the same configured template art key. */
export function structureUpgradeProgressImageName(
  structure?: StructureUpgradeProgressSource | null,
): string | null {
  return structureUpgradePreviewImageName({
    image: structure?.selected_upgrade_image,
  });
}
