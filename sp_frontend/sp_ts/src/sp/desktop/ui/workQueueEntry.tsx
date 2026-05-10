
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
  refineItemClass?,
  handleClick?,
  handleCancel?
}

export default class WorkQueueEntry extends React.Component<WorkQueueEntryProps, any> {
  private timer;

  constructor(props) {
    super(props);
    this.state = { progress: this.props.progress ?? 0 };

    this.startTimer = this.startTimer.bind(this);
    this.stopTimer = this.stopTimer.bind(this);

    this.handleClick = this.handleClick.bind(this);
    this.handleItemClick = this.handleItemClick.bind(this);
    this.handleWorkerClick = this.handleWorkerClick.bind(this);
    this.handleCancel = this.handleCancel.bind(this);
  }

  componentDidMount() {
    // Only start if there's actual work to do
    if (this.props.maxProgress > 0 && this.state.progress < this.props.maxProgress) {
      this.startTimer();
    }
  }

  componentWillUnmount() {
    this.stopTimer();
  }

  componentDidUpdate(prevProps: WorkQueueEntryProps) {
    const progressChanged = prevProps.progress !== this.props.progress;
    const maxAppeared = (prevProps.maxProgress ?? 0) <= 0 && (this.props.maxProgress ?? 0) > 0;
    const entryIdentityChanged =
      prevProps.index !== this.props.index ||
      prevProps.villagerId !== this.props.villagerId ||
      prevProps.name !== this.props.name ||
      prevProps.workType !== this.props.workType;

    // If the entry changed, sync local state to incoming props and (re)start as needed
    if (entryIdentityChanged) {
      this.stopTimer();
      this.setState({ progress: this.props.progress ?? 0 }, () => {
        if (this.props.maxProgress > 0 && this.state.progress < this.props.maxProgress) {
          this.startTimer();
        }
      });
      return;
    }

    // If parent sends new progress, mirror it
    if (progressChanged) {
      this.setState({ progress: this.props.progress ?? 0 }, () => {
        if (this.props.maxProgress > 0 && this.state.progress < this.props.maxProgress && !this.timer) {
          this.startTimer();
        }
      });
      return;
    }

    // If maxProgress changed from 0/undefined to a positive value, start the timer
    if (maxAppeared && !this.timer && (this.state.progress ?? 0) < (this.props.maxProgress ?? 0)) {
      this.startTimer();
    }
  }

  startTimer() {
    // Prevent duplicate intervals
    if (this.timer) this.stopTimer();

    if (!(this.props.maxProgress > 0)) return;

    this.timer = setInterval(() => {
      const max = this.props.maxProgress;
      const cur = this.state.progress ?? 0;

      if (cur + 1 >= max) {
        // Finish at max, then stop
        this.setState({ progress: 0 });
        this.stopTimer();
      } else {
        this.setState({ progress: cur + 1 });
      }
    }, 1000);
  }

  stopTimer() {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
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
      } else if (this.props.refineItemClass == 'Game Animal') {
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
    console.log('maxProgress Props: ' + this.props.maxProgress);
    console.log('progress Props: ' + this.props.progress + ' State: ' + this.state.progress);

    return (
      <div style={divStyle}>
        <img src={cancelbutton} style={cancelStyle} onClick={this.handleCancel} />
        <img src={frame} style={itemFrameStyle} />
        <img src={'/static/art/items/' + this.props.imageName} style={imageStyle} onClick={this.handleItemClick} />
        <img src={workTypeIcon} style={workTypeIconStyle} />
        <img src={frame} style={assignedFrameStyle} />
        {villagerImageName && <img src={'/static/art/' + villagerImageName} style={villagerFrameStyle} onClick={this.handleWorkerClick} />}
        {this.props.maxProgress > 0 && <progress max={this.props.maxProgress} value={this.state.progress} style={progressStyle}>{this.state.progress}</progress>}
      </div>
    );
  }
}

