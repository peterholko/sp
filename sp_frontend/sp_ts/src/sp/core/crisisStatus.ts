export type KnownCrisisPhase =
  | 'dormant'
  | 'signs'
  | 'pressure'
  | 'preparing'
  | 'assault_ready'
  | 'assault_active'
  | 'resolved';

export type CrisisSeverity =
  | 'quiet'
  | 'low'
  | 'medium'
  | 'high'
  | 'crisis'
  | 'resolved';

/**
 * Authoritative personal-crisis snapshot sent by the server. Optional values
 * are omitted by serde when they do not apply. The index signature keeps this
 * client forward-compatible with additional server fields.
 */
export interface CrisisStatusPacket {
  packet: 'crisis_status';
  version: number;
  exists: boolean;
  kind?: string;
  phase?: string;
  pressure?: number;
  pressure_max?: number;
  title?: string;
  summary?: string;
  action_hint?: string;
  severity?: string;
  warning: boolean;
  assault_ready: boolean;
  assault_active: boolean;
  resolved: boolean;
  remaining_attackers?: number;
  total_attackers?: number;
  preparation_seconds_remaining?: number;
  preferred_launch_window?: string;
  continues_while_disconnected: boolean;
  [futureField: string]: unknown;
}

export type CrisisTone =
  | 'neutral'
  | 'low'
  | 'warning'
  | 'high'
  | 'imminent'
  | 'urgent'
  | 'resolved';

interface PhasePresentation {
  label: string;
  tone: CrisisTone;
  urgent: boolean;
  compactLabel?: string;
}

const PHASE_PRESENTATION: Record<KnownCrisisPhase, PhasePresentation> = {
  dormant: { label: 'Dormant', tone: 'neutral', urgent: false },
  signs: { label: 'Signs', tone: 'low', urgent: false },
  pressure: { label: 'Pressure', tone: 'warning', urgent: false },
  preparing: {
    label: 'Preparing',
    tone: 'high',
    urgent: true,
    compactLabel: 'Raiders Gathering',
  },
  assault_ready: {
    label: 'Raid Imminent',
    tone: 'imminent',
    urgent: true,
    compactLabel: 'Raid Imminent',
  },
  assault_active: {
    label: 'Under Attack',
    tone: 'urgent',
    urgent: true,
    compactLabel: 'Under Attack',
  },
  resolved: { label: 'Resolved', tone: 'resolved', urgent: false },
};

const UNKNOWN_PHASE: PhasePresentation = {
  label: 'Unknown Status',
  tone: 'neutral',
  urgent: false,
};

export interface CrisisPressureView {
  value: number;
  max: number;
  percent: number;
}

export interface CrisisStatusView {
  status: CrisisStatusPacket;
  phase: string;
  phaseLabel: string;
  tone: CrisisTone;
  urgent: boolean;
  compactLabel?: string;
  title: string;
  summary: string;
  actionHint: string;
  warning: boolean;
  assaultReady: boolean;
  assaultActive: boolean;
  resolved: boolean;
  pressure: CrisisPressureView | null;
  preparationLabel: string | null;
  attackersLabel: string | null;
  disconnectedWarning: string | null;
}

export interface CrisisUiState {
  crisisStatus: CrisisStatusPacket | null;
  previousCrisisPhase: string | null;
  compactExpanded: boolean;
}

function finiteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function knownPhasePresentation(phase?: string): PhasePresentation {
  if (!phase || !Object.prototype.hasOwnProperty.call(PHASE_PRESENTATION, phase)) {
    return UNKNOWN_PHASE;
  }

  return PHASE_PRESENTATION[phase as KnownCrisisPhase];
}

export function normalizeCrisisStatus(
  packet?: CrisisStatusPacket | null,
): CrisisStatusPacket | null {
  return packet && packet.exists === true ? packet : null;
}

