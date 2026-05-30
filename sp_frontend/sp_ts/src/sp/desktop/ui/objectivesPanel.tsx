import * as React from "react";
import { Global } from "../../core/global";
import { NetworkEvent } from "../../core/networkEvent";
import { isDesktop, isWideScreen } from "../../core/config";

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

interface ObjectivesState {
  build_campfire: boolean;
  build_3_structures: boolean;
  recruit_villager: boolean;
  explore_poi: boolean;
  survive_5_nights: boolean;
  objectiveState: any;
  threatState: any;
  discoveryEvent: any;
  viewportWidth: number;
  compactExpanded: boolean;
}

const severityRank = {
  crisis: 5,
  high: 4,
  medium: 3,
  low: 2,
  quiet: 1,
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
      viewportWidth: typeof window === 'undefined' ? 0 : window.innerWidth,
      compactExpanded: false,
    };

    this.handleResize = this.handleResize.bind(this);
    this.toggleCompactExpanded = this.toggleCompactExpanded.bind(this);
  }

  componentDidMount() {
    Global.gameEmitter.on(NetworkEvent.OBJECTIVES, this.handleObjectives, this);
    Global.gameEmitter.on(NetworkEvent.OBJECTIVE_STATE, this.handleObjectiveState, this);
    Global.gameEmitter.on(NetworkEvent.THREAT_STATE, this.handleThreatState, this);
    Global.gameEmitter.on(NetworkEvent.DISCOVERY_EVENT, this.handleDiscoveryEvent, this);

    if (typeof window !== 'undefined') {
      window.addEventListener('resize', this.handleResize);
    }
  }

  componentWillUnmount() {
    Global.gameEmitter.off(NetworkEvent.OBJECTIVES, this.handleObjectives, this);
    Global.gameEmitter.off(NetworkEvent.OBJECTIVE_STATE, this.handleObjectiveState, this);
    Global.gameEmitter.off(NetworkEvent.THREAT_STATE, this.handleThreatState, this);
    Global.gameEmitter.off(NetworkEvent.DISCOVERY_EVENT, this.handleDiscoveryEvent, this);

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

  render() {
    const packetObjectives = this.state.objectiveState && this.state.objectiveState.objectives
      ? this.state.objectiveState.objectives
      : null;
    const objectives: ObjectiveProgress[] = packetObjectives || this.legacyObjectives();
    const activeObjective = this.activeObjective(objectives);
    const threatState = this.state.threatState;
    const discoveryEvent = this.state.discoveryEvent;
    const sortedRisks = this.sortedRisks();
    const visibleRisks = sortedRisks.filter(risk => risk.severity != 'quiet').slice(0, 2);
    const risksToShow = visibleRisks.length > 0 ? visibleRisks : sortedRisks.slice(0, 2);
    const legendaryThreats: LegendaryThreat[] = threatState && threatState.legendary_threats
      ? threatState.legendary_threats
      : [];

    if (!activeObjective && !threatState && !discoveryEvent) {
      return null;
    }

    const wide = isWideScreen();
    const compactDesktop = isDesktop() && !wide && this.state.viewportWidth <= COMPACT_DESKTOP_MAX_WIDTH;
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
      pointerEvents: 'none',
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
            aria-label={compactExpanded ? 'Collapse survival thread' : 'Expand survival thread'}
          >
            <span style={compactTitleStyle}>Survival Thread</span>
            {!compactExpanded && activeObjective &&
              <span style={compactObjectiveStyle}>{activeObjective.title}</span>}
            <span style={compactToggleStyle}>{compactExpanded ? '-' : '+'}</span>
          </button>
          :
          <div style={titleStyle}>Survival Thread</div>}

        {(!compactDesktop || compactExpanded) && activeObjective &&
          <div>
            <div style={categoryStyle}>{activeObjective.category}</div>
            <div style={activeTitleStyle}>{activeObjective.title}</div>
            <div style={bodyStyle}>{activeObjective.action_hint}</div>
            <div style={labelStyle}>Lesson: {activeObjective.lesson}</div>
            <div style={labelStyle}>Payoff: {activeObjective.reward}</div>
            {this.renderProgress(activeObjective, labelStyle)}
          </div>}

        {(!compactDesktop || compactExpanded) && <div style={sectionStyle}>
          {objectives.map(obj => (
            <div key={obj.id} style={objectiveRowStyle(obj.state)}>
              <span>{obj.title}</span>
              <span>{obj.state == 'complete' ? 'Done' : obj.state == 'active' ? 'Next' : 'Later'}</span>
            </div>
          ))}
        </div>}

        {(!compactDesktop || compactExpanded) && threatState &&
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

        {(!compactDesktop || compactExpanded) && discoveryEvent &&
          <div style={sectionStyle}>
            <div style={categoryStyle}>Discovery: {discoveryEvent.title}</div>
            <div style={bodyStyle}>{discoveryEvent.result}</div>
            <div style={labelStyle}>Source: {discoveryEvent.unlock_source}</div>
          </div>}
      </div>
    );
  }
}
