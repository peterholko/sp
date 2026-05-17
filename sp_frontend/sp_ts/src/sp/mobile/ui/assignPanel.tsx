import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import { Global } from "../../core/global";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import cancelbutton from "ui_comp/exitbutton.png";
import okbutton from "ui_comp/okbutton.png";
import { BUILDING, FOUNDED, UPGRADING } from "../../core/config";
import {
  MobileCard,
  MobilePanelActions,
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

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

    const landscape = isLandscapeMobile();
    const atFirst = this.state.index == 0;
    const atLast = this.state.index == (this.props.assignData.length - 1);

    const listStyle: React.CSSProperties = {
      display: 'flex',
      flexDirection: 'column',
      gap: '7px',
    };

    const rowStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: '28px 42px 1fr',
      alignItems: 'center',
      gap: '8px',
      minHeight: '48px',
      borderBottom: '1px solid rgba(255,255,255,0.08)',
      paddingBottom: '6px',
    };

    const workerIconStyle: React.CSSProperties = {
      width: '38px',
      height: '38px',
      objectFit: 'contain',
      imageRendering: 'pixelated',
    };

    const cancelStyle: React.CSSProperties = {
      width: '24px',
      height: '24px',
    };

    const emptyStyle: React.CSSProperties = {
      color: '#777d82',
      fontFamily: 'Verdana',
      fontSize: '11px',
      lineHeight: 1.3,
    };

    return (
      <MobilePanelScreen
        panelType={'assign'}
        title={'Assign'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={
            <>
              <MobileSummaryCard imageSrc={'/static/art/' + this.props.structureData.image + '.png'} title={this.props.structureData.name} subtitle={`${assignments.length} / ${totalWorkspaces} assigned`} imageSize={landscape ? 58 : 82} />
              <MobileCard compact={landscape}>
                <div style={listStyle}>
                  {Array.from({ length: totalWorkspaces }).map((_, index) => {
                    const assignment = assignments[index];

                    if (!assignment) {
                      return <div key={index} style={emptyStyle}>Workspace {index + 1}: No worker assigned</div>;
                    }

                    return (
                      <div key={index} style={rowStyle}>
                        <img src={cancelbutton} style={cancelStyle} onClick={() => Global.network.sendRemoveAssign(assignment.id, this.props.structureData.id)} />
                        <img src={'/static/art/' + assignment.image + '_single.png'} style={workerIconStyle} />
                        <div style={emptyStyle}>{assignment.name}</div>
                      </div>
                    );
                  })}
                </div>
              </MobileCard>
            </>
          }
          right={
            <>
              <MobileSummaryCard imageSrc={'/static/art/' + imageName} title={this.state.worker.name} subtitle="Selected Worker" imageSize={landscape ? 58 : 82} />
              <MobileStatsList rows={[
                { label: 'Assigned To', value: this.state.worker.structure },
              ]} />
              <MobilePanelActions actions={[
                { key: 'previous', label: 'Previous worker', icon: leftbutton, onClick: this.handleLeftClick, disabled: atFirst },
                { key: 'assign', label: 'Assign worker', icon: okbutton, onClick: this.handleOkClick },
                { key: 'next', label: 'Next worker', icon: rightbutton, onClick: this.handleRightClick, disabled: atLast },
              ]} />
            </>
          } />
      </MobilePanelScreen>
    );
  }
}


