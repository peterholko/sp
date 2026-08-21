const REPEATING_ANIMATED_STATES = new Set([
  'crafting',
  'gathering',
  'harvesting',
  'sleeping',
]);

function isButcheringActivity(state: string, activity?: string): boolean {
  return state === 'refining' && activity?.trim().toLowerCase() === 'skinning';
}

function displayedState(state: string, activity?: string): string {
  if (isButcheringActivity(state, activity)) {
    return 'Butchering';
  }

  if (state === 'gathering') {
    const gatheringActivity = activity?.trim().toLowerCase();
    if (gatheringActivity === 'hunting') {
      return 'Hunting';
    }
    if (gatheringActivity === 'logging') {
      return 'Logging';
    }
    if (gatheringActivity === 'foraging') {
      return 'Foraging';
    }
  }

  if (state === 'surveying') {
    return 'Scouting';
  }

  return state;
}

export function objectStateText(state: string, activity?: string): string {
  const displayed = displayedState(state, activity);
  return displayed === 'sleeping' ? 'Zzzzz…' : `* ${displayed} *`;
}

/**
 * States without an animation keep the legacy repeating state label. States
 * with an animation only show a label when it communicates useful ongoing
 * work; sleep always keeps its Zzz indicator visible.
 */
export function repeatingObjectStateText(
  state: string,
  animationExists: boolean,
  activity?: string,
): string | null {
  if (!state || state === 'dead' || state === 'moving') {
    return null;
  }

  if (
    animationExists
    && !REPEATING_ANIMATED_STATES.has(state)
    && !isButcheringActivity(state, activity)
  ) {
    return null;
  }

  return objectStateText(state, activity);
}
