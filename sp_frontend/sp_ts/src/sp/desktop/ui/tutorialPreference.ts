export const TUTORIAL_PREFERENCE_VERSION = 1;

const TUTORIAL_PREFERENCE_KEY_PREFIX =
  `siege-perilous.desktop.tutorial.v${TUTORIAL_PREFERENCE_VERSION}.player`;

export interface TutorialPreferenceStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export function tutorialPreferenceStorageKey(playerId: string | number): string {
  return `${TUTORIAL_PREFERENCE_KEY_PREFIX}.${encodeURIComponent(String(playerId))}`;
}

function browserStorage(): TutorialPreferenceStorage | null {
  try {
    return typeof window === 'undefined' ? null : window.localStorage;
  } catch (_error) {
    return null;
  }
}

export function loadTutorialEnabled(
  playerId: string | number,
  storage?: TutorialPreferenceStorage | null,
): boolean {
  const resolvedStorage = storage === undefined ? browserStorage() : storage;
  if (resolvedStorage === null) {
    return true;
  }

  try {
    const storedValue = resolvedStorage.getItem(tutorialPreferenceStorageKey(playerId));
    if (storedValue === 'false') {
      return false;
    }
    if (storedValue === 'true') {
      return true;
    }
  } catch (_error) {
    // Storage can be unavailable even when the browser exposes localStorage.
  }

  return true;
}

export function saveTutorialEnabled(
  playerId: string | number,
  enabled: boolean,
  storage?: TutorialPreferenceStorage | null,
): boolean {
  const resolvedStorage = storage === undefined ? browserStorage() : storage;
  if (resolvedStorage === null) {
    return false;
  }

  try {
    resolvedStorage.setItem(tutorialPreferenceStorageKey(playerId), String(enabled));
    return true;
  } catch (_error) {
    // Preference changes still apply to the current session when persistence fails.
    return false;
  }
}
