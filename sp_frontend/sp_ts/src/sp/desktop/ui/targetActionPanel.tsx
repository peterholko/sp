
import * as React from "react";
import targetactionpanel from "ui_comp/buttonsframe.png"
import { Network } from "../../core/network";
import { Global } from "../../core/global";

import inventorybutton from "ui_comp/inventorybutton.png";
import transferbutton from "ui_comp/transferbutton.png";
import explorebutton from "ui_comp/explorebutton.png";
import gatherbutton from "ui_comp/gatherbutton.png";
import followbutton from "ui_comp/followbutton.png";
import infobutton from "ui_comp/infobutton.png";
import merchantbutton from "ui_comp/merchantbutton.png";
import repairbutton from "ui_comp/repairbutton.png";
import usebutton from "ui_comp/usebutton.png";

import { Util } from "../../core/util";
import { VILLAGER, DEAD, OBJ, TILE, FOUNDED, BUTTON_WIDTH } from "../../core/config";
import { GameEvent } from "../../core/gameEvent";
import { isSafeLogoutProtectedObject } from "../../core/protectedSettlements";
import { canLightCampfireTarget } from "./campfireActionPolicy";
import SmallButton from "./smallButton";

interface TAProps {
  selectedBoxPos: integer,
  selectedKey: any
}

