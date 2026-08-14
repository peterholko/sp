import { DEAD, NPC, OBJ, UNIT } from './config';
import { isPerceivedMapObject } from './mapObjectPresence';

interface DamageMessage {
  source_id: string | number;
  target_id: string | number;
  state: string;
  missed?: boolean;
}

interface SelectedObjectKey {
  type?: string;
  id?: string | number;
}

interface CombatTargetState {
  player: string | number;
  class: string;
  subclass: string;
  state: string;
  x: number;
  y: number;
  presence?: 'perceived' | 'remembered' | 'destroyed';
  op?: string;
  eventType?: string;
}

type CombatTargetStates = Record<string, CombatTargetState>;

function sameId(left: string | number | undefined, right: string | number): boolean {
  return left !== undefined && Number(left) === Number(right);
}

/**
 * Chooses the next hostile portrait after the player's hero kills its selected
 * target. This is a UI selection only; the server still validates every attack.
 */
export function chooseCombatAutoTarget(
  damage: DamageMessage,
  heroId: string | number,
  playerId: string | number,
  selectedKey: SelectedObjectKey | null | undefined,
  objectStates: CombatTargetStates,
  random: () => number = Math.random,
): number | null {
  if (
    damage.missed
    || damage.state !== DEAD
    || !sameId(damage.source_id, heroId)
    || selectedKey?.type !== OBJ
    || !sameId(selectedKey.id, damage.target_id)
  ) {
    return null;
  }

  const defeatedTarget = objectStates[String(damage.target_id)];
  if (
    !defeatedTarget
    || defeatedTarget.class !== UNIT
    || defeatedTarget.subclass !== NPC
  ) {
    return null;
  }

  const candidates = Object.entries(objectStates)
    .filter(([id, candidate]) => (
      !sameId(id, damage.target_id)
      && candidate.x === defeatedTarget.x
      && candidate.y === defeatedTarget.y
      && candidate.class === UNIT
      && candidate.subclass === NPC
      && candidate.state !== DEAD
      && Number(candidate.player) !== Number(playerId)
      && isPerceivedMapObject(candidate)
    ))
    .map(([id]) => Number(id))
    .filter(Number.isFinite)
    .sort((left, right) => left - right);

  if (candidates.length === 0) {
    return null;
  }

  const randomValue = Math.min(0.999999999999, Math.max(0, random()));
  return candidates[Math.floor(randomValue * candidates.length)];
}
