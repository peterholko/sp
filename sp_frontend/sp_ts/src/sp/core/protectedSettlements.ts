// Safe Logout ward state; live owner-sanctuary boundaries use sanctuaryState.ts.

import { ObjectState } from './objectState';

export const PROTECTED_SETTLEMENTS_VERSION = 1;

export interface ProtectedSettlement {
  player_id: number;
  monolith_id: number;
  sanctuary_radius: number;
}

export interface ProtectedSettlementsPacket {
  packet: 'protected_settlements';
  version: number;
  settlements: ProtectedSettlement[];
}

export type ProtectedSettlementLookup = Record<string, ProtectedSettlement>;

function isValidSettlement(settlement: ProtectedSettlement): boolean {
  return Number.isInteger(settlement?.player_id)
    && settlement.player_id >= 0
    && Number.isInteger(settlement?.monolith_id)
    && settlement.monolith_id >= 0
    && Number.isInteger(settlement?.sanctuary_radius)
    && settlement.sanctuary_radius > 0;
}

/**
 * Treat every server snapshot as a full replacement. This prevents a missed
 * removal or account switch from leaving a stale ward on the shared map.
 */
export function protectedSettlementLookup(
  packet: ProtectedSettlementsPacket,
): ProtectedSettlementLookup {
  if (packet?.version !== PROTECTED_SETTLEMENTS_VERSION || !Array.isArray(packet.settlements)) {
    return {};
  }

  return packet.settlements.reduce((lookup, settlement) => {
    if (isValidSettlement(settlement)) {
      lookup[settlement.player_id.toString()] = { ...settlement };
    }
    return lookup;
  }, {} as ProtectedSettlementLookup);
}

export function protectedSettlementForObject(
  objectState: ObjectState | undefined,
  settlements: ProtectedSettlementLookup,
): ProtectedSettlement | null {
  if (!objectState) {
    return null;
  }

  const ownedSettlement = settlements[Number(objectState.player).toString()];
  if (ownedSettlement) {
    return ownedSettlement;
  }

  const objectId = Number(objectState.id);
  return Object.values(settlements).find(
    (settlement) => settlement.monolith_id === objectId,
  ) || null;
}

export function isSafeLogoutProtectedObject(
  objectState: ObjectState | undefined,
  settlements: ProtectedSettlementLookup,
): boolean {
  return protectedSettlementForObject(objectState, settlements) !== null;
}
