import type { ObjectState } from '../../core/objectState';
import { isUnlitCampfireStation } from '../../core/structureCapabilities';

type CampfireTargetState = Pick<
  ObjectState,
  'player' | 'subclass' | 'template' | 'state' | 'image'
>;

/**
 * Target actions can light any completed standalone Campfire or the current
 * player's Shelter Tent. Proximity remains server-authoritative; protected
 * settlements stay read-only on the client as well.
 */
export function canLightCampfireTarget(
  objectState: CampfireTargetState | null | undefined,
  safeLogoutProtected: boolean,
  currentPlayerId?: string,
): boolean {
  const isPublicCampfire = objectState?.template === 'Campfire'
    && objectState?.subclass === 'campfire';
  const isOwnedShelterTent = objectState?.template === 'Shelter Tent'
    && objectState?.subclass === 'shelter'
    && objectState?.player === currentPlayerId;

  return !!objectState
    && !safeLogoutProtected
    && (isPublicCampfire || isOwnedShelterTent)
    && objectState.state === 'none'
    && isUnlitCampfireStation(objectState);
}
