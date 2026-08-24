import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import { Global } from "../../core/global";
import upgradebutton from "ui_comp/upgradebutton.png";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import { GameEvent } from "../../core/gameEvent";
import {
  NO_LEARNED_STRUCTURE_UPGRADES,
  structureUpgradeOptions,
  structureUpgradePreviewImageName,
} from "../../core/structureUpgradePresentation";
import {
  MobileCard,
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
        index: newIndex
      })
    }
  }

  handleRightClick(event) {
    const upgrades = structureUpgradeOptions(this.props.upgradeData);
    if (this.state.index < (upgrades.length - 1)) {
      const newIndex = this.state.index + 1;
      this.setState({
        index: newIndex
      })
    }
  }

  handleUpgradeClick() {
    const upgrades = structureUpgradeOptions(this.props.upgradeData);
    const upgradeStructure = upgrades[this.state.index];
    if (!upgradeStructure?.name) {
      return;
    }

    Global.network.sendStartUpgrade(this.props.upgradeData.id, upgradeStructure.name);
    Global.gameEmitter.emit(GameEvent.START_UPGRADE_CLICK, {});

    Global.selectedUpgrade = upgradeStructure.name;
  }

  render() {
    console.log(this.state);

    const upgrades = structureUpgradeOptions(this.props.upgradeData);
    const index = Math.min(this.state.index, Math.max(0, upgrades.length - 1));
    const structure = upgrades[index];

    if (!structure) {
      return (
        <MobilePanelScreen
          panelType={'upgrade'}
          title={'Upgrade'}
          hideExitButton={false}>
          <MobileCard>
            <div role="status">{NO_LEARNED_STRUCTURE_UPGRADES}</div>
          </MobileCard>
        </MobilePanelScreen>
      );
    }

    const structureImageName = structureUpgradePreviewImageName(structure);
    const structureImagePath = structureImageName
      ? '/static/art/' + structureImageName
      : undefined;

    let nextStructureName = structure.name || 'Upgrade';

    const reqs = structure.req || [];
    const landscape = isLandscapeMobile();
    const atFirst = index == 0;
    const atLast = index == (upgrades.length - 1);
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
                { label: 'Option', value: `${index + 1} / ${upgrades.length}` },
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
