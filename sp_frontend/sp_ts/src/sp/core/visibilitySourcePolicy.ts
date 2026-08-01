import type { ObjectState } from './objectState';

type VisibilitySourceState = Pick<ObjectState, 'player' | 'vision' | 'perceptionObserver'>;

/**
 * The server's observer list is authoritative. Owned vision sources retain
 * their legacy behavior, while a foreign object contributes vision only when
 * the current perception snapshot explicitly designates it as an observer.
 */
export function isVisibilitySource(
  objectState: VisibilitySourceState | null | undefined,
  playerId: string | number,
): boolean {
  return !!objectState
    && (objectState.vision ?? 0) > 0
    && (
      objectState.player == playerId
      || objectState.perceptionObserver === true
    );
}

/**
 * Incremental visible-object packets do not carry observer vision. Preserve
 * the range from the latest authoritative observer snapshot so walking
 * between the center and edge of an already-active Campfire bubble cannot
 * temporarily turn that light source off on the client.
 */
export function mergedIncrementalVision(
  current: Pick<ObjectState, 'vision' | 'perceptionObserver'> | null | undefined,
  incomingVision: number | null | undefined,
): number | null {
  if (
    current?.perceptionObserver === true
    && (current.vision ?? 0) > 0
    && incomingVision == null
  ) {
    return current.vision;
  }

  // `null` means this is an ordinary visible object, not an observer. Keep it
  // distinct from an explicit zero-range observer: the shroud renderer uses
  // numeric zero to render the owning unit's single visible tile at night.
  return incomingVision ?? null;
}

/**
 * A zero-range owned unit gets a soft one-tile visibility edge only when no
 * positive-range observer already illuminates its tile. This prevents the
 * hero from painting shroud back over an active Campfire bubble.
 */
export function needsZeroVisionShroud(
  objectState: Pick<ObjectState, 'vision'> | null | undefined,
  coveredByPositiveVisibilitySource: boolean,
): boolean {
  return objectState?.vision === 0 && !coveredByPositiveVisibilitySource;
}

/** Clear observer authority left by an older connection before init data lands. */
export function resetVisibilitySourceForInit(
  objectState: Pick<ObjectState, 'vision' | 'perceptionObserver'>,
): void {
  if (objectState.perceptionObserver === true) {
    objectState.perceptionObserver = false;
    objectState.vision = null;
  }
}
