export interface StructureUpgradePreviewSource {
  image?: string;
}

export interface StructureUpgradeOption extends StructureUpgradePreviewSource {
  name?: string;
  req?: any[];
}

export interface StructureUpgradeListSource {
  upgrade_list?: StructureUpgradeOption[];
}

export const NO_LEARNED_STRUCTURE_UPGRADES =
  'No learned upgrades are available for this structure. Use the required deed first.';

/** Normalizes an upgrade response before a panel tries to select its first option. */
export function structureUpgradeOptions(
  source?: StructureUpgradeListSource | null,
): StructureUpgradeOption[] {
  return Array.isArray(source?.upgrade_list) ? source.upgrade_list : [];
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