export function crisisPressureView(
  pressure?: number,
  pressureMax?: number,
): CrisisPressureView | null {
  if (!finiteNumber(pressure) || !finiteNumber(pressureMax) || pressureMax <= 0) {
    return null;
  }

  const max = pressureMax;
  const value = Math.max(0, Math.min(max, pressure));

  return {
    value,
    max,
    percent: Math.max(0, Math.min(100, Math.round((value / max) * 100))),
  };
}

export function formatCrisisCountdown(seconds?: number): string | null {
  if (!finiteNumber(seconds) || seconds < 0) {
    return null;
  }

  const wholeSeconds = Math.floor(seconds);
  if (wholeSeconds === 0) {
    return 'complete';
  }

  const minutes = Math.floor(wholeSeconds / 60);
  const remainingSeconds = wholeSeconds % 60;

  if (minutes === 0) {
    return `${remainingSeconds}s`;
  }

  return `${minutes}m ${String(remainingSeconds).padStart(2, '0')}s`;
}

export function crisisStatusView(
  packet?: CrisisStatusPacket | null,
): CrisisStatusView | null {
  const status = normalizeCrisisStatus(packet);
  if (!status) {
    return null;
  }

  const phase = typeof status.phase === 'string' ? status.phase : '';
  const presentation = knownPhasePresentation(phase);
  const assaultReady = status.assault_ready === true || phase === 'assault_ready';
  const assaultActive = status.assault_active === true || phase === 'assault_active';
  const resolved = status.resolved === true || phase === 'resolved';
  const preparationLabel = formatCrisisCountdown(status.preparation_seconds_remaining);
  let attackersLabel: string | null = null;

  if (assaultActive && finiteNumber(status.remaining_attackers)) {
    const remaining = Math.max(0, Math.floor(status.remaining_attackers));
    if (finiteNumber(status.total_attackers)) {
      attackersLabel = `${remaining} / ${Math.max(0, Math.floor(status.total_attackers))}`;
    } else {
      attackersLabel = String(remaining);
    }
  }

  return {
    status,
    phase,
    phaseLabel: presentation.label,
    tone: presentation.tone,
    urgent: presentation.urgent,
    compactLabel: presentation.compactLabel,
    title: typeof status.title === 'string' && status.title.trim() !== ''
      ? status.title
      : 'Personal Crisis',
    summary: typeof status.summary === 'string' ? status.summary : '',
    actionHint: typeof status.action_hint === 'string' ? status.action_hint : '',
    warning: status.warning === true,
    assaultReady,
    assaultActive,
    resolved,
    pressure: crisisPressureView(status.pressure, status.pressure_max),
    preparationLabel,
    attackersLabel,
    disconnectedWarning: assaultActive
      ? 'The assault continues while disconnected.'
      : null,
  };
}

export function receiveCrisisStatus(
  state: CrisisUiState,
  packet?: CrisisStatusPacket | null,
): CrisisUiState {
  const crisisStatus = normalizeCrisisStatus(packet);
  const nextPhase = crisisStatus && typeof crisisStatus.phase === 'string'
    ? crisisStatus.phase
    : null;
  const presentation = knownPhasePresentation(nextPhase || undefined);
  const enteringUrgentPhase = Boolean(
    crisisStatus
      && presentation.urgent
      && nextPhase !== state.previousCrisisPhase,
  );

  return {
    crisisStatus,
    previousCrisisPhase: nextPhase,
    compactExpanded: enteringUrgentPhase ? true : state.compactExpanded,
  };
}

export function clearCrisisStatus(state: CrisisUiState): CrisisUiState {
  return {
    crisisStatus: null,
    previousCrisisPhase: null,
    compactExpanded: state.compactExpanded,
  };
}

export function shouldRenderSurvivalThread(
  hasActiveObjective: boolean,
  packet?: CrisisStatusPacket | null,
): boolean {
  return hasActiveObjective || normalizeCrisisStatus(packet) !== null;
}
