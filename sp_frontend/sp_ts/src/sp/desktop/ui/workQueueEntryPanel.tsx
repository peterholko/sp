import * as React from "react";
import HalfPanel from "./halfPanel";
import WorkQueueProgressBar from "../../core/workQueueProgressBar";

interface WorkQueueEntryPanelProps {
  workQueueEntryData,
}

export default class WorkQueueEntryPanel extends React.Component<WorkQueueEntryPanelProps, any> {
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
              <td>
                <WorkQueueProgressBar
                  action_id={this.props.workQueueEntryData.action_id}
                  action_duration_ms={this.props.workQueueEntryData.action_duration_ms}
                  action_elapsed_ms={this.props.workQueueEntryData.action_elapsed_ms}
                  label={this.props.workQueueEntryData.item_name + ' progress'} />
              </td>
            </tr>
          </tbody>
        </table>

      </HalfPanel>
    );
  }
}