export default class TargetActionPanel extends React.Component<TAProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };

    this.handleInventoryClick = this.handleInventoryClick.bind(this)
    this.handleTransferClick = this.handleTransferClick.bind(this)
    this.handleExploreClick = this.handleExploreClick.bind(this)
    this.handleGatherClick = this.handleGatherClick.bind(this)
    this.handleFollowClick = this.handleFollowClick.bind(this)
    this.handleInfoClick = this.handleInfoClick.bind(this)
    this.handleInfoTileResourceClick = this.handleInfoTileResourceClick.bind(this)
    this.handleMerchantClick = this.handleMerchantClick.bind(this)
    this.handleRepairClick = this.handleRepairClick.bind(this)
    this.handleLightCampfireClick = this.handleLightCampfireClick.bind(this)
  }

  handleInventoryClick(event: React.MouseEvent) {
    console.log('Inventory Click');
    Global.network.sendInfoInventory(this.props.selectedKey.id);
    Global.gameEmitter.emit(GameEvent.TAP_CLICK, {});
  }

  handleTransferClick(event: React.MouseEvent) {
    console.log('Transfer Click');
    Global.infoItemTransferAction = 'transfer';
    Global.network.sendInfoItemTransfer(Global.heroId, this.props.selectedKey.id);
    Global.gameEmitter.emit(GameEvent.TAP_CLICK, {});
  }

  handleStatsClick(event: React.MouseEvent) {
    Global.gameEmitter.emit(GameEvent.TAP_CLICK, {});
  }

  handleExploreClick(event: React.MouseEvent) {
    if (this.props.selectedKey.type == OBJ) {
      if (Util.isPlayerObj(this.props.selectedKey.id) &&
          Util.isSubclass(this.props.selectedKey.id, VILLAGER)) {
        Global.network.sendOrderProspect(this.props.selectedKey.id);
      } else if (Util.isSubclass(this.props.selectedKey.id, "poi") ||
                 Util.isSubclass(this.props.selectedKey.id, "monolith")) {
        Global.network.sendInvestigate(this.props.selectedKey.id);
      }
    }

    Global.gameEmitter.emit(GameEvent.TAP_CLICK, {});
  }

  handleGatherClick(event: React.MouseEvent) {
    Global.gameEmitter.emit(GameEvent.VILLAGER_GATHER_CLICK, this.props.selectedKey);
    Global.gameEmitter.emit(GameEvent.TAP_CLICK, {});
  }

  handleFollowClick(event: React.MouseEvent) {
    Global.network.sendFollow(this.props.selectedKey.id);
    Global.gameEmitter.emit(GameEvent.TAP_CLICK, {});
  }

  handleRepairClick(event: React.MouseEvent) {
    Global.network.sendOrderRepair(this.props.selectedKey.id);
    Global.gameEmitter.emit(GameEvent.TAP_CLICK, {});
  }

  handleLightCampfireClick() {
    Global.network.sendActivate(this.props.selectedKey.id);
    Global.gameEmitter.emit(GameEvent.TAP_CLICK, {});
  }

  handleInfoClick(event: React.MouseEvent) {
    if (this.props.selectedKey.type == OBJ) {
      console.log('handleInfoClick');
      Global.network.sendInfoObj(this.props.selectedKey.id);
    } else if (this.props.selectedKey.type == TILE) {
      Global.network.sendInfoTile(this.props.selectedKey.x,
        this.props.selectedKey.y);
    }

    Global.gameEmitter.emit(GameEvent.TAP_CLICK, {});
  }

  handleInfoTileResourceClick(event: React.MouseEvent) {
    Global.network.sendInfoTileResources(this.props.selectedKey.x,
      this.props.selectedKey.y);

    Global.gameEmitter.emit(GameEvent.TAP_CLICK, {});
  }

  handleMerchantClick() {
    Global.infoItemTransferAction = 'merchant';
    //Network.sendInfoItemTransfer(Global.heroId, this.props.selectedKey.id);
    Global.network.sendInfoMerchant(Global.heroId, this.props.selectedKey.id,);
    Global.gameEmitter.emit(GameEvent.MERCHANT_CLICK, this.props.selectedKey.id);
  }

  render() {
    const selectedObjectState = this.props.selectedKey.type == OBJ
      ? Global.objectStates[this.props.selectedKey.id]
      : undefined;
    const safeLogoutProtected = isSafeLogoutProtectedObject(
      selectedObjectState,
      Global.protectedSettlements,
    );

    var hideInfoButton = true;
    var hideInfoTileResourceButton = true;
    var hideInventoryButton = true;
    var hideTranferButton = true;
    var hideExploreButton = true;
    var hideGatherButton = true;
    var hideFollowButton = true;
    var hideMerchantButton = true;
    var hideRepairButton = true;
    var hideLightCampfireButton = !canLightCampfireTarget(
      selectedObjectState,
      safeLogoutProtected,
    );
    var exploreActionLabel = "Prospect";

    var buttonOrder = {
      info: 0,
      inventory: 1,
      transfer: 2,
      explore: 3,
      gather: 4,
      follow: 5,
      repair: 6
    };

    var numButtons = 1;

    if (this.props.selectedKey.type == OBJ) {
      if (Util.isPlayerObj(this.props.selectedKey.id)) {
        if (Util.isSubclass(this.props.selectedKey.id, VILLAGER)) {
          hideInfoButton = false;
          hideInventoryButton = false;
          hideTranferButton = false;
          hideExploreButton = false;
          hideGatherButton = false;
          hideFollowButton = false;
          hideRepairButton = false;
          exploreActionLabel = "Prospect";
          numButtons = 3; //Shortcut because the explore, gather, follow are stacked below

        } else if (Util.isState(this.props.selectedKey.id, FOUNDED)) {
          hideInfoButton = false;
          hideTranferButton = false;
          numButtons = 2;
        } else {
          hideInfoButton = false;
          hideInventoryButton = false;
          hideTranferButton = false;
          numButtons = 3;
        }
      } else {
        if (Util.isState(this.props.selectedKey.id, DEAD)) {
          hideInfoButton = false;
          hideTranferButton = false;
          numButtons = 2;
        }
        else if (Util.isSubclass(this.props.selectedKey.id, "monolith")) {
          hideTranferButton = false;
          hideInfoButton = false;
          hideExploreButton = false;
          exploreActionLabel = "Investigate";
          numButtons = 2;
        } else if (Util.isSubclass(this.props.selectedKey.id, "poi")) {
          hideTranferButton = false;
          hideInfoButton = false;
          hideExploreButton = false;
          exploreActionLabel = "Investigate";
          numButtons = 2;
        }
        else if (Util.isSubclass(this.props.selectedKey.id, "merchant")) {
          hideMerchantButton = false;
          hideInfoButton = false;
        }
        else if (Util.hasGroup(this.props.selectedKey.id, "Tax Collector")) {
          hideTranferButton = false;
          hideInfoButton = false;
        } else {
          hideInfoButton = false;
        }
      }
    } else if (this.props.selectedKey.type == TILE) {
      hideInfoButton = false;
      //hideInfoTileResourceButton = false;
      //numButtons = 2;
    }

    if (safeLogoutProtected) {
      // Read-only inspection remains available; every mutation is disabled at
      // the desktop affordance as well as at the authoritative server guard.
      hideTranferButton = true;
      hideExploreButton = true;
      hideGatherButton = true;
      hideFollowButton = true;
      hideMerchantButton = true;
      hideRepairButton = true;
      numButtons = Number(!hideInfoButton) + Number(!hideInventoryButton);
    }

    var panelWidth = numButtons * BUTTON_WIDTH;
    var panelPos = ((this.props.selectedBoxPos + 1) * 74) + 35 - 37 + panelWidth / 2;

    const targetActionPanelStyle = {
      top: '82px',
      right: panelPos + 'px',
      position: 'fixed',
      zIndex: 6
    } as React.CSSProperties

    const tapStyle = {
      position: 'fixed',
      width: '67px',
      height: '67px'
    } as React.CSSProperties

    const infoStyle = {
      transform: 'translate(0px, 0px)',
      position: 'fixed'
    } as React.CSSProperties

    const infoTileResourcesStyle = {
      transform: 'translate(50px, 0px)',
      position: 'fixed'
    } as React.CSSProperties    

    const inventoryStyle = {
      transform: safeLogoutProtected ? 'translate(50px, 0px)' : 'translate(100px, 0px)',
      position: 'fixed'
    } as React.CSSProperties

    const transferStyle = {
      transform: 'translate(50px, 0px)',
      position: 'fixed'
    } as React.CSSProperties

    const merchantStyle = {
      transform: 'translate(50px, 0px)',
      position: 'fixed'
    } as React.CSSProperties

    const exploreStyle = {
      transform: 'translate(0px, 50px)',
      position: 'fixed'
    } as React.CSSProperties

    const gatherStyle = {
      transform: 'translate(50px, 50px)',
      position: 'fixed'
    } as React.CSSProperties

    const followStyle = {
      transform: 'translate(100px, 50px)',
      position: 'fixed'
    } as React.CSSProperties

    const repairStyle = {
      transform: 'translate(0px, 100px)',
      position: 'fixed'
    } as React.CSSProperties    

    const lightCampfireStyle = {
      transform: 'translate(0px, 50px)',
      position: 'fixed'
    } as React.CSSProperties

    const protectedLabelStyle = {
      position: 'absolute',
      top: '50px',
      left: '-25px',
      padding: '5px 8px',
      border: '1px solid #e4c66f',
      borderRadius: '3px',
      background: 'rgba(16, 40, 58, 0.94)',
      color: '#e5fbff',
      fontFamily: 'Verdana',
      fontSize: '10px',
      lineHeight: '13px',
      whiteSpace: 'nowrap',
      boxShadow: '0 0 10px rgba(114, 214, 232, 0.32)',
      pointerEvents: 'none'
    } as React.CSSProperties

    return (
      <div style={targetActionPanelStyle} >

        {safeLogoutProtected &&
          <div
            style={protectedLabelStyle}
            title="This settlement is frozen and cannot be changed until its owner returns."
          >
            ◇ Safe Logout protected
          </div>}

        {!hideInfoButton &&
          <SmallButton handler={this.handleInfoClick}
            imageName="infobutton"
            style={infoStyle} />}

        {!hideInfoTileResourceButton &&
          <SmallButton handler={this.handleInfoTileResourceClick}
            imageName="resourcesbutton"
            style={infoTileResourcesStyle} />}             

        {!hideInventoryButton &&
          <SmallButton handler={this.handleInventoryClick}
            imageName="inventorybutton"
            style={inventoryStyle} />}

        {!hideTranferButton &&
          <SmallButton handler={this.handleTransferClick}
            imageName="transferbutton"
            style={transferStyle} />}

        {!hideExploreButton &&
          <img src={explorebutton}
            style={exploreStyle}
            title={exploreActionLabel}
            alt={exploreActionLabel}
            aria-label={exploreActionLabel}
            onClick={this.handleExploreClick} />}

        {!hideGatherButton &&
          <img src={gatherbutton}
            style={gatherStyle}
            onClick={this.handleGatherClick} />}

        {!hideFollowButton &&
          <SmallButton handler={this.handleFollowClick}
            imageName="followbutton"
            style={followStyle} />}

        {!hideMerchantButton &&
          <SmallButton handler={this.handleMerchantClick}
            imageName="merchantbutton"
            style={merchantStyle} />}

        {!hideRepairButton &&
          <SmallButton handler={this.handleRepairClick}
            imageName="repairbutton"
            style={repairStyle} />}             

        {!hideLightCampfireButton &&
          <div title="Light Campfire" aria-label="Light Campfire">
            <img src={usebutton}
              alt="Light Campfire"
              onClick={this.handleLightCampfireClick}
              style={lightCampfireStyle} />
          </div>}

      </div>
    );
  }
}
