export const TUTORIAL_FIRST_HINT_DELAY_MS = 60_000;
export const TUTORIAL_REPEAT_HINT_DELAY_MS = 120_000;
export const TUTORIAL_HINT_VISIBLE_MS = 12_000;
export const TUTORIAL_OBJECTIVE_STALE_MS = 15_000;

export interface TutorialHintObjective {
  id: string;
  title: string;
  actionHint: string;
  blocker: string | null;
  state: string;
  progress: number | null;
  goal: number | null;
}

export interface TutorialHintPolicyState {
  enabled: boolean;
  objective: TutorialHintObjective | null;
  lastPacketAt: number | null;
  progressAt: number | null;
  lastHintAt: number | null;
  visibleUntil: number | null;
  eligible: boolean;
}

function finiteNumber(value: unknown): number | null {
  return typeof value === 'number' && Number.isFinite(value) ? value : null;
}

function text(value: unknown): string {
  return typeof value === 'string' ? value.trim() : '';
}

export function objectiveForTutorialHint(packet: any): TutorialHintObjective | null {
  if (!packet || !Array.isArray(packet.objectives)) {
    return null;
  }

  const currentId = text(packet.current_id);
  if (!currentId || currentId === 'complete') {
    return null;
  }

  const objective = packet.objectives.find((candidate) => candidate && candidate.id === currentId)
    || packet.objectives.find((candidate) => candidate && candidate.state === 'active');
  if (!objective || objective.state === 'complete') {
    return null;
  }

  return {
    id: text(objective.id) || currentId,
    title: text(objective.title) || 'Current survival task',
    actionHint: text(objective.action_hint),
    blocker: text(objective.blocker) || null,
    state: text(objective.state),
    progress: finiteNumber(objective.progress),
    goal: finiteNumber(objective.goal),
  };
}

export function createTutorialHintPolicy(
  enabled = true,
  now = 0,
): TutorialHintPolicyState {
  return {
    enabled,
    objective: null,
    lastPacketAt: null,
    progressAt: now,
    lastHintAt: null,
    visibleUntil: null,
    eligible: false,
  };
}

function objectiveMadeProgress(
  previous: TutorialHintObjective | null,
  next: TutorialHintObjective | null,
): boolean {
  if (previous === null || next === null) {
    return previous !== next;
  }
  if (previous.id !== next.id || previous.state !== next.state) {
    return true;
  }
  if (previous.blocker !== next.blocker) {
    return true;
  }
  return next.progress !== null
    && (previous.progress === null || next.progress > previous.progress);
}

/**
 * Records an authoritative objective packet. Packet receipt keeps the snapshot
 * fresh, while only meaningful progress resets the hint grace period.
 */
export function receiveTutorialObjective(
  state: TutorialHintPolicyState,
  packet: any,
  now: number,
): TutorialHintPolicyState {
  const objective = objectiveForTutorialHint(packet);
  const progressed = objectiveMadeProgress(state.objective, objective);

  return {
    ...state,
    objective,
    lastPacketAt: now,
    progressAt: progressed || state.progressAt === null ? now : state.progressAt,
    lastHintAt: progressed ? null : state.lastHintAt,
    visibleUntil: progressed ? null : state.visibleUntil,
  };
}

export function setTutorialHintEnabled(
  state: TutorialHintPolicyState,
  enabled: boolean,
  now: number,
): TutorialHintPolicyState {
  if (state.enabled === enabled) {
    return state;
  }

  return {
    ...state,
    enabled,
    progressAt: now,
    lastHintAt: null,
    visibleUntil: null,
    eligible: false,
  };
}

export function resetTutorialHintPolicy(
  state: TutorialHintPolicyState,
  now: number,
): TutorialHintPolicyState {
  return {
    ...state,
    objective: null,
    lastPacketAt: null,
    progressAt: now,
    lastHintAt: null,
    visibleUntil: null,
    eligible: false,
  };
}

/** Advances the deterministic hint clock by one caller-supplied instant. */
export function advanceTutorialHintPolicy(
  state: TutorialHintPolicyState,
  now: number,
  paused: boolean,
): TutorialHintPolicyState {
  const packetFresh = state.lastPacketAt !== null
    && now - state.lastPacketAt <= TUTORIAL_OBJECTIVE_STALE_MS;
  const eligible = Boolean(state.enabled && state.objective && packetFresh && !paused);

  if (!eligible) {
    if (!state.eligible && state.visibleUntil === null) {
      return state;
    }
    return {
      ...state,
      eligible: false,
      visibleUntil: null,
    };
  }

  // Resuming from any pause starts a full grace period; paused time never
  // produces an immediate catch-up hint.
  if (!state.eligible) {
    return {
      ...state,
      eligible: true,
      progressAt: now,
      lastHintAt: null,
      visibleUntil: null,
    };
  }

  let visibleUntil = state.visibleUntil;
  if (visibleUntil !== null && now >= visibleUntil) {
    visibleUntil = null;
  }

  const dueAt = state.lastHintAt === null
    ? (state.progressAt === null ? now : state.progressAt) + TUTORIAL_FIRST_HINT_DELAY_MS
    : state.lastHintAt + TUTORIAL_REPEAT_HINT_DELAY_MS;
  if (visibleUntil === null && now >= dueAt) {
    return {
      ...state,
      eligible: true,
      lastHintAt: now,
      visibleUntil: now + TUTORIAL_HINT_VISIBLE_MS,
    };
  }

  if (visibleUntil !== state.visibleUntil) {
    return { ...state, eligible: true, visibleUntil };
  }
  return state;
}

export function dismissTutorialHint(
  state: TutorialHintPolicyState,
  now: number,
): TutorialHintPolicyState {
  return {
    ...state,
    lastHintAt: now,
    visibleUntil: null,
  };
}

export function tutorialHintVisible(state: TutorialHintPolicyState, now: number): boolean {
  return state.enabled
    && state.objective !== null
    && state.visibleUntil !== null
    && now < state.visibleUntil;
}
