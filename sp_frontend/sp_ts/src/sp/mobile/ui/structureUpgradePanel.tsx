import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import { Global } from "../../core/global";
import upgradebutton from "ui_comp/upgradebutton.png";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import { GameEvent } from "../../core/gameEvent";
import {
  MobilePanelActions,
  MobileRequirementGrid,
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

interface SUPProps {
  upgradeData,
}

export default class StructureUpgradePanel extends React.Component<SUPProps, any> {
  constructor(props) {
    super(props);

    this.state = {
      upgradeStructure: this.props.upgradeData.upgrade_list[0],
      index: 0
    };

    this.handleLeftClick = this.handleLeftClick.bind(this);
    this.handleRightClick = this.handleRightClick.bind(this);
    this.handleUpgradeClick = this.handleUpgradeClick.bind(this);

  }

  handleLeftClick(event) {
    if (this.state.index != 0) {
      const newIndex = this.state.index - 1;
      this.setState({
        upgradeStructure: this.props.upgradeData.upgrade_list[newIndex],
        index: newIndex
      })
    }
  }

  handleRightClick(event) {
    if (this.state.index != (this.props.upgradeData.upgrade_list.length - 1)) {
      const newIndex = this.state.index + 1;
      this.setState({
        upgradeStructure: this.props.upgradeData.upgrade_list[newIndex],
        index: newIndex
      })
    }
  }

  handleUpgradeClick() {
    Global.network.sendStartUpgrade(this.props.upgradeData.id, this.state.upgradeStructure.name);
    Global.gameEmitter.emit(GameEvent.START_UPGRADE_CLICK, {});

    Global.selectedUpgrade = this.state.upgradeStructure.name;
  }

  render() {
    console.log(this.state);

    let structureImage = this.state.upgradeStructure.template.toLowerCase().replace(/\s/g, '');
    let structureImagePath = '/static/art/' + structureImage + '.png';

    let nextStructureName = this.state.upgradeStructure.name;

    let structure = this.props.upgradeData.upgrade_list[this.state.index];
    const reqs = structure.req || [];
    const landscape = isLandscapeMobile();
    const atFirst = this.state.index == 0;
    const atLast = this.state.index == (this.props.upgradeData.upgrade_list.length - 1);
    const actions = [
      { key: 'previous', label: 'Previous upgrade', icon: leftbutton, onClick: this.handleLeftClick, disabled: atFirst },
      { key: 'upgrade', label: 'Start upgrade', icon: upgradebutton, onClick: this.handleUpgradeClick },
      { key: 'next', label: 'Next upgrade', icon: rightbutton, onClick: this.handleRightClick, disabled: atLast },
    ];

    return (
      <MobilePanelScreen
        panelType={'upgrade'}
        title={'Upgrade'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={
            <>
              <MobileSummaryCard
                imageSrc={structureImagePath}
                title={nextStructureName}
                subtitle="Upgrade option"
                imageSize={landscape ? 64 : 88} />
              <MobileStatsList rows={[
                { label: 'Current', value: this.props.upgradeData.name || this.props.upgradeData.template || 'Structure' },
                { label: 'Option', value: `${this.state.index + 1} / ${this.props.upgradeData.upgrade_list.length}` },
              ]} />
            </>
          }
          right={
            <>
              <MobileRequirementGrid title="Requirements" requirements={reqs} />
              <MobilePanelActions actions={actions} />
            </>
          } />
      </MobilePanelScreen>
    );
  }
}
