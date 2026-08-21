import {
  anchorActionProgress,
  requiresAuthoritativeActionProgress,
} from './actionProgress';

describe('server-authoritative action progress', () => {
  test('anchors a new 30-second gather cycle at its server elapsed time', () => {
    expect(anchorActionProgress({
      action_id: 41,
      action_duration_ms: 30000,
      action_elapsed_ms: 12000,
    }, 50000)).toEqual({
      actionId: 41,
      durationMs: 30000,
      elapsedMs: 12000,
      startTimeMs: 38000,
    });
  });

  test('keeps faster tool durations and consecutive cycle identities distinct', () => {
    expect(anchorActionProgress({
      action_id: 42,
      action_duration_ms: 12000,
      action_elapsed_ms: 0,
    }, 50000)).toEqual({
      actionId: 42,
      durationMs: 12000,
      elapsedMs: 0,
      startTimeMs: 50000,
    });
  });

  test('rejects incomplete timing and clamps stale snapshots', () => {
    expect(anchorActionProgress({ action_id: 1 }, 1000)).toBeNull();
    expect(anchorActionProgress({
      action_id: 2,
      action_duration_ms: 30000,
      action_elapsed_ms: 45000,
    }, 50000)?.elapsedMs).toBe(30000);
  });

  test('prospecting cannot fall back to a client-owned duration', () => {
    expect(requiresAuthoritativeActionProgress('prospecting')).toBe(true);
    expect(requiresAuthoritativeActionProgress('gathering')).toBe(false);
  });
});
