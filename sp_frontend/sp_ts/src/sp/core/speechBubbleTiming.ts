export const SHORT_SPEECH_LIFETIME_MS = 6000;
export const LONG_SPEECH_LIFETIME_MS = 10000;
export const SPEECH_LENGTH_THRESHOLD = 60;
export const SPEECH_FADE_MS = 2000;
export const SPEAKER_MESSAGE_INTERVAL_MS = 10000;

export function speechLifetimeMs(text: string): number {
  return text.length < SPEECH_LENGTH_THRESHOLD
    ? SHORT_SPEECH_LIFETIME_MS
    : LONG_SPEECH_LIFETIME_MS;
}

export function speechFadeStartOffset(totalMs: number): number {
  if (totalMs <= 0) {
    return 0;
  }
  return Math.max(0, (totalMs - SPEECH_FADE_MS) / totalMs);
}

/**
 * Maintains one ten-second speech slot per speaker. Messages from different
 * speakers remain independent, while repeated lines from one villager retain
 * their arrival order.
 */
export class PerSpeakerSpeechQueue {
  private activeSpeakers = new Set<string>();
  private pendingBySpeaker = new Map<string, string[]>();

  enqueue(sourceId: string, text: string): string | null {
    if (!this.activeSpeakers.has(sourceId)) {
      this.activeSpeakers.add(sourceId);
      return text;
    }

    const pending = this.pendingBySpeaker.get(sourceId) ?? [];
    pending.push(text);
    this.pendingBySpeaker.set(sourceId, pending);
    return null;
  }

  advance(sourceId: string): string | null {
    const pending = this.pendingBySpeaker.get(sourceId);
    const next = pending?.shift() ?? null;

    if (pending && pending.length === 0) {
      this.pendingBySpeaker.delete(sourceId);
    }
    if (next === null) {
      this.activeSpeakers.delete(sourceId);
    }

    return next;
  }

  clearSource(sourceId: string): void {
    this.activeSpeakers.delete(sourceId);
    this.pendingBySpeaker.delete(sourceId);
  }

  clear(): void {
    this.activeSpeakers.clear();
    this.pendingBySpeaker.clear();
  }
}
