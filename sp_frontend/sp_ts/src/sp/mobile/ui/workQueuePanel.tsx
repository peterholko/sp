import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import { Global } from "../../core/global";
import { Util } from "../../core/util";
import cancelbutton from "ui_comp/exitbutton.png";
import {
  MobileCard,
  MobileSplitPanelLayout,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

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
    const landscape = isLandscapeMobile();
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

    const listStyle: React.CSSProperties = {
      display: 'flex',
      flexDirection: 'column',
      gap: '7px',
    };

    const rowStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: '28px 42px 1fr 70px',
      alignItems: 'center',
      gap: '8px',
      minHeight: '50px',
      borderBottom: '1px solid rgba(255,255,255,0.08)',
      paddingBottom: '6px',
    };

    const cancelStyle: React.CSSProperties = {
      width: '24px',
      height: '24px',
    };

    const imageStyle: React.CSSProperties = {
      width: '38px',
      height: '38px',
      objectFit: 'contain',
      imageRendering: 'pixelated',
    };

    const nameStyle: React.CSSProperties = {
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '11px',
      lineHeight: 1.25,
      overflowWrap: 'anywhere',
    };

    const metaStyle: React.CSSProperties = {
      color: '#c9aa71',
      fontFamily: 'Verdana',
      fontSize: '10px',
      lineHeight: 1.2,
    };

    const progressStyle: React.CSSProperties = {
      width: '70px',
    };

    const emptyStyle: React.CSSProperties = {
      color: '#777d82',
      fontFamily: 'Verdana',
      fontSize: '11px',
      textAlign: 'center',
      padding: '12px 0',
    };

      return (
        <MobilePanelScreen
          panelType={'workqueue'}
          title={'Work Queue'}
          hideExitButton={false}
          contentStyle={landscape ? { padding: '8px 0' } : undefined}>
          <MobileSplitPanelLayout
            left={<MobileSummaryCard imageSrc={'/static/art/' + structureImageName} title={structureName || 'Structure'} subtitle="Queue" imageSize={landscape ? 58 : 82} />}
            right={
              <MobileCard compact={landscape}>
                {this.props.workQueue.length == 0 && <div style={emptyStyle}>No work in queue</div>}
                {this.props.workQueue.length > 0 &&
                  <div style={listStyle}>
                    {this.props.workQueue.map((entry, index) => {
                      const workType = entry.work_type;
                      let name = workType;
                      let imageName = 'recipe.png';

                      if (workType == 'Craft') {
                        name = entry.recipe_name;
                        imageName = entry.recipe_image + '.png';
                      } else if (workType == 'Refine') {
                        name = entry.refine_item_id;
                        imageName = entry.refine_item_image + '.png';
                      } else if (workType == 'Operate') {
                        name = 'Operate';
                        imageName = 'valleyruncopperore.png';
                      }

                      return (
                        <div key={index} style={rowStyle}>
                          <img src={cancelbutton} style={cancelStyle} onClick={() => this.handleCancel(index)} />
                          <img src={'/static/art/items/' + imageName} style={imageStyle} onClick={() => Global.network.sendInfoItemByName(name)} />
                          <div>
                            <div style={nameStyle}>{name}</div>
                            <div style={metaStyle}>{workType}</div>
                          </div>
                          {entry.work_time > 0 && <progress max={entry.work_time} value={entry.progress} style={progressStyle}>{entry.progress}</progress>}
                        </div>
                      );
                    })}
                  </div>}
              </MobileCard>
            } />
        </MobilePanelScreen>
      );
    }
  }



