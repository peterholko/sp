import * as React from "react";
import { Global } from "../../core/global";
import { NetworkEvent } from "../../core/networkEvent";
import { isDesktop, isWideScreen } from "../../core/config";
import {
  CrisisStatusPacket,
  CrisisStatusView,
  CrisisTone,
  CrisisUiState,
  clearCrisisStatus,
  crisisStatusView,
  receiveCrisisStatus,
  shouldRenderSurvivalThread,
} from "../../core/crisisStatus";
import {
  SAFE_LOGOUT_CONDITIONS,
  SAFE_LOGOUT_ARIA_LIVE,
  SafeLogoutStatusPacket,
  SafeLogoutStatusView,
  SafeLogoutUiState,
  beginSafeLogoutCancellation,
  beginSafeLogoutRequest,
  clearSafeLogoutStatus,
  receiveSafeLogoutStatus,
  safeLogoutStatusView,
  safeLogoutLayoutMode,
  shouldRenderSafeLogout,
} from "../../core/safeLogoutStatus";

const COMPACT_DESKTOP_MAX_WIDTH = 1280;
const DESKTOP_THREAD_BOTTOM = '145px';

interface ObjectiveProgress {
  id: string;
  title: string;
  state: string;
  category: string;
  target?: string;
  action_hint: string;
  lesson: string;
  reward: string;
  progress?: number;
  goal?: number;
}

interface ThreatRisk {
  id: string;
  label: string;
  severity: string;
  trigger_hint: string;
  counter_hint: string;
  current?: number;
  threshold?: number;
}

interface LegendaryThreat {
  name: string;
  status: string;
  days_active: number;
  hideout_known: boolean;
  hideout_location?: string;
  next_attack_eta?: number;
  followers_defeated: number;
  captains_defeated: number;
}

interface ObjectivesState extends CrisisUiState, SafeLogoutUiState {
  build_campfire: boolean;
  build_3_structures: boolean;
  recruit_villager: boolean;
  explore_poi: boolean;
  survive_5_nights: boolean;
  objectiveState: any;
  threatState: any;
  discoveryEvent: any;
  viewportWidth: number;
}

const severityRank = {
  crisis: 5,
  high: 4,
  medium: 3,
  low: 2,
  quiet: 1,
};

const crisisToneColor: Record<CrisisTone, string> = {
  neutral: '#9aa0a6',
  low: '#a7c59a',
  warning: '#e2bd67',
  high: '#e49a52',
  imminent: '#e66d4e',
  urgent: '#e05252',
  resolved: '#78b978',
};

export default class ObjectivesPanel extends React.Component<{}, ObjectivesState> {
  private observedHeroId: string | null = null;
  private safeLogoutRequestLocked = false;
  private safeLogoutCancelLocked = false;

  constructor(props) {
    super(props);
    this.state = {
      build_campfire: false,
      build_3_structures: false,
      recruit_villager: false,
      explore_poi: false,
      survive_5_nights: false,
      objectiveState: null,
      threatState: null,
      discoveryEvent: null,
      crisisStatus: null,
      previousCrisisPhase: null,
      safeLogoutStatus: null,
      safeLogoutRequestInFlight: false,
      safeLogoutCancelInFlight: false,
      viewportWidth: typeof window === 'undefined' ? 0 : window.innerWidth,
      // QW1: start the Survival Thread expanded so the tutorial guidance is
      // visible by default on compact desktops; the player can still collapse it.
      compactExpanded: true,
    };

    this.handleResize = this.handleResize.bind(this);
    this.toggleCompactExpanded = this.toggleCompactExpanded.bind(this);
    this.handleBeginSafeLogout = this.handleBeginSafeLogout.bind(this);
    this.handleCancelSafeLogout = this.handleCancelSafeLogout.bind(this);
  }

