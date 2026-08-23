import { ActionProgressSnapshot } from './actionProgress';
import { ObjectState } from './objectState';

const REFINING_STATE = 'refining';

/**
 * Return the hero's authoritative refining timeline only for the item whose
 * action was started from the open item panel. The server remains responsible
 * for the duration and elapsed time; the item id only scopes presentation.
 */
export function refiningProgressForItem(
  hero: ObjectState | undefined,
  itemId: unknown,
  activeRefineItemId: unknown,
): ActionProgressSnapshot | null {
  if (itemId === null || itemId === undefined ||
      activeRefineItemId === null || activeRefineItemId === undefined) {
    return null;
  }

  const selectedId = Number(itemId);
  const activeId = Number(activeRefineItemId);
  if (!hero || hero.state !== REFINING_STATE ||
      !Number.isFinite(selectedId) || !Number.isFinite(activeId) ||
      selectedId !== activeId) {
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
