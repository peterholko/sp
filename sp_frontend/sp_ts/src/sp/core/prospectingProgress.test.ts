import { anchorActionProgress } from './actionProgress';
import { prospectingProgressForTile } from './prospectingProgress';

describe('prospecting progress presentation', () => {
  const hero = {
    id: '7',
    state: 'prospecting',
    x: 12,
    y: 14,
    action_id: 91,
    action_duration_ms: 5000,
    action_elapsed_ms: 2300,
  } as any;

  test('uses the hero action identity, duration, and reconnect elapsed time', () => {
    const snapshot = prospectingProgressForTile(hero, 12, 14);
    expect(snapshot).toEqual({
      action_id: 91,
      action_duration_ms: 5000,
      action_elapsed_ms: 2300,
    });
    expect(anchorActionProgress(snapshot!, 10000)).toEqual({
      actionId: 91,
      durationMs: 5000,
      elapsedMs: 2300,
      startTimeMs: 7700,
    });
  });

  test('hides on another tile, after interruption, or before timing arrives', () => {
    expect(prospectingProgressForTile(hero, 13, 14)).toBeNull();
    expect(prospectingProgressForTile({ ...hero, state: 'moving' }, 12, 14)).toBeNull();
    expect(prospectingProgressForTile({ ...hero, action_elapsed_ms: undefined }, 12, 14))
      .toBeNull();
  });
});
