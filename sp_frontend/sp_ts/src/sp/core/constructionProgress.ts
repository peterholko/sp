export interface ConstructionProgressSource {
  id?: string | number;
  structure_id?: string | number;
  work_done?: number;
  total_work?: number;
  work_per_sec?: number;
  work_done_milliunits?: number;
  total_work_milliunits?: number;
  work_per_sec_milliunits?: number;
  construction_action_id?: number;
  construction_updated_at_ms?: number;
}

interface ConstructionProgressTimeline {
  structureId: string;
  actionId: number;
  serverUpdatedAtMs: number;
  authoritativeWorkDone: number;
  visualWorkDone: number;
  totalWork: number;
  workPerSec: number;
  receivedAtMs: number;
}

export interface ConstructionProgressSample {
  structureId: string;
  actionId: number;
  serverUpdatedAtMs: number;
  workDone: number;
  totalWork: number;
  workPerSec: number;
  fraction: number;
}

type ConstructionProgressListener = (sample: ConstructionProgressSample | null) => void;

function clientNowMs(): number {
  return typeof performance != 'undefined' ? performance.now() : Date.now();
}

function exactWorkValue(source: ConstructionProgressSource, milliunitKey: keyof ConstructionProgressSource, valueKey: keyof ConstructionProgressSource): number | null {
  const milliunits = Number(source[milliunitKey]);
  if (Number.isFinite(milliunits)) {
    return milliunits / 1000;
  }

  const value = Number(source[valueKey]);
  return Number.isFinite(value) ? value : null;
}

/**
 * One client-side timeline for every structure construction action. Network
 * snapshots re-anchor this store; renderers only sample it and never maintain
 * independent progress counters.
 */
export class ConstructionProgressTimelineStore {
  private timelines = new Map<string, ConstructionProgressTimeline>();
  private listeners = new Map<string, Set<ConstructionProgressListener>>();
  private animationFrame: number | null = null;

  updateFromSource(
    source: ConstructionProgressSource,
    receivedAtMs: number = clientNowMs(),
  ): ConstructionProgressSample | null {
    const rawStructureId = source.structure_id ?? source.id;
    const actionId = Number(source.construction_action_id);
    const serverUpdatedAtMs = Number(source.construction_updated_at_ms);
    const workDone = exactWorkValue(source, 'work_done_milliunits', 'work_done');
    const totalWork = exactWorkValue(source, 'total_work_milliunits', 'total_work');
    const workPerSec = exactWorkValue(source, 'work_per_sec_milliunits', 'work_per_sec');

    if (rawStructureId == null ||
      !Number.isFinite(actionId) || actionId <= 0 ||
      !Number.isFinite(serverUpdatedAtMs) ||
      workDone == null || totalWork == null || totalWork <= 0 || workPerSec == null) {
      return null;
    }

    const structureId = rawStructureId.toString();
    const existing = this.timelines.get(structureId);
    const boundedWorkDone = Math.max(0, Math.min(totalWork, workDone));
    const boundedWorkPerSec = Math.max(0, workPerSec);

    // Global action IDs and authoritative server timestamps let us reject a
    // delayed perception snapshot instead of making both bars jump backward.
    if (existing && (actionId < existing.actionId ||
      (actionId == existing.actionId &&
        (serverUpdatedAtMs < existing.serverUpdatedAtMs ||
          (serverUpdatedAtMs == existing.serverUpdatedAtMs &&
            boundedWorkDone < existing.authoritativeWorkDone))))) {
      return this.sample(structureId, receivedAtMs);
    }

    // A perception refresh can repeat the exact authoritative progress with a
    // newer serialization timestamp. It is not new progress and must not move
    // the local interpolation origin forward.
    if (existing &&
      actionId == existing.actionId &&
      boundedWorkDone == existing.authoritativeWorkDone &&
      totalWork == existing.totalWork &&
      boundedWorkPerSec == existing.workPerSec) {
      return this.sample(structureId, receivedAtMs);
    }

    const currentVisualWorkDone = existing && actionId == existing.actionId
      ? this.sample(structureId, receivedAtMs)?.workDone ?? boundedWorkDone
      : boundedWorkDone;
    const visualWorkDone = Math.min(
      totalWork,
      Math.max(boundedWorkDone, currentVisualWorkDone),
    );

    this.timelines.set(structureId, {
      structureId,
      actionId,
      serverUpdatedAtMs,
      authoritativeWorkDone: boundedWorkDone,
      visualWorkDone,
      totalWork,
      workPerSec: boundedWorkPerSec,
      receivedAtMs,
    });

    const sample = this.sample(structureId, receivedAtMs);
    this.notify(structureId, sample);
    this.ensureAnimationFrame();
    return sample;
  }

  sample(
    structureId: string | number,
    nowMs: number = clientNowMs(),
  ): ConstructionProgressSample | null {
    const timeline = this.timelines.get(structureId.toString());
    if (!timeline) {
      return null;
    }

    const elapsedSeconds = Math.max(0, nowMs - timeline.receivedAtMs) / 1000;
    const workDone = Math.max(
      0,
      Math.min(
        timeline.totalWork,
        timeline.visualWorkDone + elapsedSeconds * timeline.workPerSec,
      ),
    );

    return {
      structureId: timeline.structureId,
      actionId: timeline.actionId,
      serverUpdatedAtMs: timeline.serverUpdatedAtMs,
      workDone,
      totalWork: timeline.totalWork,
      workPerSec: timeline.workPerSec,
      fraction: workDone / timeline.totalWork,
    };
  }

  clear(structureId: string | number): void {
    const key = structureId.toString();
    if (this.timelines.delete(key)) {
      this.notify(key, null);
    }
  }

  subscribe(
    structureId: string | number,
    listener: ConstructionProgressListener,
  ): () => void {
    const key = structureId.toString();
    const listeners = this.listeners.get(key) ?? new Set<ConstructionProgressListener>();
    listeners.add(listener);
    this.listeners.set(key, listeners);
    listener(this.sample(key));
    this.ensureAnimationFrame();

    return () => {
      const current = this.listeners.get(key);
      current?.delete(listener);
      if (current?.size == 0) {
        this.listeners.delete(key);
      }
      if (this.listeners.size == 0 && this.animationFrame != null &&
        typeof cancelAnimationFrame == 'function') {
        cancelAnimationFrame(this.animationFrame);
        this.animationFrame = null;
      }
    };
  }

  private notify(structureId: string, sample: ConstructionProgressSample | null): void {
    this.listeners.get(structureId)?.forEach(listener => listener(sample));
  }

  private hasAnimatingSubscriber(): boolean {
    for (const structureId of this.listeners.keys()) {
      const sample = this.sample(structureId);
      if (sample && sample.workPerSec > 0 && sample.workDone < sample.totalWork) {
        return true;
      }
    }
    return false;
  }

  private ensureAnimationFrame(): void {
    if (this.animationFrame != null ||
      typeof requestAnimationFrame != 'function' ||
      !this.hasAnimatingSubscriber()) {
      return;
    }

    this.animationFrame = requestAnimationFrame(this.animate);
  }

  private animate = (nowMs: number): void => {
    this.animationFrame = null;
    for (const structureId of this.listeners.keys()) {
      this.notify(structureId, this.sample(structureId, nowMs));
    }
    this.ensureAnimationFrame();
  };
}

export const constructionProgressTimeline = new ConstructionProgressTimelineStore();
