import * as React from "react";
import HalfPanel from "./halfPanel";
import { Global } from "../global";
import WorkQueueEntry from "./workQueueEntry";
import { Util } from "../util";
import { GameEvent } from "../gameEvent";

interface WorkQueueEntryPanelProps {
  workQueueEntryData,
}

export default class WorkQueueEntryPanel extends React.Component<WorkQueueEntryPanelProps, any> {
  private timer;

  constructor(props) {
    super(props);

    this.state = {
      maxProgress: this.props.workQueueEntryData.work_time,
      progress: this.props.workQueueEntryData.progress,
    };

    this.startTimer = this.startTimer.bind(this)
    this.stopTimer = this.stopTimer.bind(this)
  }

  componentDidMount() {
    this.startTimer();
  }

  componentWillUnmount() {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  startTimer() {
    console.log('Start Timer Work Queue Entry Panel');
    this.timer = setInterval(() => {
      console.log("progress: " + this.state.progress);
      console.log("maxProgress: " + this.state.maxProgress);

      if (this.state.progress >= this.state.maxProgress) {
        console.log('progress >>> maxProgress');
        this.stopTimer();
      } else {
        this.setState({ progress: this.state.progress + 1 });
      }
    }, 1000);
  }

  stopTimer() {
    console.log('Stop Timer Work Queue Entry Panel');
    clearInterval(this.timer)
    this.timer = null;
  }

  render() {
    const spanNameStyle = {
      transform: 'translate(-323px, 25px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    const itemStyle = {
      transform: 'translate(-185px, 75px)',
      position: 'fixed'
    } as React.CSSProperties

    const itemNameStyle = {
      transform: 'translate(-323px, 125px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    const workQueueEntryTableStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '0px',
      position: 'fixed',
      textAlign: 'left',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '200px',
      transform: 'translate(50px, 300px)',
      zIndex: 8,
      userSelect: 'none'
    } as React.CSSProperties

    return (
      <HalfPanel left={false}
        panelType={'workqueueentry'}
        hideExitButton={false}>
        <span style={spanNameStyle}>
          {this.props.workQueueEntryData.work_type}
        </span>

        <img src={'/static/art/items/' + this.props.workQueueEntryData.item_image + '.png'} style={itemStyle} />

        <span style={itemNameStyle}>
          {this.props.workQueueEntryData.item_name}
        </span>

        <table style={workQueueEntryTableStyle}>
          <tbody>
            <tr>
              <td>Progress: </td>
              <td><progress max={this.state.maxProgress} value={this.state.progress}>{this.state.progress}</progress></td>
            </tr>
          </tbody>
        </table>

      </HalfPanel>
    );
  }
}




