export interface CombatFloatingTextSource {
  dmg: number;
  combo?: string;
  missed?: boolean;
}

export interface CombatFloatingTextPresentation {
  targetText: string;
  sourceText?: string;
}

export const COMBO_ANNOUNCEMENT_DEDUP_MS = 1000;

/**
 * Combo names describe the attacker's action, while damage describes what
 * happened to each target. Keep those as separate floating-text elements so
 * area finishers can report every affected target without repeating the name.
 */
export function combatFloatingTextPresentation(
  message: CombatFloatingTextSource,
): CombatFloatingTextPresentation {
  return {
    targetText: message.missed ? 'Miss' : String(message.dmg),
    sourceText: message.combo ? `${message.combo}!` : undefined,
  };
}

/**
 * Area finishers arrive as one damage packet per affected target. Only the
 * first packet should announce the combo above its source.
 */
export function shouldAnnounceCombo(
  announcementTimes: Map<string, number>,
  sourceId: number,
  combo: string,
  now: number,
): boolean {
  const key = `${sourceId}:${combo}`;
  const lastAnnouncement = announcementTimes.get(key);

  if (
    lastAnnouncement !== undefined
    && now >= lastAnnouncement
    && now - lastAnnouncement < COMBO_ANNOUNCEMENT_DEDUP_MS
  ) {
    return false;
  }

  announcementTimes.set(key, now);

  for (const [announcementKey, announcementAt] of announcementTimes) {
    if (now - announcementAt >= COMBO_ANNOUNCEMENT_DEDUP_MS * 5) {
      announcementTimes.delete(announcementKey);
    }
  }

  return true;
}
