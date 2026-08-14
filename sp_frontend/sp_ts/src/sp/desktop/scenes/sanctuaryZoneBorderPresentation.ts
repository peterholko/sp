// Live owner-sanctuary border presentation; this is distinct from the Safe Logout ward.

import { ObjectState } from '../../core/objectState';
import { isDestroyedMapObject } from '../../core/mapObjectPresence';
import { ProtectedSettlementLookup } from '../../core/protectedSettlements';
import {
  SanctuaryZone,
  SanctuaryZoneLookup,
} from '../../core/sanctuaryState';
import {
  WardSegment,
  sanctuaryWardSegments,
} from './sanctuaryWardGeometry';

export interface SanctuaryZoneBorderPresentation {
  zone: SanctuaryZone;
  segments: WardSegment[];
}

export function sanctuaryZoneBorderPresentation(
  objectState: ObjectState | undefined,
  zones: SanctuaryZoneLookup,
  protectedSettlements: ProtectedSettlementLookup,
  borderVisible = false,
): SanctuaryZoneBorderPresentation | null {
  if (
    !borderVisible
    || !objectState
    || objectState.subclass !== 'monolith'
    || isDestroyedMapObject(objectState)
  ) {
    return null;
  }

  const monolithId = Number(objectState.id);
  const zone = zones[monolithId.toString()];
  if (!zone || zone.monolith_id !== monolithId) {
    return null;
  }

  if (Object.values(protectedSettlements).some(
    (settlement) => settlement.monolith_id === monolithId,
  )) {
    return null;
  }

  return {
    zone,
    segments: sanctuaryWardSegments(
      Number(objectState.x),
      Number(objectState.y),
      zone.radius,
    ),
  };
}
