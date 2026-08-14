import { ObjectState } from './objectState';
import { isDestroyedMapObject } from './mapObjectPresence';
import { SanctuaryZoneLookup } from './sanctuaryState';
import { Util } from './util';

export const SANCTUARY_EXIT_WARNING =
  'Leave the Sanctuary? Beyond the green border, hostile encounters can occur. Continue?';

export interface HexPosition {
  q: number;
  r: number;
}

function isLiveMonolith(objectState: ObjectState | undefined): boolean {
  return Boolean(
    objectState
      && objectState.subclass === 'monolith'
      && !isDestroyedMapObject(objectState),
  );
}

/** Uses the same strict distance < radius rule as the authoritative server. */
export function isInsideSanctuary(
  position: HexPosition,
  zones: SanctuaryZoneLookup,
  objectStates: Record<string, ObjectState>,
): boolean {
  return Object.values(zones).some((zone) => {
    const monolith = objectStates[zone.monolith_id.toString()];
    if (!isLiveMonolith(monolith)) {
      return false;
    }

    return Util.distance(
      position.q,
      position.r,
      Number(monolith.x),
      Number(monolith.y),
    ) < zone.radius;
  });
}

/** Warn once only when an attempted step actually crosses the owned Sanctuary boundary. */
export function shouldConfirmSanctuaryExit(
  current: HexPosition,
  destination: HexPosition,
  zones: SanctuaryZoneLookup,
  objectStates: Record<string, ObjectState>,
  alreadyConfirmed: boolean,
): boolean {
  return !alreadyConfirmed
    && isInsideSanctuary(current, zones, objectStates)
    && !isInsideSanctuary(destination, zones, objectStates);
}
