import * as React from "react";
import HalfPanel from "./halfPanel";
import { Global } from "../global";
import WorkQueueEntry from "./workQueueEntry";
import { Util } from "../util";
import { GameEvent } from "../gameEvent";

interface WorkQueuePanelProps {
  structureData,
  workQueue,
}

export default class WorkQueuePanel extends React.Component<WorkQueuePanelProps, any> {
  constructor(props) {
    console.log('WorkQueuePanel constructor');
    super(props);

    this.state = {
    };

    this.handleClick = this.handleClick.bind(this);
    this.handleCancel = this.handleCancel.bind(this);
  }

  handleClick(index) {
    Global.network.sendInfoWorkQueueEntry(this.props.structureData.id, index);
  }

  handleCancel(index) {
    console.log('handleCancel in workQueuePanel');
    Global.network.sendRemoveWorkEntry(this.props.structureData.id, index);
  }

  render() {
    console.log('WorkQueuePanel render');
    const workQueue = [];
    let structureImageName;
    let structureName;

    if (Global.objectStates[this.props.structureData.id]) {
      if (Util.isSprite(Global.objectStates[this.props.structureData.id].image)) {
        structureImageName = Global.objectStates[this.props.structureData.id].image + '_single.png';
      } else {
        structureImageName = Global.objectStates[this.props.structureData.id].image + '.png';
      }

      structureName = Global.objectStates[this.props.structureData.id].name;
    }

    for (var i = 0; i < this.props.workQueue.length; i++) {

      var xPos = 15;
      var yPos = -200 + (i * 60);

      const workType = this.props.workQueue[i].work_type;

      if (workType == 'Craft') {
        const recipeName = this.props.workQueue[i].recipe_name;
        const imageName = this.props.workQueue[i].recipe_image + '.png';

        workQueue.push(<WorkQueueEntry
          key={i}
          index={i}
          workType={workType}
          villagerId={this.props.workQueue[i].villager_id}
          name={recipeName}
          imageName={imageName}
          xPos={xPos}
          yPos={yPos}
          maxProgress={this.props.workQueue[i].work_time}
          progress={this.props.workQueue[i].progress}
          handleCancel={this.handleCancel} />)
      } else if (workType == 'Refine') {
        const refineItemId = this.props.workQueue[i].refine_item_id;
        const imageName = this.props.workQueue[i].refine_item_image + '.png';

        workQueue.push(<WorkQueueEntry
          key={i}
          index={i}
          workType={workType}
          villagerId={this.props.workQueue[i].villager_id}
          name={refineItemId}
          imageName={imageName}
          xPos={xPos}
          yPos={yPos}
          refineItemClass={this.props.workQueue[i].refine_item_class}
          maxProgress={this.props.workQueue[i].work_time}
          progress={this.props.workQueue[i].progress}
          handleCancel={this.handleCancel} />)
      } else if (workType == 'Operate') {

        workQueue.push(<WorkQueueEntry
          key={i}
          index={i}
          workType={workType}
          villagerId={this.props.workQueue[i].villager_id}
          name={'valleyruncopperore'}
          imageName={'valleyruncopperore.png'}
          xPos={xPos}
          yPos={yPos}
          maxProgress={this.props.workQueue[i].work_time}
          progress={this.props.workQueue[i].progress}
          handleCancel={this.handleCancel} />)
        }
      }

      const spanNameStyle = {
        transform: 'translate(-323px, 25px)',
        position: 'fixed',
        textAlign: 'center',
        color: 'white',
        fontFamily: 'Verdana',
        fontSize: '12px',
        width: '323px'
      } as React.CSSProperties

      const spanNoWorkStyle = {
        transform: 'translate(-323px, 75px)',
        position: 'fixed',
        textAlign: 'center',
        color: 'white',
        fontFamily: 'Verdana',
        fontSize: '12px',
        width: '323px'
      } as React.CSSProperties

      const structureSpriteStyle = {
        transform: 'translate(-290px, 5px)',
        position: 'fixed'
      } as React.CSSProperties

      return (
        <HalfPanel left={true}
          panelType={'workqueue'}
          hideExitButton={false}>
          <img src={'/static/art/' + structureImageName} style={structureSpriteStyle} />
          <span style={spanNameStyle}>
            {structureName} Queue
          </span>

          {workQueue.length > 0 && workQueue}
          {workQueue.length == 0 && <span style={spanNoWorkStyle}>No work in queue</span>}

        </HalfPanel>
      );
    }
  }




