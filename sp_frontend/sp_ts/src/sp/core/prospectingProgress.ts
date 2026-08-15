import { ActionProgressSnapshot } from './actionProgress';
import { ObjectState } from './objectState';

const PROSPECTING_STATE = 'prospecting';

/**
 * Return the hero's authoritative prospecting timeline only for the tile that
 * hero is currently prospecting. Missing timing is deliberately not replaced
 * with a local duration: the UI waits for the authoritative object update.
 */
export function prospectingProgressForTile(
  hero: ObjectState | undefined,
  tileX: number,
  tileY: number,
): ActionProgressSnapshot | null {
  if (!hero || hero.state !== PROSPECTING_STATE || hero.x !== tileX || hero.y !== tileY) {
    return null;
  }

  if (!Number.isFinite(hero.action_id) ||
      !Number.isFinite(hero.action_duration_ms) ||
      !Number.isFinite(hero.action_elapsed_ms) ||
      hero.action_duration_ms <= 0) {
    return null;
  }

  return {
    action_id: hero.action_id,
    action_duration_ms: hero.action_duration_ms,
    action_elapsed_ms: hero.action_elapsed_ms,
  };
}
