import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import { Global } from "../../core/global";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import buildbutton from "ui_comp/buildbutton.png";
import { GameEvent } from "../../core/gameEvent";
import {
  MobilePanelActions,
  MobileRequirementGrid,
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

interface BuildPanelProps {
  structuresData,
}

export default class BuildPanel extends React.Component<BuildPanelProps, any> {
  constructor(props) {
    super(props);

    this.state = {
      structure : this.props.structuresData[0],
      index : 0
    };

    this.handleLeftClick = this.handleLeftClick.bind(this);
    this.handleRightClick = this.handleRightClick.bind(this);
    this.handleBuildClick = this.handleBuildClick.bind(this);
  }

  handleLeftClick(event) {
    if(this.state.index != 0) {
      const newIndex = this.state.index - 1;
      this.setState({structure: this.props.structuresData[newIndex],
                     index: newIndex})
    } 
  }

  handleRightClick(event) {
    if(this.state.index != (this.props.structuresData.length - 1)) {
      const newIndex = this.state.index + 1;
      this.setState({structure: this.props.structuresData[newIndex],
                     index: newIndex})
    } 
  }

  handleBuildClick() {
    Global.network.sendCreateFoundation(Global.heroId, this.state.structure.name);
    Global.gameEmitter.emit(GameEvent.START_BUILD_CLICK, {});
  }

  render() {
    var imageName = this.state.structure.image + '.png';
    const reqs = this.state.structure.req || [];
    const atFirst = this.state.index == 0;
    const atLast = this.state.index == (this.props.structuresData.length - 1);
    const landscape = isLandscapeMobile();
    const level = this.state.structure.level != null ? `Level ${this.state.structure.level}` : null;
    const actions = [
      { key: 'previous', label: 'Previous structure', icon: leftbutton, onClick: this.handleLeftClick, disabled: atFirst },
      { key: 'build', label: 'Build structure', icon: buildbutton, onClick: this.handleBuildClick },
      { key: 'next', label: 'Next structure', icon: rightbutton, onClick: this.handleRightClick, disabled: atLast },
    ];


    return (
      <MobilePanelScreen
        panelType={'build'}
        title={'Build'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={
            <>
              <MobileSummaryCard
                imageSrc={'/static/art/' + imageName}
                title={this.state.structure.name}
                subtitle={level}
                imageSize={landscape ? 64 : 88} />
              <MobileStatsList rows={[
                { label: 'Class', value: this.state.structure.subclass },
                { label: 'HP', value: this.state.structure.base_hp },
                { label: 'Defense', value: this.state.structure.base_def },
                { label: 'Build Time', value: this.state.structure.build_time },
              ]} />
            </>
          }
          right={
            <>
              <MobileRequirementGrid title="Materials" requirements={reqs} />
              <MobilePanelActions actions={actions} />
            </>
          } />
      </MobilePanelScreen>
    );
  }
}

