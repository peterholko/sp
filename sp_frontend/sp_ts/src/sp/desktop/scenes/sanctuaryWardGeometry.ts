// Safe Logout ward geometry; live sanctuary boundaries use this perimeter helper separately.

import { ObjectState } from '../../core/objectState';
import { isPerceivedMapObject } from '../../core/mapObjectPresence';
import {
  ProtectedSettlement,
  ProtectedSettlementLookup,
  protectedSettlementForObject,
} from '../../core/protectedSettlements';
import { Util } from '../../core/util';
import {
  MAP_HEX_HALF,
  MAP_HEX_QUARTER,
  MAP_HEX_SIZE,
  MAP_HEX_THREE_QUARTERS,
} from '../../core/mapGeometry';

export interface WardPoint {
  x: number;
  y: number;
}

export interface WardSegment {
  start: WardPoint;
  end: WardPoint;
}

export interface SanctuaryWardPresentation {
  settlement: ProtectedSettlement;
  center: WardPoint;
  segments: WardSegment[];
}

const HEX_VERTICES: WardPoint[] = [
  { x: MAP_HEX_QUARTER, y: 0 },
  { x: MAP_HEX_THREE_QUARTERS, y: 0 },
  { x: MAP_HEX_SIZE, y: MAP_HEX_HALF },
  { x: MAP_HEX_THREE_QUARTERS, y: MAP_HEX_SIZE },
  { x: MAP_HEX_QUARTER, y: MAP_HEX_SIZE },
  { x: 0, y: MAP_HEX_HALF },
];

function pointKey(point: WardPoint): string {
  return `${point.x},${point.y}`;
}

function edgeKey(start: WardPoint, end: WardPoint): string {
  const first = pointKey(start);
  const second = pointKey(end);
  return first < second ? `${first}|${second}` : `${second}|${first}`;
}

/**
 * The server sanctuary predicate is `distance < radius`. Build the exact
 * union of those hexes, then cancel shared edges so only its perimeter remains.
 */
export function sanctuaryWardSegments(
  anchorQ: number,
  anchorR: number,
  sanctuaryRadius: number,
): WardSegment[] {
  const radius = Math.max(1, Math.floor(sanctuaryRadius));
  const edges = new Map<string, WardSegment>();

  for (const cell of Util.range(anchorQ, anchorR, radius - 1)) {
    const origin = Util.hex_to_pixel(cell.q, cell.r);
    const vertices = HEX_VERTICES.map((vertex) => ({
      x: origin.x + vertex.x,
      y: origin.y + vertex.y,
    }));

    for (let index = 0; index < vertices.length; index += 1) {
      const segment = {
        start: vertices[index],
        end: vertices[(index + 1) % vertices.length],
      };
      const key = edgeKey(segment.start, segment.end);
      if (edges.has(key)) {
        edges.delete(key);
      } else {
        edges.set(key, segment);
      }
    }
  }

  return Array.from(edges.entries())
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([, segment]) => segment);
}

export function sanctuaryWardPresentation(
  objectState: ObjectState | undefined,
  settlements: ProtectedSettlementLookup,
): SanctuaryWardPresentation | null {
  if (
    !objectState
    || objectState.subclass !== 'monolith'
    || !isPerceivedMapObject(objectState)
  ) {
    return null;
  }

  const settlement = protectedSettlementForObject(objectState, settlements);
  if (!settlement || settlement.monolith_id !== Number(objectState.id)) {
    return null;
  }

  const origin = Util.hex_to_pixel(Number(objectState.x), Number(objectState.y));
  return {
    settlement,
    center: { x: origin.x + MAP_HEX_HALF, y: origin.y + MAP_HEX_HALF },
    segments: sanctuaryWardSegments(
      Number(objectState.x),
      Number(objectState.y),
      settlement.sanctuary_radius,
    ),
  };
}
