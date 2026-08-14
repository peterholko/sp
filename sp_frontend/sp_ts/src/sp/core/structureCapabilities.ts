export interface StructureCapabilitySource {
  template?: string;
  subclass?: string;
  image?: string;
}

export function isShelterStructure(structure?: StructureCapabilitySource | null): boolean {
  return structure?.subclass === 'shelter';
}

export function isCampfireStation(structure?: StructureCapabilitySource | null): boolean {
  return structure?.template === 'Campfire' || structure?.template === 'Shelter Tent';
}

export function isUnlitCampfireStation(structure?: StructureCapabilitySource | null): boolean {
  return isCampfireStation(structure) && !structure?.image?.endsWith('lit');
}
