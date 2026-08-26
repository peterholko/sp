export interface StructureCapabilitySource {
  template?: string;
  subclass?: string;
  image?: string;
  state?: string;
  workspaces?: number;
}

const ASSIGNABLE_CONSTRUCTION_STATES = new Set([
  'founded',
  'progressing',
  'building',
  'planning_upgrade',
  'upgrading',
  'stalled',
]);

export function isShelterStructure(structure?: StructureCapabilitySource | null): boolean {
  return structure?.subclass === 'shelter';
}

export function isCampfireStation(structure?: StructureCapabilitySource | null): boolean {
  return structure?.template === 'Campfire' || structure?.template === 'Shelter Tent';
}

export function isUnlitCampfireStation(structure?: StructureCapabilitySource | null): boolean {
  return isCampfireStation(structure) && !structure?.image?.endsWith('lit');
}

/**
 * Construction can accept temporary builders. Once construction is complete,
 * only worker-capable structures can accept assignments.
 */
export function canAssignWorkersToStructure(
  structure?: StructureCapabilitySource | null,
): boolean {
  if (!structure || structure.state === 'dead') return false;

  return (structure.workspaces ?? 0) > 0
    || structure.subclass === 'craft'
    || structure.subclass === 'resource'
    || ASSIGNABLE_CONSTRUCTION_STATES.has(structure.state ?? '');
}
