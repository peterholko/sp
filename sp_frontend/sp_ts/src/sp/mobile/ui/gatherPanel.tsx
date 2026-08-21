
import * as React from "react";
import gatherpanel from "ui_comp/errorframe.png"
import { Global } from "../../core/global";

import orebutton from "ui_comp/resource_categories/ore.png";
import logbutton from "ui_comp/resource_categories/timber.png";
import foragebutton from "ui_comp/resource_categories/forage.png";
import stonebutton from "ui_comp/resource_categories/stone.png";
import waterbutton from "ui_comp/resource_categories/water.png";
import fishbutton from "ui_comp/resource_categories/fish.png";
import gamebutton from "ui_comp/resource_categories/game.png";
import { GameEvent } from "../../core/gameEvent";

interface GatherProps {
  selectedKey: any
}

export default class GatherPanel extends React.Component<GatherProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };

    this.handleOreClick = this.handleOreClick.bind(this)
    this.handleLogClick = this.handleLogClick.bind(this)
    this.handleForageClick = this.handleForageClick.bind(this)
    this.handleStoneClick = this.handleStoneClick.bind(this)
    this.handleFishClick = this.handleFishClick.bind(this)
    this.handleGameClick = this.handleGameClick.bind(this)
  }

  handleOreClick(event: React.MouseEvent) {
    console.log('Ore Click');
    if (this.props.selectedKey.id == Global.heroId) {
      Global.network.sendGather('Ore');
    } else {
      Global.network.sendOrderGather(this.props.selectedKey.id, 'Ore');
    }
    Global.gameEmitter.emit(GameEvent.RESOURCE_GATHER_CLICK, {});
  }

  handleLogClick(event: React.MouseEvent) {
    console.log('Transfer Click');
    if (this.props.selectedKey.id == Global.heroId) {
      Global.network.sendGather('Log');
    } else {
      Global.network.sendOrderGather(this.props.selectedKey.id, 'Log');
    }
    Global.gameEmitter.emit(GameEvent.RESOURCE_GATHER_CLICK, {});
  }

  handleForageClick(event: React.MouseEvent) {
    console.log('Forage Click');
    if (this.props.selectedKey.id == Global.heroId) {
      Global.network.sendGather('Forage');
    } else {
      Global.network.sendOrderGather(this.props.selectedKey.id, 'Forage');
    }
    Global.gameEmitter.emit(GameEvent.RESOURCE_GATHER_CLICK, {});
  }

  handleStoneClick(event: React.MouseEvent) {
    console.log('Stone Click');
    if (this.props.selectedKey.id == Global.heroId) {
      Global.network.sendGather('Stone');
    } else {
      Global.network.sendOrderGather(this.props.selectedKey.id, 'Stone');
    }
    Global.gameEmitter.emit(GameEvent.RESOURCE_GATHER_CLICK, {});
  }

  handleFishClick(event: React.MouseEvent) {
    console.log('Fish Click');
    if (this.props.selectedKey.id == Global.heroId) {
      Global.network.sendGather('Fish');
    } else {
      Global.network.sendOrderGather(this.props.selectedKey.id, 'Fish');
    }
    Global.gameEmitter.emit(GameEvent.RESOURCE_GATHER_CLICK, {});
  }

  handleGameClick(event: React.MouseEvent) {
    console.log('Game Click');
    if (this.props.selectedKey.id == Global.heroId) {
      Global.network.sendGather('Game Animal');
    } else {
      Global.network.sendOrderGather(this.props.selectedKey.id, 'Game Animal');
    }
    Global.gameEmitter.emit(GameEvent.RESOURCE_GATHER_CLICK, {});
  }

  render() {
    var hideOreButton = false;
    var hideLogButton = false;
    var hideForageButton = false;
    var hideStoneButton = false;
    var hideWaterButton = false;
    var hideFishButton = false;
    var hideGameButton = false;

    const gatherStyle = {
      top: '50%',
      left: '50%',
      width: '333px',
      height: '119px',
      transform: 'translate(-50%, -50%)',
      position: 'fixed',
      zIndex: 6
    } as React.CSSProperties

    const panelStyle = {
      position: 'absolute',
      inset: 0,
      width: '333px',
      height: '119px',
      pointerEvents: 'none'
    } as React.CSSProperties

    const buttonRowsStyle = {
      position: 'absolute',
      inset: '9px 10px 10px',
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center',
      gap: '3px'
    } as React.CSSProperties

    const buttonRowStyle = {
      display: 'flex',
      justifyContent: 'center',
      gap: '8px',
      height: '48px'
    } as React.CSSProperties

    const buttonStyle = {
      width: '48px',
      height: '48px',
      cursor: 'pointer'
    } as React.CSSProperties

    const waterStyle = {
      ...buttonStyle,
      cursor: 'help'
    } as React.CSSProperties

    return (
      <div style={gatherStyle} >
        <img src={gatherpanel} style={panelStyle} />
        <div style={buttonRowsStyle}>
          <div style={buttonRowStyle}>
            {!hideOreButton &&
              <img src={orebutton}
                style={buttonStyle}
                title="Ore"
                alt="Ore"
                onClick={this.handleOreClick} />}

            {!hideLogButton &&
              <img src={logbutton}
                style={buttonStyle}
                title="Timber"
                alt="Timber"
                onClick={this.handleLogClick} />}

            {!hideForageButton &&
              <img src={foragebutton}
                style={buttonStyle}
                title="Forage — gather revealed underbrush, resin, or wild food"
                alt="Forage"
                onClick={this.handleForageClick} />}

            {!hideStoneButton &&
              <img src={stonebutton}
                style={buttonStyle}
                title="Stone"
                alt="Stone"
                onClick={this.handleStoneClick} />}
          </div>

          <div style={buttonRowStyle}>
            {!hideWaterButton &&
              <img src={waterbutton}
                style={waterStyle}
                title="Water — use an empty waterskin at a revealed spring"
                alt="Water"
                aria-disabled="true" />}

            {!hideFishButton &&
              <img src={fishbutton}
                style={buttonStyle}
                title="Fish"
                alt="Fish"
                onClick={this.handleFishClick} />}

            {!hideGameButton &&
              <img src={gamebutton}
                style={buttonStyle}
                title="Game"
                alt="Game"
                onClick={this.handleGameClick} />}
          </div>
        </div>
      </div>
    );
  }
}
