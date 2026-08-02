// Safe Logout ward geometry; live sanctuary boundaries use this perimeter helper separately.

import { ObjectState } from '../../core/objectState';
import {
  ProtectedSettlement,
  ProtectedSettlementLookup,
  protectedSettlementForObject,
} from '../../core/protectedSettlements';
import { Util } from '../../core/util';

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
  { x: 18, y: 0 },
  { x: 54, y: 0 },
  { x: 72, y: 36 },
  { x: 54, y: 72 },
  { x: 18, y: 72 },
  { x: 0, y: 36 },
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
 * The server sanctuary predicate is `distance < full_radius`. Build the exact
 * union of those hexes, then cancel shared edges so only its perimeter remains.
 */
export function sanctuaryWardSegments(
  anchorQ: number,
  anchorR: number,
  fullRadius: number,
): WardSegment[] {
  const radius = Math.max(1, Math.floor(fullRadius));
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
  if (!objectState || objectState.subclass !== 'monolith' || objectState.op === 'deleted') {
    return null;
  }

  const settlement = protectedSettlementForObject(objectState, settlements);
  if (!settlement || settlement.monolith_id !== Number(objectState.id)) {
    return null;
  }

  const origin = Util.hex_to_pixel(Number(objectState.x), Number(objectState.y));
  return {
    settlement,
    center: { x: origin.x + 36, y: origin.y + 36 },
    segments: sanctuaryWardSegments(
      Number(objectState.x),
      Number(objectState.y),
      settlement.sanctuary_radius,
    ),
  };
}
