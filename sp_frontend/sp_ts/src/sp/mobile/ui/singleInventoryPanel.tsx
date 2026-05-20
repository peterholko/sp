

import * as React from "react";
import BaseInventoryPanel from "./baseInventoryPanel";
import equipbutton from "ui_comp/equipbutton.png";
import usebutton from "ui_comp/usebutton.png";
import { Network } from "../../core/network";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";


export default class SingleInventoryPanel extends React.Component<any, any> {
  constructor(props) {
    super(props);

    this.state = {
      hideSelect: true,
      selectedItemId: -1
    };

    this.handleSelect = this.handleSelect.bind(this);
    this.handleEquipClick = this.handleEquipClick.bind(this);
    //this.handleUseClick = this.handleUseClick.bind(this);

    Global.gameEmitter.on(GameEvent.ITEM_USE_CLICK, this.handleUseClick, this);
  }

  handleSelect(eventData) {
    this.setState({
      hideSelect: false,
      selectedItemId: eventData.itemId
    });

    Global.infoItemAction = 'inventory';
    Global.network.sendInfoItem(this.props.inventoryData.id,eventData.itemId, "None");
  }

  handleEquipClick() {
    Global.network.sendInfoEquip(this.props.inventoryData.id);
  }

  handleUseClick() {
    this.setState({
      hideSelect: true,
      selectedItemId: -1
    });
  }

  render() {

    var objId = this.props.inventoryData.id;
    var showEquipButton = false;
    //var showUseButton = false;

    if (Global.objectStates[objId].subclass == 'hero' || Global.objectStates[objId].subclass == 'villager') {
 
      showEquipButton = true;

      /*if (this.state.selectedItemId != -1) {
        var itemData = null;

        // Get the item by id
        for (var i = 0; i < this.props.inventoryData.items.length; i++) {
          if (this.props.inventoryData.items[i].id == this.state.selectedItemId) {
            itemData = this.props.inventoryData.items[i];
            break;
          }
        }

        if (itemData != null) {
          showEquipButton = (itemData.class == "Weapon") ||
            (itemData.class == "Armor") ||
            (itemData.class == "Torch");

          showUseButton = (itemData.class == "Potion") ||
            (itemData.class == "Deed") ||
            (itemData.subclass == "Bucket");
        }
      }*/
    }

    const equipStyle = {
      width: '100%',
      minHeight: '44px',
      border: '1px solid rgba(201, 170, 113, 0.55)',
      borderRadius: '4px',
      background: '#25282b',
      color: '#f2e7cf',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
    } as React.CSSProperties

    /*const useStyle = {
      top: '50%',
      left: '50%',
      marginTop: equipY,
      marginLeft: '-25px',
      position: 'fixed',
      transform: 'translate(-134px, 135px)',
      zIndex: 6
    } as React.CSSProperties*/

    return (
      <div>
        <BaseInventoryPanel left={this.props.left}
          id={this.props.inventoryData.id}
          items={this.props.inventoryData.items}
          capacity={this.props.inventoryData.cap}
          totalWeight={this.props.inventoryData.tw}
          panelType={'inventory'}
          hideExitButton={false}
          hideSelect={this.state.hideSelect}
          showEquipped={true}
          handleSelect={this.handleSelect}
          selectedItemId={this.state.selectedItemId}
          footer={showEquipButton &&
            <button type="button" style={equipStyle} onClick={this.handleEquipClick}>
              <img src={equipbutton} style={{ width: '40px', height: '40px' }} />
            </button>} />

      </div>
    );
  }
}
