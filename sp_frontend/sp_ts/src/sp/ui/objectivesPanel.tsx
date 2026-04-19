
import * as React from "react";
import { Global } from "../global";
import { NetworkEvent } from "../networkEvent";

interface ObjectivesState {
  build_campfire: boolean;
  build_3_structures: boolean;
  recruit_villager: boolean;
  explore_poi: boolean;
  survive_5_nights: boolean;
}

export default class ObjectivesPanel extends React.Component<{}, ObjectivesState> {
  constructor(props) {
    super(props);
    this.state = {
      build_campfire: false,
      build_3_structures: false,
      recruit_villager: false,
      explore_poi: false,
      survive_5_nights: false,
    };
  }

  componentDidMount() {
    Global.gameEmitter.on(NetworkEvent.OBJECTIVES, this.handleObjectives, this);
  }

  componentWillUnmount() {
    Global.gameEmitter.off(NetworkEvent.OBJECTIVES, this.handleObjectives, this);
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

  render() {
    const objectives = [
      { key: 'build_campfire', label: 'Build a Campfire', done: this.state.build_campfire },
      { key: 'recruit_villager', label: 'Recruit a Villager', done: this.state.recruit_villager },
      { key: 'explore_poi', label: 'Explore a Point of Interest', done: this.state.explore_poi },
      { key: 'build_3_structures', label: 'Build 3 Structures', done: this.state.build_3_structures },
      { key: 'survive_5_nights', label: 'Survive 5 Nights', done: this.state.survive_5_nights },
    ];

    const allComplete = objectives.every(o => o.done);

    if (allComplete) {
      return null;
    }

    const containerStyle: React.CSSProperties = {
      position: 'fixed',
      bottom: '150px',
      left: '78%',
      width: '220px',
      backgroundColor: 'rgba(0, 0, 0, 0.7)',
      borderRadius: '4px',
      padding: '8px 10px',
      zIndex: 50,
      pointerEvents: 'none',
    };

    const titleStyle: React.CSSProperties = {
      color: '#c9aa71',
      fontFamily: 'Verdana',
      fontSize: '11px',
      fontWeight: 'bold',
      marginBottom: '6px',
      textTransform: 'uppercase',
      letterSpacing: '1px',
    };

    const itemStyle = (done: boolean): React.CSSProperties => ({
      color: done ? '#5a8a5a' : '#d4d4d4',
      fontFamily: 'Verdana',
      fontSize: '10px',
      padding: '2px 0',
      textDecoration: done ? 'line-through' : 'none',
      opacity: done ? 0.6 : 1.0,
    });

    const checkStyle: React.CSSProperties = {
      marginRight: '6px',
      fontSize: '10px',
    };

    return (
      <div style={containerStyle}>
        <div style={titleStyle}>Objectives</div>
        {objectives.map(obj => (
          <div key={obj.key} style={itemStyle(obj.done)}>
            <span style={checkStyle}>{obj.done ? '\u2713' : '\u25CB'}</span>
            {obj.label}
          </div>
        ))}
      </div>
    );
  }
}
