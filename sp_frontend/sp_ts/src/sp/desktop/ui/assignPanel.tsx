import * as React from "react";
import HalfPanel from "./halfPanel";
import { Global } from "../../core/global";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import cancelbutton from "ui_comp/exitbutton.png";
import okbutton from "ui_comp/okbutton.png";
import unitframe from "ui_comp/itemframe.png";
import { BUILDING, FOUNDED, UPGRADING } from "../../core/config";

interface AssignPanelProps {
  structureData,
  assignData,
}

export default class AssignPanel extends React.Component<AssignPanelProps, any> {
  constructor(props) {
    super(props);

    this.state = {
      worker: this.props.assignData[0],
      index: 0
    };

    this.handleLeftClick = this.handleLeftClick.bind(this);
    this.handleRightClick = this.handleRightClick.bind(this);
    this.handleOkClick = this.handleOkClick.bind(this);
    this.handleCancelClick = this.handleCancelClick.bind(this);
  }

  handleLeftClick(event) {
    if (this.state.index != 0) {
      const newIndex = this.state.index - 1;
      this.setState({
        worker: this.props.assignData[newIndex],
        index: newIndex
      })
    }
  }

  handleRightClick(event) {
    if (this.state.index != (this.props.assignData.length - 1)) {
      const newIndex = this.state.index + 1;
      this.setState({
        worker: this.props.assignData[newIndex],
        index: newIndex
      })
    }
  }

  handleOkClick() {
    Global.network.sendAssign(this.state.worker.id, this.props.structureData.id);
  }

  handleCancelClick() {
    Global.network.sendRemoveAssign(this.state.worker.id, this.props.structureData.id);
  }

  render() {
    var imageName = this.state.worker.image.toLowerCase() + '_single.png';

    const workspaces = [];
    const assignments = [];

    for (var i = 0; i < this.props.assignData.length; i++) {
      if (this.props.assignData[i].structure_id == this.props.structureData.id) {
        assignments.push(this.props.assignData[i]);
      }
    }

    // For structures under construction, workspaces number equals
    const structureState = this.props.structureData.state;
    let totalWorkspaces = 0;

    if (structureState == FOUNDED || structureState == BUILDING || structureState == UPGRADING) {
      totalWorkspaces = assignments.length;
    }
    else {
      totalWorkspaces = this.props.structureData.workspaces;
    }

    for (var i = 0; i < totalWorkspaces; i++) {

      const frameStyle = {
        transform: 'translate(0px, 0px)',
        position: 'fixed'
      } as React.CSSProperties

      const imageStyle = {
        transform: 'translate(-14px, -12px)',
        position: 'fixed'
      } as React.CSSProperties

      const workspaceStyle = {
        transform: 'translate(58px, ' + (-250 + (i * 80)) + 'px)',
        position: 'fixed'
      } as React.CSSProperties

      if (assignments.length > 0) {
        // Pop an assignment off the assignments array
        const assignment = assignments.shift();

        const workspaceSpanNameStyle = {
          transform: 'translate(70px, 15px)',
          position: 'fixed',
          textAlign: 'left',
          color: 'white',
          fontFamily: 'Verdana',
          fontSize: '12px',
          width: '200px'
        } as React.CSSProperties

        var cancelStyle = {
          transform: 'translate(-36px, 8px)',
          position: 'fixed',
          width: '30px',
          height: '30px'
        } as React.CSSProperties

        workspaces.push(
          <div key={i} style={workspaceStyle}>
            <img src={cancelbutton} style={cancelStyle} onClick={this.handleCancelClick} />
            <img src={unitframe} style={frameStyle} />
            <img src={'/static/art/' + assignment.image + '_single.png'} style={imageStyle} />
            <span style={workspaceSpanNameStyle}>
              {assignment.name}
            </span>
          </div>);
      }
      else {
        const workspaceSpanNameStyle = {
          transform: 'translate(20px, 15px)',
          position: 'fixed',
          textAlign: 'left',
          color: 'white',
          fontFamily: 'Verdana',
          fontSize: '12px',
          width: '200px'
        } as React.CSSProperties

        workspaces.push(
          <div key={i} style={workspaceStyle}>
            <img src={unitframe} />
            <span style={workspaceSpanNameStyle}>
              No Worker Assigned
            </span>
          </div>);
      }
    }

    const imageStyle = {
      transform: 'translate(-195px, 25px)',
      position: 'fixed'
    } as React.CSSProperties

    const spanNameStyle = {
      transform: 'translate(-323px, 100px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    const tableStyle = {
      transform: 'translate(20px, -230px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const tableStyle2 = {
      transform: 'translate(-80px, 10px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const leftStyle = {
      transform: 'translate(-250px, 40px)',
      position: 'fixed'
    } as React.CSSProperties

    const rightStyle = {
      transform: 'translate(-115px, 40px)',
      position: 'fixed'
    } as React.CSSProperties

    const okButtonStyle = {
      transform: 'translate(-186px, 265px)',
      position: 'fixed'
    } as React.CSSProperties

    const structureSpriteStyle = {
      transform: 'translate(-275px, 15px)',
      position: 'fixed'
    } as React.CSSProperties

    const structureSpanNameStyle = {
      transform: 'translate(-200px, 40px)',
      position: 'fixed',
      textAlign: 'left',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '200px'
    } as React.CSSProperties

    var cancelStyle = {
      transform: 'translate(-200px, 175px)',
      position: 'fixed',
      width: '20px',
      height: '20px'
    } as React.CSSProperties

    return (
      <div>
        <HalfPanel left={true}
          panelType={'assign'}
          hideExitButton={true}>
          <img src={'/static/art/' + this.props.structureData.image + '.png'} style={structureSpriteStyle} />
          <span style={structureSpanNameStyle}>
            {this.props.structureData.name}
          </span>

          {workspaces}

        </HalfPanel>
        <HalfPanel left={false}
          panelType={'assign'}
          hideExitButton={false}>
          <img src={'/static/art/' + imageName} style={imageStyle} />
          <span style={spanNameStyle}>
            {this.state.worker.name}
          </span>
          <table style={tableStyle}>
            <tbody>
              <tr>
                <td>Assigned To:</td>
                <td>{this.state.worker.structure}</td>
              </tr>
            </tbody>
          </table>
          <img src={leftbutton} style={leftStyle} onClick={this.handleLeftClick} />
          <img src={rightbutton} style={rightStyle} onClick={this.handleRightClick} />

          <img src={okbutton} style={okButtonStyle} onClick={this.handleOkClick} />
        </HalfPanel>
      </div>
    );
  }
}