  componentDidMount() {
    Global.gameEmitter.on(NetworkEvent.OBJECTIVES, this.handleObjectives, this);
    Global.gameEmitter.on(NetworkEvent.OBJECTIVE_STATE, this.handleObjectiveState, this);
    Global.gameEmitter.on(NetworkEvent.THREAT_STATE, this.handleThreatState, this);
    Global.gameEmitter.on(NetworkEvent.DISCOVERY_EVENT, this.handleDiscoveryEvent, this);
    Global.gameEmitter.on(NetworkEvent.CRISIS_STATUS, this.handleCrisisStatus, this);
    Global.gameEmitter.on(NetworkEvent.SAFE_LOGOUT_STATUS, this.handleSafeLogoutStatus, this);
    Global.gameEmitter.on(NetworkEvent.SAFE_LOGOUT_RESET, this.handleSafeLogoutReset, this);
    Global.gameEmitter.on(NetworkEvent.INFO_TRUE_DEATH, this.handleRunReset, this);
    Global.gameEmitter.on(NetworkEvent.SELECT_CLASS, this.handleRunReset, this);
    Global.gameEmitter.on(NetworkEvent.FIRST_LOGIN, this.handleRunReset, this);
    Global.gameEmitter.on(NetworkEvent.HERO_INIT, this.handleHeroInit, this);

    if (typeof window !== 'undefined') {
      window.addEventListener('resize', this.handleResize);
    }

    const latestSafeLogoutStatus = Global.network
      && typeof Global.network.getLatestSafeLogoutStatus === 'function'
      ? Global.network.getLatestSafeLogoutStatus()
      : null;
    if (latestSafeLogoutStatus) {
      this.handleSafeLogoutStatus(latestSafeLogoutStatus);
    }
  }

  componentWillUnmount() {
    Global.gameEmitter.off(NetworkEvent.OBJECTIVES, this.handleObjectives, this);
    Global.gameEmitter.off(NetworkEvent.OBJECTIVE_STATE, this.handleObjectiveState, this);
    Global.gameEmitter.off(NetworkEvent.THREAT_STATE, this.handleThreatState, this);
    Global.gameEmitter.off(NetworkEvent.DISCOVERY_EVENT, this.handleDiscoveryEvent, this);
    Global.gameEmitter.off(NetworkEvent.CRISIS_STATUS, this.handleCrisisStatus, this);
    Global.gameEmitter.off(NetworkEvent.SAFE_LOGOUT_STATUS, this.handleSafeLogoutStatus, this);
    Global.gameEmitter.off(NetworkEvent.SAFE_LOGOUT_RESET, this.handleSafeLogoutReset, this);
    Global.gameEmitter.off(NetworkEvent.INFO_TRUE_DEATH, this.handleRunReset, this);
    Global.gameEmitter.off(NetworkEvent.SELECT_CLASS, this.handleRunReset, this);
    Global.gameEmitter.off(NetworkEvent.FIRST_LOGIN, this.handleRunReset, this);
    Global.gameEmitter.off(NetworkEvent.HERO_INIT, this.handleHeroInit, this);

    if (typeof window !== 'undefined') {
      window.removeEventListener('resize', this.handleResize);
    }
  }

  handleResize() {
    if (typeof window === 'undefined') {
      return;
    }

    this.setState({ viewportWidth: window.innerWidth });
  }

  toggleCompactExpanded() {
    this.setState((state) => ({ compactExpanded: !state.compactExpanded }));
  }

  handleObjectives(message) {
    this.setState({
      build_campfire: message.build_campfire,
      build_3_structures: message.build_3_structures,
      recruit_villager: message.recruit_villager,
      explore_poi: message.explore_poi,
      survive_5_nights: message.survive_5_nights,
    });
  }

  handleObjectiveState(message) {
    this.setState({ objectiveState: message });
  }

  handleThreatState(message) {
    this.setState({ threatState: message });
  }

  handleDiscoveryEvent(message) {
    this.setState({ discoveryEvent: message });
  }

  handleCrisisStatus(message: CrisisStatusPacket) {
    this.setState((state) => receiveCrisisStatus(state, message));
  }

