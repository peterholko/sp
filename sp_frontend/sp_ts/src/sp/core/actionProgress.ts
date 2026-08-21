export interface ActionProgressSnapshot {
  action_id?: number;
  action_duration_ms?: number;
  action_elapsed_ms?: number;
}

export interface AnchoredActionProgress {
  actionId: number;
  durationMs: number;
  elapsedMs: number;
  startTimeMs: number;
}

/**
 * These actions must never invent a client-side duration. Their server timing
 * can vary independently of the presentation and must survive reconnects.
 */
export function requiresAuthoritativeActionProgress(state: string): boolean {
  return state === 'prospecting';
}

/**
 * Anchor a server action snapshot to the receiving Phaser clock. The server
 * supplies elapsed time so perception refreshes and reconnects resume partway
 * through an action instead of restarting its bar.
 */
export function anchorActionProgress(
  snapshot: ActionProgressSnapshot,
  receivedAtMs: number,
): AnchoredActionProgress | null {
  const actionId = snapshot.action_id;
  const durationMs = snapshot.action_duration_ms;
  const suppliedElapsedMs = snapshot.action_elapsed_ms;

  if (!Number.isFinite(actionId) || !Number.isFinite(durationMs) || durationMs <= 0) {
    return null;
  }

  const elapsedMs = Math.max(
    0,
    Math.min(durationMs, Number.isFinite(suppliedElapsedMs) ? suppliedElapsedMs : 0),
  );

  return {
    actionId,
    durationMs,
    elapsedMs,
    startTimeMs: receivedAtMs - elapsedMs,
  };
}
