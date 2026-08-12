import { NetworkEvent } from './networkEvent';

/**
 * Perception packets that can add newly explored terrain to Global.tileStates.
 * The map scene must redraw for all three; NEW_PERCEPTION is used when vision
 * changes in place, such as equipping a torch without moving.
 */
export const MAP_RENDER_EVENTS = [
  NetworkEvent.PERCEPTION,
  NetworkEvent.NEW_PERCEPTION,
  NetworkEvent.OBJ_PERCEPTION,
] as const;
