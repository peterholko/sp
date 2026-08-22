import * as React from "react";
import inventorybutton from "ui_comp/inventorybutton.png";
import transferbutton from "ui_comp/transferbutton.png";
import explorebutton from "ui_comp/explorebutton.png";
import gatherbutton from "ui_comp/gatherbutton.png";
import followbutton from "ui_comp/followbutton.png";
import infobutton from "ui_comp/infobutton.png";
import merchantbutton from "ui_comp/merchantbutton.png";
import repairbutton from "ui_comp/buildbutton.png";
import { Global } from "../../core/global";
import { Util } from "../../core/util";
import { VILLAGER, DEAD, OBJ, TILE, FOUNDED } from "../../core/config";
import { GameEvent } from "../../core/gameEvent";
import { canOfferItemTransfer } from "../../core/shipwreckTransferPolicy";
import styles from "./../ui.module.css";

interface TAProps {
  selectedBoxPos: integer,
  selectedKey: any,
}

interface TargetAction {
  label: string,
  image: string,
  handler: (event: React.MouseEvent) => void,
}

export default class TargetActionPanel extends React.Component<TAProps, any> {
  closeTray() {
    Global.gameEmitter.emit(GameEvent.TAP_CLICK, {});
  }

  handleInventoryClick = (event: React.MouseEvent) => {
    Global.network.sendInfoInventory(this.props.selectedKey.id);
    this.closeTray();
  };

  handleTransferClick = (event: React.MouseEvent) => {
    Global.infoItemTransferAction = 'transfer';
    Global.network.sendInfoItemTransfer(Global.heroId, this.props.selectedKey.id);
    this.closeTray();
  };

  handleExploreClick = (event: React.MouseEvent) => {
    if (this.props.selectedKey.type == OBJ) {
      if (Util.isPlayerObj(this.props.selectedKey.id)
          && Util.isSubclass(this.props.selectedKey.id, VILLAGER)) {
        Global.network.sendOrderProspect(this.props.selectedKey.id);
      } else if (Util.isSubclass(this.props.selectedKey.id, 'poi')
          || Util.isSubclass(this.props.selectedKey.id, 'monolith')) {
        Global.network.sendInvestigate(this.props.selectedKey.id);
      }
    }
    this.closeTray();
  };

  handleGatherClick = (event: React.MouseEvent) => {
    Global.gameEmitter.emit(GameEvent.VILLAGER_GATHER_CLICK, this.props.selectedKey);
    this.closeTray();
  };

  handleFollowClick = (event: React.MouseEvent) => {
    Global.network.sendFollow(this.props.selectedKey.id);
    this.closeTray();
  };

  handleRepairClick = (event: React.MouseEvent) => {
    Global.network.sendOrderRepair(this.props.selectedKey.id);
    this.closeTray();
  };

  handleInfoClick = (event: React.MouseEvent) => {
    if (this.props.selectedKey.type == OBJ) {
      Global.network.sendInfoObj(this.props.selectedKey.id);
    } else if (this.props.selectedKey.type == TILE) {
      Global.network.sendInfoTile(this.props.selectedKey.x, this.props.selectedKey.y);
    }
    this.closeTray();
  };

  handleMerchantClick = (event: React.MouseEvent) => {
    Global.infoItemTransferAction = 'merchant';
    Global.network.sendInfoMerchant(Global.heroId, this.props.selectedKey.id);
    Global.gameEmitter.emit(GameEvent.MERCHANT_CLICK, this.props.selectedKey.id);
    this.closeTray();
  };

  render() {
    const key = this.props.selectedKey;
    const selectedObjectState = key.type == OBJ ? Global.objectStates[key.id] : undefined;
    const actions: TargetAction[] = [];
    const add = (label: string, image: string, handler: (event: React.MouseEvent) => void) => {
      actions.push({ label, image, handler });
    };

    if (key.type == TILE) {
      add('Information', infobutton, this.handleInfoClick);
    } else if (key.type == OBJ) {
      if (Util.isPlayerObj(key.id)) {
        if (Util.isSubclass(key.id, VILLAGER)) {
          add('Information', infobutton, this.handleInfoClick);
          add('Inventory', inventorybutton, this.handleInventoryClick);
          add('Transfer', transferbutton, this.handleTransferClick);
          add('Prospect', explorebutton, this.handleExploreClick);
          add('Gather', gatherbutton, this.handleGatherClick);
          add('Follow', followbutton, this.handleFollowClick);
          add('Repair', repairbutton, this.handleRepairClick);
        } else if (Util.isState(key.id, FOUNDED)) {
          add('Information', infobutton, this.handleInfoClick);
          add('Transfer', transferbutton, this.handleTransferClick);
        } else {
          add('Information', infobutton, this.handleInfoClick);
          add('Inventory', inventorybutton, this.handleInventoryClick);
          add('Transfer', transferbutton, this.handleTransferClick);
        }
      } else if (Util.isState(key.id, DEAD)) {
        add('Information', infobutton, this.handleInfoClick);
        add('Loot', transferbutton, this.handleTransferClick);
      } else if (Util.isSubclass(key.id, 'monolith')) {
        add('Information', infobutton, this.handleInfoClick);
        add('Transfer', transferbutton, this.handleTransferClick);
        add('Investigate', explorebutton, this.handleExploreClick);
      } else if (Util.isSubclass(key.id, 'poi')) {
        add('Information', infobutton, this.handleInfoClick);
        if (canOfferItemTransfer(selectedObjectState, Global.shipwreckSearched)) {
          add('Transfer', transferbutton, this.handleTransferClick);
        }
        add('Investigate', explorebutton, this.handleExploreClick);
      } else if (Util.isSubclass(key.id, 'merchant')) {
        add('Information', infobutton, this.handleInfoClick);
        add('Trade', merchantbutton, this.handleMerchantClick);
      } else if (Util.hasGroup(key.id, 'Tax Collector')) {
        add('Information', infobutton, this.handleInfoClick);
        add('Transfer', transferbutton, this.handleTransferClick);
      } else {
        add('Information', infobutton, this.handleInfoClick);
      }
    }

    if (actions.length === 0) return null;

    return (
      <nav className={styles.targetActionTray} aria-label="Selected target actions">
        {actions.map((action) => (
          <button
            type="button"
            key={action.label}
            className={styles.targetActionButton}
            onClick={action.handler}
            title={action.label}
            aria-label={action.label}
          >
            <img src={action.image} alt="" aria-hidden="true" />
            <span>{action.label}</span>
          </button>
        ))}
      </nav>
    );
  }
}