  handleSafeLogoutStatus(message: SafeLogoutStatusPacket) {
    const view = safeLogoutStatusView(message);
    const keepRequestLocked = Boolean(
      this.safeLogoutRequestLocked
      && view
      && view.state === 'online'
      && view.canRequest
      && !view.reason,
    );
    const keepCancelLocked = Boolean(
      this.safeLogoutCancelLocked
      && view
      && view.pending,
    );
    this.safeLogoutRequestLocked = keepRequestLocked;
    this.safeLogoutCancelLocked = keepCancelLocked;
    const next = {
      ...receiveSafeLogoutStatus(message),
      safeLogoutRequestInFlight: keepRequestLocked,
      safeLogoutCancelInFlight: keepCancelLocked,
    };
    this.setState((state) => ({
      ...next,
      compactExpanded: Boolean(
        view
        && (view.pending || view.protected || view.reason)
      ) ? true : state.compactExpanded,
    }));
  }

  handleSafeLogoutReset() {
    this.safeLogoutRequestLocked = false;
    this.safeLogoutCancelLocked = false;
    this.setState(clearSafeLogoutStatus());
  }

  handleBeginSafeLogout() {
    if (this.safeLogoutRequestLocked) {
      return;
    }

    const current: SafeLogoutUiState = this.state;
    const next = beginSafeLogoutRequest(current);
    if (next === current) {
      return;
    }

    this.safeLogoutRequestLocked = true;
    if (!Global.network || !Global.network.sendRequestSafeLogout()) {
      this.safeLogoutRequestLocked = false;
      return;
    }
    this.setState(next);
  }

  handleCancelSafeLogout() {
    if (this.safeLogoutCancelLocked) {
      return;
    }

    const current: SafeLogoutUiState = this.state;
    const next = beginSafeLogoutCancellation(current);
    if (next === current) {
      return;
    }

    this.safeLogoutCancelLocked = true;
    if (!Global.network || !Global.network.sendCancelSafeLogout()) {
      this.safeLogoutCancelLocked = false;
      return;
    }
    this.setState(next);
  }

  handleRunReset() {
    if (Global.network && typeof Global.network.clearLatestSafeLogoutStatus === 'function') {
      Global.network.clearLatestSafeLogoutStatus();
    }
    this.observedHeroId = null;
    this.safeLogoutRequestLocked = false;
    this.safeLogoutCancelLocked = false;
    this.setState((state) => ({
      ...clearCrisisStatus(state),
      ...clearSafeLogoutStatus(),
    }));
  }

  handleHeroInit(heroId) {
    const nextHeroId = String(heroId);

    // A reconnect reuses the same hero id and should not replay urgent
    // auto-expansion. A recreated run receives a different hero id and clears
    // any locally retained snapshot while the authoritative packet is resent.
    if (this.observedHeroId !== null && this.observedHeroId !== nextHeroId) {
      if (Global.network && typeof Global.network.clearLatestSafeLogoutStatus === 'function') {
        Global.network.clearLatestSafeLogoutStatus();
      }
      this.safeLogoutRequestLocked = false;
      this.safeLogoutCancelLocked = false;
      this.setState((state) => ({
        ...clearCrisisStatus(state),
        ...clearSafeLogoutStatus(),
      }));
    }

    this.observedHeroId = nextHeroId;
  }

