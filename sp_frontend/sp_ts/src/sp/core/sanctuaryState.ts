// Live owner-sanctuary boundary state; this is distinct from Safe Logout wards.

export const SANCTUARY_STATE_VERSION = 1;

export interface SanctuaryZone {
  monolith_id: number;
  radius: number;
}
export interface SanctuaryStatePacket {
  packet: 'sanctuary_state';
  version: number;
  zones: SanctuaryZone[];
}

export type SanctuaryZoneLookup = Record<string, SanctuaryZone>;

function isValidZone(zone: SanctuaryZone): boolean {
  return Number.isInteger(zone?.monolith_id)
    && zone.monolith_id >= 0
    && Number.isInteger(zone?.radius)
    && zone.radius > 0;
}

/** Every packet is a full replacement so rebinding and True Death clear stale borders. */
export function sanctuaryZoneLookup(packet: SanctuaryStatePacket): SanctuaryZoneLookup {
  if (packet?.version !== SANCTUARY_STATE_VERSION || !Array.isArray(packet.zones)) {
    return {};
  }

  return packet.zones.reduce((lookup, zone) => {
    if (isValidZone(zone)) {
      lookup[zone.monolith_id.toString()] = { ...zone };
    }
    return lookup;
  }, {} as SanctuaryZoneLookup);
}
