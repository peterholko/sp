import type { MapObjectPresence } from './objectState';

interface MapObjectPresenceState {
  class: string;
  presence?: MapObjectPresence;
  op?: string;
  eventType?: string;
}

interface MutableMapObjectPresenceState extends MapObjectPresenceState {
  presence?: MapObjectPresence;
  op?: string;
  eventType?: string;
  updateAttr?: string;
}

export function isRememberedMapObject(
  objectState: MapObjectPresenceState | null | undefined,
): boolean {
  return objectState?.class === 'structure' || objectState?.class === 'poi';
}

export function isDestroyedMapObject(
  objectState: MapObjectPresenceState | null | undefined,
): boolean {
  return objectState?.presence === 'destroyed'
    || (objectState?.op === 'deleted' && objectState?.eventType === 'obj_delete');
}

export function isPerceivedMapObject(
  objectState: MapObjectPresenceState | null | undefined,
): boolean {
  if (!objectState || isDestroyedMapObject(objectState)) {
    return false;
  }

  if (objectState.presence) {
    return objectState.presence === 'perceived';
  }

  return objectState.op !== 'deleted';
}

export function markMapObjectPerceived(
  objectState: MutableMapObjectPresenceState,
): void {
  // Only an authoritative create or initial snapshot may replace a tombstone.
  if (isDestroyedMapObject(objectState)) {
    objectState.presence = 'destroyed';
    return;
  }

  objectState.presence = 'perceived';
}

export function markMapObjectOutsidePerception(
  objectState: MutableMapObjectPresenceState,
): void {
  // Perception contraction must never downgrade an authoritative deletion into
  // a remembered structure.
  if (isDestroyedMapObject(objectState)) {
    objectState.presence = 'destroyed';
    return;
  }

  objectState.presence = 'remembered';
  objectState.op = 'deleted';
  objectState.updateAttr = undefined;
  objectState.eventType = 'perception';
}

export function markMapObjectDestroyed(
  objectState: MutableMapObjectPresenceState,
): void {
  objectState.presence = 'destroyed';
  objectState.op = 'deleted';
  objectState.updateAttr = undefined;
  objectState.eventType = 'obj_delete';
}

/**
 * Perception loss is not object deletion. Structures and POIs remain on the
 * explored map and must stay selectable wherever their remembered sprite is
 * rendered. An explicit obj_delete packet still removes them immediately.
 */
export function isPresentMapObject(
  objectState: MapObjectPresenceState | null | undefined,
): boolean {
  if (!objectState) {
    return false;
  }

  if (isDestroyedMapObject(objectState)) {
    return false;
  }

  if (objectState.presence === 'perceived') {
    return true;
  }

  if (objectState.presence === 'remembered') {
    return isRememberedMapObject(objectState);
  }

  if (objectState.op !== 'deleted') {
    return true;
  }

  return isRememberedMapObject(objectState);
}
