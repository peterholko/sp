import { BUILDING, FOUNDED, STRUCTURE } from './config';
import { ObjectState } from './objectState';

/**
 * A newly placed structure keeps the shared foundation graphic while it is
 * waiting to be built and while construction work is actively in progress.
 * Upgrade states intentionally keep the existing completed structure graphic.
 */
export function usesFoundationGraphic(
  objectState: Pick<ObjectState, 'class' | 'state'>,
): boolean {
  return objectState.class == STRUCTURE &&
    (objectState.state == FOUNDED || objectState.state == BUILDING);
}
