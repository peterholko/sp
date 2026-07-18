import * as React from "react";
import { Global } from "../../core/global";
import { NetworkEvent } from "../../core/networkEvent";
import {
  CrisisStatusPacket,
  CrisisStatusView,
  CrisisTone,
  crisisStatusView,
  normalizeCrisisStatus,
} from "../../core/crisisStatus";

interface ObjectiveProgress {
  id: string;
  title: string;
  state: string;
  category: string;
  target?: string;
  action_hint: string;
  lesson: string;
  blocker?: string;
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

interface ObjectivesState {
  build_campfire: boolean;
  build_3_structures: boolean;
  recruit_villager: boolean;
  explore_poi: boolean;
  survive_5_nights: boolean;
  objectiveState: any;
  threatState: any;
  discoveryEvent: any;
  crisisStatus: CrisisStatusPacket | null;
  expanded: boolean;
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
      expanded: false,
    };
    this.toggleExpanded = this.toggleExpanded.bind(this);
  }

  toggleExpanded() {
    this.setState({ expanded: !this.state.expanded });
  }

  componentDidMount() {
    Global.gameEmitter.on(NetworkEvent.OBJECTIVES, this.handleObjectives, this);
    Global.gameEmitter.on(NetworkEvent.OBJECTIVE_STATE, this.handleObjectiveState, this);
    Global.gameEmitter.on(NetworkEvent.THREAT_STATE, this.handleThreatState, this);
    Global.gameEmitter.on(NetworkEvent.DISCOVERY_EVENT, this.handleDiscoveryEvent, this);
    Global.gameEmitter.on(NetworkEvent.CRISIS_STATUS, this.handleCrisisStatus, this);
    Global.gameEmitter.on(NetworkEvent.INFO_TRUE_DEATH, this.handleRunReset, this);
    Global.gameEmitter.on(NetworkEvent.SELECT_CLASS, this.handleRunReset, this);
    Global.gameEmitter.on(NetworkEvent.FIRST_LOGIN, this.handleRunReset, this);
  }

  componentWillUnmount() {
    Global.gameEmitter.off(NetworkEvent.OBJECTIVES, this.handleObjectives, this);
    Global.gameEmitter.off(NetworkEvent.OBJECTIVE_STATE, this.handleObjectiveState, this);
    Global.gameEmitter.off(NetworkEvent.THREAT_STATE, this.handleThreatState, this);
    Global.gameEmitter.off(NetworkEvent.DISCOVERY_EVENT, this.handleDiscoveryEvent, this);
    Global.gameEmitter.off(NetworkEvent.CRISIS_STATUS, this.handleCrisisStatus, this);
    Global.gameEmitter.off(NetworkEvent.INFO_TRUE_DEATH, this.handleRunReset, this);
    Global.gameEmitter.off(NetworkEvent.SELECT_CLASS, this.handleRunReset, this);
    Global.gameEmitter.off(NetworkEvent.FIRST_LOGIN, this.handleRunReset, this);
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
    this.setState({ crisisStatus: normalizeCrisisStatus(message) });
  }

  handleRunReset() {
    this.setState({
      build_campfire: false,
      build_3_structures: false,
      recruit_villager: false,
      explore_poi: false,
      survive_5_nights: false,
      objectiveState: null,
      threatState: null,
      discoveryEvent: null,
      crisisStatus: null,
    });
  }

  legacyObjectives(): ObjectiveProgress[] {
    return [
      {
        id: 'build_campfire',
        title: 'Use the campfire',
        state: this.state.build_campfire ? 'complete' : 'active',
        category: 'Settlement',
        action_hint: 'Use the lit campfire beside your start.',
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

  renderCrisisSection(
    crisis: CrisisStatusView,
    sectionStyle: React.CSSProperties,
    bodyStyle: React.CSSProperties,
    labelStyle: React.CSSProperties,
  ) {
    const accent = crisisToneColor[crisis.tone];
    const crisisSectionStyle: React.CSSProperties = {
      ...sectionStyle,
      border: `1px solid ${accent}`,
      borderLeft: `3px solid ${accent}`,
      borderRadius: '4px',
      background: 'rgba(255,255,255,0.035)',
      padding: '8px 9px',
      marginTop: 0,
      marginBottom: '10px',
    };
    const crisisHeaderStyle: React.CSSProperties = {
      display: 'flex',
      alignItems: 'flex-start',
      justifyContent: 'space-between',
      gap: '8px',
      marginBottom: '5px',
    };
    const crisisTitleStyle: React.CSSProperties = {
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '13px',
      fontWeight: 'bold',
      lineHeight: 1.2,
    };
    const phaseStyle: React.CSSProperties = {
      color: accent,
      fontFamily: 'Verdana',
      fontSize: '9px',
      fontWeight: 'bold',
      lineHeight: 1.2,
      textTransform: 'uppercase',
      whiteSpace: 'nowrap',
    };
    const detailSectionStyle: React.CSSProperties = {
      borderTop: '1px solid rgba(255,255,255,0.12)',
      marginTop: '7px',
      paddingTop: '6px',
    };
    const detailHeadingStyle: React.CSSProperties = {
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '10px',
      fontWeight: 'bold',
      marginBottom: '3px',
    };
    const statusRowStyle: React.CSSProperties = {
      ...labelStyle,
      color: '#d4d4d4',
      display: 'flex',
      justifyContent: 'space-between',
      gap: '8px',
      marginTop: '5px',
    };
    const optionRowStyle: React.CSSProperties = {
      padding: '4px 0',
      borderTop: '1px solid rgba(255,255,255,0.07)',
    };
    const optionHeaderStyle: React.CSSProperties = {
      display: 'flex',
      justifyContent: 'space-between',
      gap: '8px',
      color: '#e2dacb',
      fontFamily: 'Verdana',
      fontSize: '9px',
      fontWeight: 'bold',
      lineHeight: 1.25,
    };
    const preparationStateColor = (state: string) => {
      if (state === 'ready') return '#8fbf88';
      if (state === 'needs_attention') return '#f2d27a';
      return '#a9adb1';
    };
    const urgentStyle: React.CSSProperties = {
      ...bodyStyle,
      color: crisis.assaultActive ? '#ffaaaa' : accent,
      fontWeight: 'bold',
      marginTop: '6px',
      marginBottom: 0,
    };

    return (
      <section
        style={crisisSectionStyle}
        role="region"
        aria-label={crisis.statusAriaLabel}
      >
        <div
          style={crisisHeaderStyle}
          aria-live={crisis.urgent ? 'assertive' : 'polite'}
          aria-atomic="true"
        >
          <div style={crisisTitleStyle}>{crisis.title}</div>
          <div style={phaseStyle}>
            {crisis.phaseLabel}{crisis.warning ? ' · Warning' : ''}
          </div>
        </div>

        {crisis.summary && <div style={bodyStyle}>{crisis.summary}</div>}
        {crisis.actionHint &&
          <div style={labelStyle}><strong>Next:</strong> {crisis.actionHint}</div>}

        {crisis.preparationLabel &&
          <div style={statusRowStyle}>
            <span>Minimum warning</span>
            <span>{crisis.preparationLabel}</span>
          </div>}

        {crisis.preparationOptions.length > 0 &&
          <section style={detailSectionStyle} aria-label="Settlement preparation">
            <div style={detailHeadingStyle}>Prepare your settlement</div>
            {crisis.preparationOptions.map((option) =>
              <div key={option.id} style={optionRowStyle}>
                <div style={optionHeaderStyle}>
                  <span>{option.label}</span>
                  <span style={{ color: preparationStateColor(option.state) }}>
                    {option.stateLabel}
                  </span>
                </div>
                {option.detail && <div style={labelStyle}>{option.detail}</div>}
                {option.actionHint &&
                  <div style={labelStyle}><strong>Action:</strong> {option.actionHint}</div>}
              </div>)}
          </section>}

        {crisis.assaultActive &&
          <div style={statusRowStyle}>
            <span>Attackers remaining</span>
            <span>{crisis.attackersLabel || 'Updating'}</span>
          </div>}

        {crisis.assaultIntents.length > 0 &&
          <section style={detailSectionStyle} aria-label="Raid intents">
            <div style={detailHeadingStyle}>Raid intents</div>
            <dl style={{ margin: 0 }}>
              {crisis.assaultIntents.map((intent) =>
                <div key={intent.role} style={optionRowStyle}>
                  <dt style={optionHeaderStyle}>{intent.label}</dt>
                  <dd style={{ ...labelStyle, marginLeft: 0 }}>{intent.intent}</dd>
                </div>)}
            </dl>
          </section>}

        {crisis.disconnectedWarning &&
          <div style={urgentStyle}>{crisis.disconnectedWarning}</div>}

        {crisis.resolved &&
          <div style={urgentStyle}>Crisis resolved. Recover, repair, and rebuild.</div>}
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
    const threatState = this.state.threatState;
    const discoveryEvent = this.state.discoveryEvent;
    const sortedRisks = this.sortedRisks();
    const visibleRisks = sortedRisks.filter(risk => risk.severity != 'quiet').slice(0, 2);
    const risksToShow = visibleRisks.length > 0 ? visibleRisks : sortedRisks.slice(0, 2);
    const legendaryThreats: LegendaryThreat[] = threatState && threatState.legendary_threats
      ? threatState.legendary_threats
      : [];

    if (!activeObjective && !threatState && !discoveryEvent && !crisis) {
      return null;
    }

    const expanded = this.state.expanded;

    const chipStyle: React.CSSProperties = {
      position: 'fixed',
      right: 'calc(20px + env(safe-area-inset-right, 0px))',
      bottom: 'calc(166px + env(safe-area-inset-bottom, 0px))',
      width: '132px',
      backgroundColor: 'rgba(8, 10, 12, 0.82)',
      border: '1px solid rgba(201, 170, 113, 0.38)',
      borderRadius: '4px',
      zIndex: 50,
      pointerEvents: 'auto',
      boxSizing: 'border-box',
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '10px',
      lineHeight: 1.25,
      padding: '8px 9px',
      textAlign: 'center',
      boxShadow: '0 4px 14px rgba(0,0,0,0.35)',
    };

    const drawerStyle: React.CSSProperties = {
      position: 'fixed',
      top: 0,
      right: 0,
      bottom: 0,
      left: 0,
      zIndex: 70,
      pointerEvents: 'auto',
      boxSizing: 'border-box',
      display: 'flex',
      flexDirection: 'column',
      padding: 'calc(12px + env(safe-area-inset-top, 0px)) calc(10px + env(safe-area-inset-right, 0px)) calc(12px + env(safe-area-inset-bottom, 0px)) calc(10px + env(safe-area-inset-left, 0px))',
      backgroundColor: 'rgba(8, 10, 12, 0.97)',
      color: '#f2e7cf',
    };

    const headerStyle: React.CSSProperties = {
      flex: '0 0 auto',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      gap: '8px',
      color: '#c9aa71',
      fontFamily: 'Verdana',
      fontSize: '11px',
      fontWeight: 'bold',
      textTransform: 'uppercase',
      letterSpacing: 0,
      padding: '0 0 9px',
      minHeight: '44px',
      boxSizing: 'border-box',
      userSelect: 'none',
      borderBottom: '1px solid rgba(201, 170, 113, 0.35)',
    };

    const closeButtonStyle: React.CSSProperties = {
      flex: '0 0 auto',
      minHeight: '36px',
      minWidth: '64px',
      border: '1px solid rgba(201, 170, 113, 0.55)',
      borderRadius: '4px',
      background: '#25282b',
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '12px',
    };

    const bodyStylePanel: React.CSSProperties = {
      flex: '1 1 auto',
      minHeight: 0,
      overflowY: 'auto',
      WebkitOverflowScrolling: 'touch',
      padding: '12px 0 0',
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

    const crisisChipLabel = crisis
      ? crisis.compactLabel || crisis.phaseLabel || crisis.title
      : '';
    const chipLabel = crisis && crisis.urgent
      ? crisisChipLabel
      : activeObjective
        ? activeObjective.title
        : crisisChipLabel;

    if (!expanded) {
      return (
        <button
          type="button"
          style={chipStyle}
          onClick={this.toggleExpanded}
          aria-expanded={false}
        >
          <div style={{ color: '#c9aa71', fontWeight: 'bold', textTransform: 'uppercase' }}>Survival</div>
          {chipLabel && <div>{chipLabel}</div>}
        </button>
      );
    }

    return (
      <div style={drawerStyle}>
        <div style={headerStyle}>
          <span>Survival Thread</span>
          <button type="button" style={closeButtonStyle} onClick={this.toggleExpanded}>Close</button>
        </div>

        <div style={bodyStylePanel}>
            {crisis && this.renderCrisisSection(crisis, sectionStyle, bodyStyle, labelStyle)}

            {activeObjective &&
              <div>
                <div style={activeTitleStyle}>{activeObjective.title}</div>
                <div style={bodyStyle}><strong>Why:</strong> {activeObjective.lesson}</div>
                <div style={bodyStyle}><strong>Next:</strong> {activeObjective.action_hint}</div>
                {activeObjective.blocker &&
                  <div style={bodyStyle}><strong>Blocked:</strong> {activeObjective.blocker}</div>}
                {this.renderProgress(activeObjective, labelStyle)}
              </div>}

            <div style={sectionStyle}>
              {objectives.map(obj => (
                <div key={obj.id} style={objectiveRowStyle(obj.state)}>
                  <span>{obj.title}</span>
                  <span>{obj.state == 'complete' ? 'Done' : obj.state == 'active' ? 'Next' : 'Later'}</span>
                </div>
              ))}
            </div>

            {threatState &&
              <div style={sectionStyle}>
                <div style={categoryStyle}>Threat Pressure: {threatState.pressure_level}</div>
                <div style={bodyStyle}>Day {threatState.day}, {threatState.phase}. {threatState.next_night_warning}</div>
                {risksToShow.map(risk => (
                  <div key={risk.id} style={labelStyle}>
                    <span style={{ color: riskColor(risk.severity), fontWeight: 'bold' }}>{risk.severity.toUpperCase()}</span>
                    {' '}{risk.label}
                    {typeof risk.current == 'number' && typeof risk.threshold == 'number' &&
                      <span> ({risk.current}/{risk.threshold})</span>}
                    <div>{risk.counter_hint}</div>
                  </div>
                ))}
                {legendaryThreats.map(threat => (
                  <div key={threat.name} style={labelStyle}>
                    <span style={{ color: threat.status == 'defeated' ? '#8fbf88' : '#ffb36b', fontWeight: 'bold' }}>
                      {threat.status.toUpperCase()}
                    </span>
                    {' '}{threat.name}
                    {threat.days_active > 0 && <span> day {threat.days_active}</span>}
                    <div>
                      {threat.hideout_known
                        ? `Hideout: ${threat.hideout_location || 'known'}`
                        : `Followers defeated: ${threat.followers_defeated}, captains: ${threat.captains_defeated}/2`}
                    </div>
                    {typeof threat.next_attack_eta == 'number' &&
                      <div>Next attack in {threat.next_attack_eta}s</div>}
                  </div>
                ))}
              </div>}

            {discoveryEvent &&
              <div style={sectionStyle}>
                <div style={categoryStyle}>Discovery: {discoveryEvent.title}</div>
                <div style={bodyStyle}>{discoveryEvent.result}</div>
                <div style={labelStyle}>Source: {discoveryEvent.unlock_source}</div>
              </div>}
        </div>
      </div>
    );
  }
}