  legacyObjectives(): ObjectiveProgress[] {
    return [
      {
        id: 'build_campfire',
        title: 'Build a campfire',
        state: this.state.build_campfire ? 'complete' : 'active',
        category: 'Settlement',
        action_hint: 'Build a campfire before dusk.',
        lesson: 'Fire makes night danger easier to read.',
        reward: 'Warmth and vision.',
      },
      {
        id: 'explore_poi',
        title: 'Investigate a point of interest',
        state: this.state.explore_poi ? 'complete' : 'locked',
        category: 'Exploration',
        action_hint: 'Look for useful places near camp.',
        lesson: 'Discovery should tell you what new option it enables.',
        reward: 'New supplies and plans.',
      },
      {
        id: 'recruit_villager',
        title: 'Recruit a villager',
        state: this.state.recruit_villager ? 'complete' : 'locked',
        category: 'Villager',
        action_hint: 'Rescue or hire a settler.',
        lesson: 'Villagers turn one-off survival into repeatable work.',
        reward: 'A new worker and new guidance.',
      },
      {
        id: 'build_3_structures',
        title: 'Build three structures',
        state: this.state.build_3_structures ? 'complete' : 'locked',
        category: 'Settlement',
        action_hint: 'Add buildings that solve rest, storage, and defense.',
        lesson: 'Each building should answer a visible problem.',
        reward: 'A camp that can survive pressure.',
      },
      {
        id: 'survive_5_nights',
        title: 'Survive five nights',
        state: this.state.survive_5_nights ? 'complete' : 'locked',
        category: 'Survival',
        action_hint: 'Use daylight to prepare before danger rises.',
        lesson: 'Threats are pressure signals.',
        reward: 'A stable foothold.',
      },
    ];
  }

  activeObjective(objectives: ObjectiveProgress[]): ObjectiveProgress | null {
    return objectives.find(obj => obj.state == 'active') || objectives.find(obj => obj.state != 'complete') || null;
  }

  sortedRisks(): ThreatRisk[] {
    const threatState = this.state.threatState;
    if (!threatState || !threatState.known_risks) {
      return [];
    }

    return [...threatState.known_risks].sort((a, b) => {
      return (severityRank[b.severity] || 0) - (severityRank[a.severity] || 0);
    });
  }

  renderProgress(objective: ObjectiveProgress, labelStyle: React.CSSProperties) {
    if (typeof objective.progress != 'number' || typeof objective.goal != 'number' || objective.goal <= 0) {
      return null;
    }

    const pct = Math.max(0, Math.min(100, Math.round((objective.progress / objective.goal) * 100)));
    const railStyle: React.CSSProperties = {
      height: '5px',
      width: '100%',
      background: 'rgba(255,255,255,0.16)',
      borderRadius: '3px',
      overflow: 'hidden',
      marginTop: '5px',
    };
    const fillStyle: React.CSSProperties = {
      height: '5px',
      width: pct + '%',
      background: '#c9aa71',
    };

    return (
      <div>
        <div style={railStyle}>
          <div style={fillStyle} />
        </div>
        <div style={labelStyle}>{objective.progress}/{objective.goal}</div>
      </div>
    );
  }

