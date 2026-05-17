
import * as React from "react";

interface WorldProps {
  worldData,
}

export default class WorldPanel extends React.Component<WorldProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };
   
  }

  render() {
    const isLandscape = window.innerWidth > window.innerHeight;
    const textAlign = isLandscape ? 'left' : 'center';

    const divStyle = {
      position: 'fixed',
      left: isLandscape ? '155px' : 'calc(158px + env(safe-area-inset-left, 0px))',
      bottom: 'calc(10px + env(safe-area-inset-bottom, 0px))',
      width: '88px',
      minHeight: '36px',
      zIndex: 4,
      border: '1px solid rgba(201, 170, 113, 0.38)',
      borderRadius: '4px',
      background: 'rgba(8, 10, 12, 0.74)',
      boxSizing: 'border-box',
      padding: '4px 6px',
    } as React.CSSProperties

    const timeOfDayStyle = {
      display: 'block',
      textAlign,
      color: '#f2e7cf',
      fontFamily: 'Cinzel',
      fontSize: '12px',
      lineHeight: 1.2,
    } as React.CSSProperties

    const dayStyle = {
      display: 'block',
      textAlign,
      color: '#c9aa71',
      fontFamily: 'Cinzel',
      fontSize: '11px',
      lineHeight: 1.2,
      fontWeight: 'bold',
    } as React.CSSProperties

    return (
      <div style={divStyle}>
        <span style={dayStyle}>Day {this.props.worldData.day}</span>
        <span style={timeOfDayStyle}>{this.props.worldData.time_of_day}</span>
      </div>
    );
  }
}
