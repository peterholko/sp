import * as React from "react";
import HalfPanel from "./halfPanel";
import { Global } from "../../core/global";
import upgradebutton from "ui_comp/upgradebutton.png";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import { Network } from "../../core/network";
import { NetworkEvent } from "../../core/networkEvent";
import ResourceItem from "./resourceItem";
import { GameEvent } from "../../core/gameEvent";
import {
  NO_LEARNED_STRUCTURE_UPGRADES,
  structureUpgradeOptions,
  structureUpgradePreviewImageName,
} from "../../core/structureUpgradePresentation";

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
        <HalfPanel left={false}
          panelType={'upgrade'}
          hideExitButton={false}>
          <div style={{ color: 'white', fontFamily: 'Verdana', padding: '24px' }}>
            {NO_LEARNED_STRUCTURE_UPGRADES}
          </div>
        </HalfPanel>
      );
    }

    const structureImageName = structureUpgradePreviewImageName(structure);
    const structureImagePath = structureImageName
      ? '/static/art/' + structureImageName
      : null;

    let nextStructureName = structure.name || 'Upgrade';

    const reqs = [];

    const requirements = structure.req || [];
    for (var i = 0; i < requirements.length; i++) {
      var req = requirements[i];
      var resourceImage = req.type.toLowerCase().replace(/\s/g, '');

      reqs.push(
        <ResourceItem key={i}
          resourceName={req.type}
          resourceImage={resourceImage}
          quantity={req.quantity}
          index={i}
          showQuantity={true} />
      )
    }

    const structureStyle = {
      transform: 'translate(-195px, 25px)',
      position: 'fixed'
    } as React.CSSProperties

    const nextStructureNameStyle = {
      transform: 'translate(-323px, 100px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    const upgradeStyle = {
      transform: 'translate(-185px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const tableStyle = {
      transform: 'translate(20px, -230px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const divReqsStyle = {
      transform: 'translate(-100px, 15px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const leftStyle = {
      transform: 'translate(-305px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const rightStyle = {
      transform: 'translate(-65px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    return (
      <HalfPanel left={false}
        panelType={'upgrade'}
        hideExitButton={false}>

        {structureImagePath && <img src={structureImagePath} style={structureStyle} />}
        <span style={nextStructureNameStyle}>{nextStructureName}</span>

        <table style={tableStyle}>
          <tbody>
            <tr>
              <td>Requirements:</td>
              <td>
                <div style={divReqsStyle}>
                  {reqs}
                </div>
              </td>
            </tr>
          </tbody>
        </table>

        <img src={leftbutton} style={leftStyle} onClick={this.handleLeftClick} />
        <img src={rightbutton} style={rightStyle} onClick={this.handleRightClick} />
        <img src={upgradebutton} style={upgradeStyle} onClick={this.handleUpgradeClick} />
      </HalfPanel>
    );
  }
}