  renderCrisisCard(
    crisis: CrisisStatusView,
    bodyStyle: React.CSSProperties,
    labelStyle: React.CSSProperties,
  ) {
    const accent = crisisToneColor[crisis.tone];
    const cardStyle: React.CSSProperties = {
      border: `1px solid ${accent}`,
      borderLeft: `3px solid ${accent}`,
      borderRadius: '3px',
      background: 'rgba(255,255,255,0.035)',
      padding: '7px 8px',
      marginBottom: '8px',
    };
    const headerStyle: React.CSSProperties = {
      display: 'flex',
      justifyContent: 'space-between',
      alignItems: 'flex-start',
      gap: '8px',
      marginBottom: '4px',
    };
    const headingStyle: React.CSSProperties = {
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '12px',
      fontWeight: 'bold',
      lineHeight: 1.2,
    };
    const phaseStyle: React.CSSProperties = {
      color: accent,
      fontFamily: 'Verdana',
      fontSize: '8px',
      fontWeight: 'bold',
      lineHeight: 1.2,
      textTransform: 'uppercase',
      whiteSpace: 'nowrap',
    };
    const pressureRailStyle: React.CSSProperties = {
      height: '6px',
      width: '100%',
      background: 'rgba(255,255,255,0.16)',
      borderRadius: '3px',
      overflow: 'hidden',
      marginTop: '3px',
    };
    const statusRowStyle: React.CSSProperties = {
      ...labelStyle,
      color: '#d4d4d4',
      display: 'flex',
      justifyContent: 'space-between',
      gap: '8px',
      marginTop: '5px',
    };
    const urgentTextStyle: React.CSSProperties = {
      ...bodyStyle,
      color: crisis.assaultActive ? '#ffaaaa' : accent,
      fontWeight: 'bold',
      marginTop: '5px',
      marginBottom: 0,
    };

    return (
      <section
        style={cardStyle}
        role="region"
        aria-label="Personal goblin crisis status"
        aria-labelledby="personal-crisis-title"
      >
        <div
          style={headerStyle}
          aria-live={crisis.urgent ? 'assertive' : 'polite'}
          aria-atomic="true"
        >
          <div id="personal-crisis-title" style={headingStyle}>{crisis.title}</div>
          <div style={phaseStyle}>
            {crisis.phaseLabel}{crisis.warning ? ' · Warning' : ''}
          </div>
        </div>

        {crisis.summary && <div style={bodyStyle}>{crisis.summary}</div>}
        {crisis.actionHint &&
          <div style={labelStyle}><strong>Next:</strong> {crisis.actionHint}</div>}

        {crisis.pressure &&
          <div style={{ marginTop: '6px' }}>
            <div style={statusRowStyle}>
              <span>Pressure</span>
              <span>{crisis.pressure.value} / {crisis.pressure.max}</span>
            </div>
            <div
              style={pressureRailStyle}
              role="progressbar"
              aria-label="Goblin crisis pressure"
              aria-valuemin={0}
              aria-valuemax={crisis.pressure.max}
              aria-valuenow={crisis.pressure.value}
              title={`Goblin crisis pressure: ${crisis.pressure.value} of ${crisis.pressure.max}`}
            >
              <div style={{
                height: '100%',
                width: `${crisis.pressure.percent}%`,
                background: accent,
              }} />
            </div>
          </div>}

        {crisis.preparationLabel &&
          <div style={statusRowStyle}>
            <span>Minimum warning</span>
            <span>{crisis.preparationLabel}</span>
          </div>}

        {crisis.assaultActive &&
          <div style={statusRowStyle}>
            <span>Attackers remaining</span>
            <span>{crisis.attackersLabel || 'Updating'}</span>
          </div>}

        {crisis.disconnectedWarning &&
          <div style={urgentTextStyle}>{crisis.disconnectedWarning}</div>}

        {crisis.resolved &&
          <div style={urgentTextStyle}>Crisis resolved. Recover, repair, and rebuild.</div>}
      </section>
    );
  }

