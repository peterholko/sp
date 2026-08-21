
import * as React from "react";
import styles from "./../ui.module.css";
import cancelbutton from "ui_comp/exitbutton.png";
import crafticon from "ui_comp/craftbutton.png";
import refineicon from "ui_comp/refinebutton.png";
import refineoreicon from "ui_comp/refineorebutton.png";
import refinewoodicon from "ui_comp/refinewoodbutton.png";
import refinestoneicon from "ui_comp/refinestonebutton.png";
import operateicon from "ui_comp/gatherbutton.png";
import refinegameanimalicon from "ui_comp/refinegameanimalbutton.png";
import experimenticon from "ui_comp/experimentbutton.png";
import frame from "ui_comp/itemframe.png";
import { Global } from "../../core/global";
import WorkQueueProgressBar from "../../core/workQueueProgressBar";

interface WorkQueueEntryProps {
  xPos,
  yPos,
  name,
  workType,
  villagerId,
  imageName,
  index,
  maxProgress,
  progress,
  actionId?,
  actionDurationMs?,
  actionElapsedMs?,
  refineItemClass?,
  handleClick?,
  handleCancel?
}

export default class WorkQueueEntry extends React.Component<WorkQueueEntryProps, any> {
  constructor(props) {
    super(props);

    this.handleClick = this.handleClick.bind(this);
    this.handleItemClick = this.handleItemClick.bind(this);
    this.handleWorkerClick = this.handleWorkerClick.bind(this);
    this.handleCancel = this.handleCancel.bind(this);
  }

  handleClick = () => {
    this.props.handleClick(this.props.index)
  }

  handleItemClick = () => {
    console.log('handleItemClick in workQueueEntry');
    Global.network.sendInfoItemByName(this.props.name);
  }

  handleWorkerClick = () => {
    console.log('handleWorkerClick in workQueueEntry');
    Global.network.sendInfoObj(this.props.villagerId);
  }

  handleCancel = () => {
    console.log('handleCancel in workQueueEntry');
    this.props.handleCancel(this.props.index)
  }

  render() {

    //31px -286px
    const divStyle = {
      transform: 'translate(' + this.props.xPos + 'px, ' + this.props.yPos + 'px)',
      position: 'fixed'
    } as React.CSSProperties

    const cancelStyle = {
      transform: 'translate(10px, -55px)',
      position: 'fixed',
      width: '25px',
      height: '25px'
    } as React.CSSProperties

    const itemFrameStyle = {
      transform: 'translate(43px, -66px)',
      position: 'fixed'
    } as React.CSSProperties

    const imageStyle = {
      transform: 'translate(45px, -65px)',
      position: 'fixed'
    } as React.CSSProperties

    const assignedFrameStyle = {
      transform: 'translate(165px, -66px)',
      position: 'fixed'
    } as React.CSSProperties

    const workTypeIconStyle = {
      transform: 'translate(105px, -66px)',
      position: 'fixed'
    } as React.CSSProperties

    const villagerFrameStyle = {
      transform: 'translate(150px, -79px)',
      position: 'fixed'
    } as React.CSSProperties

    const progressStyle = {
      transform: 'translate(225px, -50px)',
      position: 'fixed',
      width: '60px',
    } as React.CSSProperties

    let workTypeIcon = null;

    if (this.props.workType == 'Craft') {
      workTypeIcon = crafticon;
    } else if (this.props.workType == 'Refine') {
      if (this.props.refineItemClass == 'Ore') {
        workTypeIcon = refineicon;
      } else if (this.props.refineItemClass == 'Log') {
        workTypeIcon = refinewoodicon;
      } else if (this.props.refineItemClass == 'Stone') {
        workTypeIcon = refinestoneicon;
      } else if (this.props.refineItemClass == 'Game Animal' ||
        this.props.refineItemClass == 'Carcass') {
        workTypeIcon = refinegameanimalicon;
      }
    } else if (this.props.workType == 'Experiment') {
      workTypeIcon = experimenticon;
    } else if (this.props.workType == 'Operate') {
      workTypeIcon = operateicon;
    }

    let villagerImageName = null;

    if (Global.objectStates[this.props.villagerId]) {
      villagerImageName = Global.objectStates[this.props.villagerId].image + '_single.png';
    }

    console.log('-----  Index: ' + this.props.index + '  -----');
    return (
      <div style={divStyle}>
        <img src={cancelbutton} style={cancelStyle} onClick={this.handleCancel} />
        <img src={frame} style={itemFrameStyle} />
        <img src={'/static/art/items/' + this.props.imageName} style={imageStyle} onClick={this.handleItemClick} />
        <img src={workTypeIcon} style={workTypeIconStyle} />
        <img src={frame} style={assignedFrameStyle} />
        {villagerImageName && <img src={'/static/art/' + villagerImageName} style={villagerFrameStyle} onClick={this.handleWorkerClick} />}
        <WorkQueueProgressBar
          action_id={this.props.actionId}
          action_duration_ms={this.props.actionDurationMs}
          action_elapsed_ms={this.props.actionElapsedMs}
          style={progressStyle}
          label={this.props.name + ' progress'} />
      </div>
    );
  }
}
