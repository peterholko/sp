import type { ObjectState } from '../../core/objectState';

type CampfireTargetState = Pick<
  ObjectState,
  'subclass' | 'template' | 'state' | 'image'
>;

/**
 * Target actions can light any completed standalone Campfire. Ownership and
 * proximity are intentionally left to the server; protected settlements stay
 * read-only on the client as well.
 */
export function canLightCampfireTarget(
  objectState: CampfireTargetState | null | undefined,
  safeLogoutProtected: boolean,
): boolean {
  return !!objectState
    && !safeLogoutProtected
    && objectState.subclass === 'campfire'
    && objectState.template === 'Campfire'
    && objectState.state === 'none'
    && objectState.image === 'campfire';
}
