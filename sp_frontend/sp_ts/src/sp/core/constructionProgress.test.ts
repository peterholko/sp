import { ConstructionProgressTimelineStore } from './constructionProgress';

describe('shared construction progress timeline', () => {
  test('projects every consumer from the same exact structure snapshot', () => {
    const store = new ConstructionProgressTimelineStore();
    store.updateFromSource({
      id: 17,
      work_done: 12,
      total_work: 50,
      work_per_sec: 2,
      work_done_milliunits: 12_345,
      total_work_milliunits: 50_000,
      work_per_sec_milliunits: 2_500,
      construction_action_id: 41,
      construction_updated_at_ms: 10_000,
    }, 20_000);

    const mapBar = store.sample(17, 21_000);
    const panelBar = store.sample('17', 21_000);

    expect(mapBar).toEqual(panelBar);
    expect(mapBar?.workDone).toBeCloseTo(14.845);
    expect(mapBar?.fraction).toBeCloseTo(14.845 / 50);
  });

  test('ignores stale snapshots and resets only for a new action id', () => {
    const store = new ConstructionProgressTimelineStore();
    store.updateFromSource({
      structure_id: 9,
      work_done: 20,
      total_work: 100,
      work_per_sec: 1,
      construction_action_id: 50,
      construction_updated_at_ms: 5_000,
    }, 10_000);

    store.updateFromSource({
      structure_id: 9,
      work_done: 5,
      total_work: 100,
      work_per_sec: 1,
      construction_action_id: 50,
      construction_updated_at_ms: 4_000,
    }, 11_000);
    expect(store.sample(9, 11_000)?.workDone).toBeCloseTo(21);

    store.updateFromSource({
      structure_id: 9,
      work_done: 19,
      total_work: 100,
      work_per_sec: 1,
      construction_action_id: 50,
      construction_updated_at_ms: 5_000,
    }, 11_000);
    expect(store.sample(9, 11_000)?.workDone).toBeCloseTo(21);

    store.updateFromSource({
      structure_id: 9,
      work_done: 3,
      total_work: 80,
      work_per_sec: 0,
      construction_action_id: 51,
      construction_updated_at_ms: 6_000,
    }, 12_000);
    expect(store.sample(9, 20_000)?.workDone).toBe(3);
    expect(store.sample(9, 20_000)?.actionId).toBe(51);
  });

  test('ignores a newer timestamp when progress itself is unchanged', () => {
    const store = new ConstructionProgressTimelineStore();
    store.updateFromSource({
      structure_id: 12,
      work_done: 20,
      total_work: 100,
      work_per_sec: 2,
      construction_action_id: 60,
      construction_updated_at_ms: 5_000,
    }, 10_000);

    expect(store.sample(12, 11_000)?.workDone).toBeCloseTo(22);

    store.updateFromSource({
      structure_id: 12,
      work_done: 20,
      total_work: 100,
      work_per_sec: 2,
      construction_action_id: 60,
      construction_updated_at_ms: 6_000,
    }, 11_000);

    expect(store.sample(12, 11_000)?.workDone).toBeCloseTo(22);
    expect(store.sample(12, 12_000)?.workDone).toBeCloseTo(24);
    expect(store.sample(12, 12_000)?.serverUpdatedAtMs).toBe(5_000);
  });

  test('does not snap visual progress backward within one action', () => {
    const store = new ConstructionProgressTimelineStore();
    store.updateFromSource({
      structure_id: 14,
      work_done: 20,
      total_work: 100,
      work_per_sec: 2,
      construction_action_id: 61,
      construction_updated_at_ms: 5_000,
    }, 10_000);

    expect(store.sample(14, 11_000)?.workDone).toBeCloseTo(22);

    store.updateFromSource({
      structure_id: 14,
      work_done: 21,
      total_work: 100,
      work_per_sec: 2,
      construction_action_id: 61,
      construction_updated_at_ms: 6_000,
    }, 11_000);

    expect(store.sample(14, 11_000)?.workDone).toBeCloseTo(22);
    expect(store.sample(14, 12_000)?.workDone).toBeCloseTo(24);
  });
});
