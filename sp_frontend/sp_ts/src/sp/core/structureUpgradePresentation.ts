export interface StructureUpgradePreviewSource {
  image?: string;
}

/** Uses the template's configured art key instead of deriving one from its name. */
export function structureUpgradePreviewImageName(
  upgrade?: StructureUpgradePreviewSource | null,
): string | null {
  const image = upgrade?.image?.trim();
  return image ? image + '.png' : null;
}