  renderSafeLogoutCard(
    safeLogout: SafeLogoutStatusView,
    bodyStyle: React.CSSProperties,
    labelStyle: React.CSSProperties,
  ) {
    const pending = safeLogout.pending;
    const protectedStatus = safeLogout.protected;
    const accent = protectedStatus ? '#78b978' : pending ? '#e2bd67' : '#8fb7d9';
    const cardStyle: React.CSSProperties = {
      border: `1px solid ${accent}`,
      borderLeft: `3px solid ${accent}`,
      borderRadius: '3px',
      background: 'rgba(255,255,255,0.035)',
      padding: '8px',
      marginBottom: '8px',
      boxSizing: 'border-box',
    };
    const headingStyle: React.CSSProperties = {
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '12px',
      fontWeight: 'bold',
      lineHeight: 1.2,
      marginBottom: '4px',
    };
    const countdownStyle: React.CSSProperties = {
      color: '#f2d27a',
      fontFamily: 'Verdana',
      fontSize: '18px',
      fontWeight: 'bold',
      lineHeight: 1.25,
      margin: '6px 0',
      textAlign: 'center',
    };
    const contractStyle: React.CSSProperties = {
      ...labelStyle,
      color: '#c9aa71',
      marginTop: '6px',
    };
    const buttonStyle: React.CSSProperties = {
      width: '100%',
      minHeight: '32px',
      marginTop: '7px',
      padding: '6px 9px',
      border: `1px solid ${accent}`,
      borderRadius: '3px',
      background: 'rgba(20, 24, 28, 0.92)',
      color: '#f2e7cf',
      cursor: 'pointer',
      fontFamily: 'Verdana',
      fontSize: '10px',
      fontWeight: 'bold',
      whiteSpace: 'normal',
    };
    const disabledButtonStyle: React.CSSProperties = {
      ...buttonStyle,
      borderColor: '#5c6268',
      color: '#92979c',
      cursor: 'not-allowed',
    };
    const requestDisabled = !safeLogout.canRequest || this.state.safeLogoutRequestInFlight;
    const cancelDisabled = !safeLogout.canCancel || this.state.safeLogoutCancelInFlight;

    return (
      <section
        style={cardStyle}
        role="region"
        aria-label="Safe Logout status"
        aria-labelledby="safe-logout-title"
      >
        <div id="safe-logout-title" style={headingStyle}>
          {protectedStatus ? 'Settlement Protected' : 'Safe Logout'}
        </div>

        <div
          id="safe-logout-message"
          style={bodyStyle}
          role="status"
          aria-live={SAFE_LOGOUT_ARIA_LIVE}
          aria-atomic="true"
        >
          {safeLogout.message}
        </div>

        {safeLogout.state === 'online' && safeLogout.canRequest && (
          <div style={labelStyle}>Protect your settlement before ending this session.</div>
        )}

        {pending && (
          <div style={countdownStyle} aria-live={SAFE_LOGOUT_ARIA_LIVE} aria-atomic="true">
            {safeLogout.countdownLabel || 'Countdown updating…'}
          </div>
        )}

        {safeLogout.state === 'online' && !safeLogout.activeAssault && (
          <div id="safe-logout-conditions" style={contractStyle}>{SAFE_LOGOUT_CONDITIONS}</div>
        )}

        {safeLogout.state === 'online' && !safeLogout.activeAssault && (
          <button
            type="button"
            style={requestDisabled ? disabledButtonStyle : buttonStyle}
            onClick={this.handleBeginSafeLogout}
            disabled={requestDisabled}
            aria-label="Begin Safe Logout countdown"
            aria-describedby="safe-logout-message safe-logout-conditions"
            title={requestDisabled ? safeLogout.message : 'Begin the server-authoritative Safe Logout countdown'}
          >
            {this.state.safeLogoutRequestInFlight ? 'Requesting…' : 'Begin Safe Logout'}
          </button>
        )}

        {pending && (
          <button
            type="button"
            style={cancelDisabled ? disabledButtonStyle : buttonStyle}
            onClick={this.handleCancelSafeLogout}
            disabled={cancelDisabled}
            aria-label="Cancel Safe Logout countdown"
            title="Cancel Safe Logout"
          >
            {this.state.safeLogoutCancelInFlight ? 'Cancelling…' : 'Cancel'}
          </button>
        )}
      </section>
    );
  }

