
import * as React from "react";
import worldframe from "ui_comp/hpframe.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";

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

    const divStyle = {
      position: 'fixed',
      bottom: '75px',
      left: '78%',
      marginLeft: '0px',    
    } as React.CSSProperties

    const worldFrameStyle = {
      transform: 'translate(0px, 0px)',
      position: 'fixed'
    } as React.CSSProperties

    const timeOfDayStyle = {
      transform: 'translate(15px, 32px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Cinzel',
      fontSize: '16px',
      width: '160px'
    } as React.CSSProperties

    const dayStyle = {
      transform: 'translate(15px, 12px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Cinzel',
      fontSize: '16px',
      width: '160px'
    } as React.CSSProperties

    return (
      <div style={divStyle}>
        <img src={worldframe} style={worldFrameStyle}/>
        <span style={timeOfDayStyle}>{this.props.worldData.time_of_day}</span>
        <span style={dayStyle}>Day {this.props.worldData.day}</span>
      </div>
    );
  }
}

