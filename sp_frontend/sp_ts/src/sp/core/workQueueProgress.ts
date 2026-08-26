import {
  ActionProgressSnapshot,
  AnchoredActionProgress,
  anchorActionProgress,
} from './actionProgress';

/**
 * Reconcile a newly received queue-action snapshot with the bar already being
 * displayed. A repeated snapshot for the same action may move the anchor
 * forward, but never backward. Only a distinct action id starts a new cycle.
 */
export function reconcileWorkQueueProgress(
  previous: AnchoredActionProgress | null,
  snapshot: ActionProgressSnapshot,
  receivedAtMs: number,
): AnchoredActionProgress | null {
  const incoming = anchorActionProgress(snapshot, receivedAtMs);
  if (!incoming) {
    return null;
  }

  if (!previous || previous.actionId !== incoming.actionId) {
    return incoming;
  }

  const displayedElapsedMs = Math.max(
    0,
    Math.min(previous.durationMs, receivedAtMs - previous.startTimeMs),
  );
  const monotonicElapsedMs = Math.min(
    incoming.durationMs,
    Math.max(displayedElapsedMs, incoming.elapsedMs),
  );

  return {
    ...incoming,
    elapsedMs: monotonicElapsedMs,
    startTimeMs: receivedAtMs - monotonicElapsedMs,
  };
}

export function workQueueProgressFraction(
  progress: AnchoredActionProgress | null,
  nowMs: number,
): number {
  if (!progress || progress.durationMs <= 0) {
    return 0;
  }

  const elapsedMs = Math.max(0, nowMs - progress.startTimeMs);
  return Math.max(0, Math.min(1, elapsedMs / progress.durationMs));
}