  render() {
    const packetObjectives = this.state.objectiveState && this.state.objectiveState.objectives
      ? this.state.objectiveState.objectives
      : null;
    const objectives: ObjectiveProgress[] = packetObjectives || this.legacyObjectives();
    const activeObjective = this.activeObjective(objectives);
    const crisis = crisisStatusView(this.state.crisisStatus);
    const safeLogout = safeLogoutStatusView(this.state.safeLogoutStatus);

    // Threat Pressure and Discovery sections were intentionally removed from the
    // Survival Thread (too wordy for players). The data still arrives over the
    // wire and the handlers/state remain, so they can be re-added later.
    if (
      !shouldRenderSurvivalThread(Boolean(activeObjective), this.state.crisisStatus)
      && !shouldRenderSafeLogout(this.state.safeLogoutStatus)
    ) {
      return null;
    }

    const wide = isWideScreen();
    const layoutMode = safeLogoutLayoutMode(
      isDesktop(),
      wide,
      this.state.viewportWidth,
      COMPACT_DESKTOP_MAX_WIDTH,
    );
    const compactDesktop = layoutMode === 'compact';
    const compactExpanded = compactDesktop && this.state.compactExpanded;
    const panelChrome: React.CSSProperties = {
      backgroundColor: 'rgba(8, 10, 12, 0.82)',
      border: '1px solid rgba(201, 170, 113, 0.38)',
      borderRadius: '4px',
      zIndex: 50,
      boxSizing: 'border-box',
    };
    const containerStyle: React.CSSProperties = wide ? {
      ...panelChrome,
      position: 'fixed',
      top: 'calc(50% - 500px)',
      left: 'calc(50% + 612px)',
      width: '290px',
      maxHeight: '1000px',
      overflowY: 'auto',
      padding: '9px 10px',
      pointerEvents: 'auto',
    } : compactDesktop ? {
      ...panelChrome,
      position: 'fixed',
      bottom: DESKTOP_THREAD_BOTTOM,
      right: '12px',
      width: compactExpanded ? '280px' : '260px',
      maxWidth: 'calc(100vw - 24px)',
      maxHeight: compactExpanded ? 'calc(100vh - 169px)' : '42px',
      overflowY: compactExpanded ? 'auto' : 'hidden',
      padding: compactExpanded ? '8px 10px' : '7px 9px',
      pointerEvents: 'auto',
    } : {
      ...panelChrome,
      position: 'fixed',
      bottom: DESKTOP_THREAD_BOTTOM,
      right: '12px',
      width: '290px',
      maxWidth: 'calc(100vw - 24px)',
      padding: '9px 10px',
      // QW1: keep the panel interactive (scroll/click) on standard desktops
      // instead of letting pointer events fall through and disable it.
      pointerEvents: 'auto',
    };

    const titleStyle: React.CSSProperties = {
      color: '#c9aa71',
      fontFamily: 'Verdana',
      fontSize: '11px',
      fontWeight: 'bold',
      marginBottom: '6px',
      textTransform: 'uppercase',
      letterSpacing: 0,
    };

    const categoryStyle: React.CSSProperties = {
      color: '#8fb7d9',
      fontFamily: 'Verdana',
      fontSize: '9px',
      fontWeight: 'bold',
      textTransform: 'uppercase',
      letterSpacing: 0,
      marginBottom: '2px',
    };

    const activeTitleStyle: React.CSSProperties = {
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '13px',
      fontWeight: 'bold',
      marginBottom: '4px',
    };

    const bodyStyle: React.CSSProperties = {
      color: '#d4d4d4',
      fontFamily: 'Verdana',
      fontSize: '10px',
      lineHeight: 1.32,
      marginBottom: '4px',
    };

    const labelStyle: React.CSSProperties = {
      color: '#9aa0a6',
      fontFamily: 'Verdana',
      fontSize: '9px',
      lineHeight: 1.25,
      marginTop: '2px',
    };

    const sectionStyle: React.CSSProperties = {
      borderTop: '1px solid rgba(255,255,255,0.14)',
      marginTop: '7px',
      paddingTop: '7px',
    };

    const compactHeaderStyle: React.CSSProperties = {
      display: 'flex',
      alignItems: 'center',
      gap: '8px',
      width: '100%',
      border: 0,
      background: 'transparent',
      color: 'inherit',
      padding: 0,
      margin: 0,
      marginBottom: compactExpanded ? '7px' : 0,
      textAlign: 'left',
      cursor: 'pointer',
      fontFamily: 'Verdana',
      boxSizing: 'border-box',
    };

    const compactTitleStyle: React.CSSProperties = {
      ...titleStyle,
      marginBottom: 0,
      flex: '0 0 auto',
    };

    const compactObjectiveStyle: React.CSSProperties = {
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '10px',
      lineHeight: 1.2,
      minWidth: 0,
      overflow: 'hidden',
      textOverflow: 'ellipsis',
      whiteSpace: 'nowrap',
      flex: '1 1 auto',
    };

    const compactToggleStyle: React.CSSProperties = {
      color: '#c9aa71',
      fontFamily: 'Verdana',
      fontSize: '13px',
      fontWeight: 'bold',
      lineHeight: 1,
      flex: '0 0 auto',
    };

    const safeLogoutCompactSummary = safeLogout
      ? safeLogout.pending
        ? safeLogout.countdownLabel || 'Safe Logout pending'
        : safeLogout.inOwnSanctuary
          ? 'Safe Logout available'
          : safeLogout.reasonMessage || ''
      : '';
    const compactSummary = crisis && crisis.urgent
      ? crisis.compactLabel || crisis.phaseLabel
      : safeLogout && safeLogout.pending
        ? safeLogoutCompactSummary
        : activeObjective
          ? activeObjective.title
          : safeLogoutCompactSummary || (crisis ? crisis.title : '');

    const objectiveRowStyle = (state: string): React.CSSProperties => ({
      display: 'flex',
      justifyContent: 'space-between',
      gap: '8px',
      color: state == 'complete' ? '#8fbf88' : state == 'active' ? '#f2e7cf' : '#777d82',
      fontFamily: 'Verdana',
      fontSize: '9px',
      lineHeight: 1.25,
      padding: '1px 0',
    });

    const riskColor = (severity: string) => {
      if (severity == 'high' || severity == 'crisis') return '#ffb36b';
      if (severity == 'medium') return '#f2d27a';
      if (severity == 'low') return '#9fcf95';
      return '#9aa0a6';
    };

    return (
      <div style={containerStyle}>
        {compactDesktop ?
          <button
            type="button"
            style={compactHeaderStyle}
            onClick={this.toggleCompactExpanded}
            aria-expanded={compactExpanded}
            aria-label={compactExpanded
              ? 'Collapse survival thread'
              : `Expand survival thread${compactSummary ? `: ${compactSummary}` : ''}`}
            title={compactExpanded ? 'Collapse survival thread' : 'Expand survival thread'}
          >
            <span style={compactTitleStyle}>Survival Thread</span>
            {!compactExpanded && compactSummary &&
              <span style={compactObjectiveStyle}>{compactSummary}</span>}
            <span style={compactToggleStyle}>{compactExpanded ? '-' : '+'}</span>
          </button>
          :
          <div style={titleStyle}>Survival Thread</div>}

        {(!compactDesktop || compactExpanded) && crisis &&
          this.renderCrisisCard(crisis, bodyStyle, labelStyle)}

        {(!compactDesktop || compactExpanded) && safeLogout
          && shouldRenderSafeLogout(this.state.safeLogoutStatus)
          && this.renderSafeLogoutCard(safeLogout, bodyStyle, labelStyle)}

        {(!compactDesktop || compactExpanded) && activeObjective &&
          <div>
            <div style={activeTitleStyle}>{activeObjective.title}</div>
            <div style={bodyStyle}>{activeObjective.action_hint}</div>
            {this.renderProgress(activeObjective, labelStyle)}
          </div>}

        {(!compactDesktop || compactExpanded) && <div style={sectionStyle}>
          {objectives.map(obj => {
            const rowTooltip = [obj.action_hint, obj.lesson, obj.reward ? `Reward: ${obj.reward}` : '']
              .filter(Boolean)
              .join('\n');
            return (
              <div key={obj.id} style={objectiveRowStyle(obj.state)} title={rowTooltip}>
                <span>{obj.title}</span>
                <span>{obj.state == 'complete' ? 'Done' : obj.state == 'active' ? 'Next' : 'Later'}</span>
              </div>
            );
          })}
        </div>}

      </div>
    );
  }
}
