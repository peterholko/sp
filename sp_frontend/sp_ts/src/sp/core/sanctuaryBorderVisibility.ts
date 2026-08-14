export interface SanctuaryBorderVisibilityState {
  sanctuaryBorderVisible: boolean;
}

export const SANCTUARY_BORDER_VISIBLE_BY_DEFAULT = false;

interface SanctuaryBorderSelection {
  type?: string;
  id?: string | number;
}

interface SanctuaryBorderObject {
  subclass?: string;
}

function setSanctuaryBorderVisibility(
  state: SanctuaryBorderVisibilityState,
  next: boolean,
  onChange: (visible: boolean) => void,
): boolean {
  if (state.sanctuaryBorderVisible === next) {
    return next;
  }

  // EventEmitter listeners run synchronously. Commit the shared value first so
  // the Phaser overlay sees the new preference during this notification.
  state.sanctuaryBorderVisible = next;
  onChange(next);
  return next;
}

/** Local-only preference controlled by the shield and Monolith selection. */
export function toggleSanctuaryBorderVisibility(
  state: SanctuaryBorderVisibilityState,
  onChange: (visible: boolean) => void,
): boolean {
  return setSanctuaryBorderVisibility(
    state,
    !state.sanctuaryBorderVisible,
    onChange,
  );
}

/** Selecting the player's live Monolith reveals its initially hidden boundary. */
export function showSanctuaryBorderForMonolithSelection(
  state: SanctuaryBorderVisibilityState,
  selection: SanctuaryBorderSelection | null | undefined,
  objectStates: Record<string, SanctuaryBorderObject | undefined>,
  sanctuaryZones: Record<string, unknown>,
  onChange: (visible: boolean) => void,
): boolean {
  const objectId = selection?.id?.toString();
  const selectedObject = objectId ? objectStates[objectId] : undefined;

  if (
    !objectId
    || selection?.type !== 'obj'
    || selectedObject?.subclass !== 'monolith'
    || !sanctuaryZones[objectId]
  ) {
    return state.sanctuaryBorderVisible;
  }

  return setSanctuaryBorderVisibility(state, true, onChange);
}
