import * as React from "react";
import HalfPanel from "./halfPanel";
import { Global } from "../../core/global";
import helmbg from "ui_comp/helm_background.png";
import shoulderbg from "ui_comp/shoulder_background.png";
import chestbg from "ui_comp/chest_background.png";
import pantsbg from "ui_comp/pants_background.png";
import bootsbg from "ui_comp/boots_background.png";
import neckbg from "ui_comp/neck_background.png";
import backbg from "ui_comp/back_background.png";
import bracersbg from "ui_comp/bracers_background.png";
import glovesbg from "ui_comp/gloves_background.png";
import ringbg from "ui_comp/ring_background.png";
import meleebg from "ui_comp/melee_background.png";
import offhandbg from "ui_comp/shield_background.png";
import wideframe from "ui_comp/wide_frame.png";
import okbutton from "ui_comp/okbutton.png";
import selectitemborder from "ui_comp/selectitemborder.png";
import rightarrow from "ui_comp/rightarrow.png";
import leftarrow from "ui_comp/leftarrow.png";
import itemframeborder from "ui_comp/itemframeborder.png";
import itemframe from "ui_comp/itemframe.png";
import InventoryItem from "./inventoryItem";
import BaseInventoryPanel from "./baseInventoryPanel";
import SmallButton from "./smallButton";
import { Network } from "../../core/network";
import cancelbutton from "ui_comp/exitbutton.png";
import { getHalfPanelOffsetMarginTop } from "../../core/uiLayout";

interface EquipPanelProps {
  equipData
}

export default class EquipPanel extends React.Component<EquipPanelProps, any> {
  constructor(props) {
    super(props);

    const selectItemStyle = {
      position: "fixed"
    } as React.CSSProperties

    this.state = {
      selectItemStyle: selectItemStyle,
      showItemTransferPanel: false,
      showSelectedName: false,
      showSelectItemBorder: false,
      equipedSelectedItem: false,
      hideRightSelect: true,
      inventorySelectedItemId: -1,
    };

    this.handleSelect = this.handleSelect.bind(this);
    this.handleEquipSelect = this.handleEquipSelect.bind(this);
    this.handleOkClick = this.handleOkClick.bind(this);
    this.handleCancelClick = this.handleCancelClick.bind(this);
    this.handleItemTransferClick = this.handleItemTransferClick.bind(this);
    this.handleItemInfoClick = this.handleItemInfoClick.bind(this);
  }

  isVillager() {
    const objState = Global.objectStates[this.props.equipData.id];
    return objState && objState.subclass == 'villager';
  }

  handleSelect(eventData) {
    Global.selectedItemOwnerId = eventData.ownerId;
    Global.selectedItemId = eventData.itemId;

    this.setState({
      hideRightSelect: false,
      showSelectedName: true,
      equipedClick: false,
      showSelectItemBorder: false,
      equipedSelectedItem: false,
      inventorySelectedItemId: eventData.itemId
    });
  }

  handleEquipSelect(eventData) {
    console.log('handleSelect ' + eventData);

    var selectItemStyle;

    if (eventData.index == "helm") {
      selectItemStyle = {
        transform: 'translate(-300px, 40px)',
        position: 'fixed'
      } as React.CSSProperties
    } else if (eventData.index == "shoulder") {
      selectItemStyle = {
        transform: 'translate(-300px, 95px)',
        position: 'fixed'
      } as React.CSSProperties
    } else if (eventData.index == "chest") {
      selectItemStyle = {
        transform: 'translate(-300px, 150px)',
        position: 'fixed'
      } as React.CSSProperties
    } else if (eventData.index == "pants") {
      selectItemStyle = {
        transform: 'translate(-300px, 205px)',
        position: 'fixed'
      } as React.CSSProperties
    } else if (eventData.index == "boots") {
      selectItemStyle = {
        transform: 'translate(-300px, 260px)',
        position: 'fixed'
      } as React.CSSProperties
    } else if (eventData.index == "neck") {
      selectItemStyle = {
        transform: 'translate(-78px, 40px)',
        position: 'fixed'
      } as React.CSSProperties
    } else if (eventData.index == "back") {
      selectItemStyle = {
        transform: 'translate(-78px, 95px)',
        position: 'fixed'
      } as React.CSSProperties
    } else if (eventData.index == "bracers") {
      selectItemStyle = {
        transform: 'translate(-78px, 150px)',
        position: 'fixed'
      } as React.CSSProperties
    } else if (eventData.index == "gloves") {
      selectItemStyle = {
        transform: 'translate(-78px, 205px)',
        position: 'fixed'
      } as React.CSSProperties
    } else if (eventData.index == "ring") {
      selectItemStyle = {
        transform: 'translate(-78px, 260px)',
        position: 'fixed'
      } as React.CSSProperties
    } else if (eventData.index == "mainhand") {
      selectItemStyle = {
        transform: 'translate(-215px, 280px)',
        position: 'fixed'
      } as React.CSSProperties
    } else if (eventData.index == "offhand") {
      selectItemStyle = {
        transform: 'translate(-160px, 280px)',
        position: 'fixed'
      } as React.CSSProperties
    }

    Global.selectedItemOwnerId = eventData.ownerId;
    Global.selectedItemId = eventData.itemId;

    this.setState({
      hideRightSelect: true,
      showSelectedName: true,
      showSelectItemBorder: true,
      selectItemStyle: selectItemStyle,
      equipedSelectedItem: true,
      inventorySelectedItemId: -1
    });
  }

  handleOkClick() {
    if (this.isVillager()) {
      this.setState({ showItemTransferPanel: false });
      return;
    }

    var selectedItem;

    for (var i = 0; i < this.props.equipData.items.length; i++) {
      var item = this.props.equipData.items[i];

      if (item.id == Global.selectedItemId) {
        selectedItem = item;
        break;
      }
    }

    if (selectedItem) {
      if ('equipped' in selectedItem) {
        if (selectedItem.equipped) {
          Global.network.sendEquip(this.props.equipData.id, item.id, false);
        } else {
          Global.network.sendEquip(this.props.equipData.id, item.id, true);
        }
      }
    }

    //Reset Global selected item 
    Global.selectedItemOwnerId = -1;
    Global.selectedItemId = -1;

    this.setState({
      hideRightSelect: true,
      showItemTransferPanel: false,
      showSelectItemBorder: false,
      showSelectedName: false,
      equipedSelectedItem: false,
    });
  }

  handleCancelClick() {
    this.setState({
      showItemTransferPanel: false,
    });
  }

  handleItemTransferClick() {
    if (this.isVillager()) {
      return;
    }

    if (Global.selectedItemId != -1) {
      this.setState({ showItemTransferPanel: true });
    }
  }

  handleItemInfoClick() {
    if (Global.selectedItemId != -1) {
      this.setState({ hideSelect: false });

      if (this.state.equipedSelectedItem) {
        Global.infoItemAction = 'inventory'; // Standard inventory
      } else {
        Global.infoItemAction = "equip"; // Equipped item
      }

      Global.network.sendInfoItem(this.props.equipData.id, Global.selectedItemId, "None");
    }
  }

  render() {
    const isVillager = this.isVillager();
    let imageName = Global.objectStates[this.props.equipData.id].image;

    let imagePath = '/static/art/' + imageName + '_single.png';

    var helmEquiped = false;
    var helmItem;

    var shoulderEquiped = false;
    var shoulderItem;

    var chestEquiped = false;
    var chestItem;

    var pantsEquiped = false;
    var pantsItem;

    var bootsEquiped = false;
    var bootsItem;

    var bracersEquiped = false;
    var bracersItem;

    var mainHandEquiped = false;
    var mainHandItem;

    var offHandEquiped = false;
    var offHandItem;

    var selectedItem;
    var equipedItemMatchingSlot;

    // Get selected item for slot matching
    for (var i = 0; i < this.props.equipData.items.length; i++) {
      var item = this.props.equipData.items[i];

      if (Global.selectedItemId == item.id) {
        selectedItem = item;
      }
    }

    // Loop through all items to find equipped items
    for (var i = 0; i < this.props.equipData.items.length; i++) {
      var item = this.props.equipData.items[i];

      // If selected item is not equipped
      if (selectedItem && selectedItem.id != item.id && !selectedItem.equipped) {
        if (item.equipped && item.slot == selectedItem.slot) {
          equipedItemMatchingSlot = item;
        }
      }

      // If selected item is equipped
      if (selectedItem && selectedItem.id != item.id && selectedItem.equipped) {
        if (!item.equipped && item.slot == selectedItem.slot) {
          equipedItemMatchingSlot = item;
        }
      }

      if (item.equipped) {
        if (item.slot == 'Helm') {
          helmEquiped = true;
          helmItem = item;
        }
        else if (item.slot == 'Shoulder') {
          shoulderEquiped = true;
          shoulderItem = item;
        }
        else if (item.slot == 'Chest') {
          chestEquiped = true;
          chestItem = item;
        }
        else if (item.slot == 'Pants') {
          pantsEquiped = true;
          pantsItem = item;
        }
        else if (item.slot == 'Bracers') {
          bracersEquiped = true;
          bracersItem = item;
        }
        else if (item.slot == 'Boots') {
          bootsEquiped = true;
          bootsItem = item;
        } else if (item.slot == 'Main Hand') {
          mainHandEquiped = true;
          mainHandItem = item;
        } else if (item.slot == 'Off Hand') {
          offHandEquiped = true;
          offHandItem = item;
        }
      }
    }

    var selectedItemAttrs = [];

    if (selectedItem && selectedItem.hasOwnProperty('attrs')) {
      for (var attrKey in selectedItem.attrs) {
        var attrValue = selectedItem.attrs[attrKey];

        if (typeof attrValue === "number") {
          if (attrValue < 0) {
            attrValue = '-' + String(attrValue);
          } else {
            attrValue = '+' + String(attrValue);
          }
        } else {
          attrValue = String(attrValue);
        }

        selectedItemAttrs.push(<tr key={attrKey}>
          <td colSpan={2}>{attrValue} {attrKey}</td>
        </tr>)
      }
    }

    var equipedItemMatchingSlotAttrs = [];

    if (equipedItemMatchingSlot && equipedItemMatchingSlot.hasOwnProperty('attrs')) {
      for (var attrKey in equipedItemMatchingSlot.attrs) {
        var attrValue = equipedItemMatchingSlot.attrs[attrKey];

        if (typeof attrValue === "number") {
          if (attrValue < 0) {
            attrValue = '-' + String(attrValue);
          } else {
            attrValue = '+' + String(attrValue);
          }
        } else {
          attrValue = String(attrValue);
        }

        equipedItemMatchingSlotAttrs.push(<tr key={attrKey}>
          <td colSpan={2}>{attrValue} {attrKey}</td>
        </tr>)
      }
    }

    const imageStyle = {
      transform: 'translate(-195px, 120px)',
      position: 'fixed'
    } as React.CSSProperties

    const spanNameStyle = {
      transform: 'translate(-323px, 185px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    var helmBgStyle = {
      transform: 'translate(-300px, 40px)',
      position: 'fixed'
    } as React.CSSProperties

    var shoulderBgStyle = {
      transform: 'translate(-300px, 95px)',
      position: 'fixed'
    } as React.CSSProperties

    var chestBgStyle = {
      transform: 'translate(-300px, 150px)',
      position: 'fixed'
    } as React.CSSProperties

    var pantsBgStyle = {
      transform: 'translate(-300px, 205px)',
      position: 'fixed'
    } as React.CSSProperties

    var bootsBgStyle = {
      transform: 'translate(-300px, 260px)',
      position: 'fixed'
    } as React.CSSProperties

    var neckBgStyle = {
      transform: 'translate(-78px, 40px)',
      position: 'fixed'
    } as React.CSSProperties

    var backBgStyle = {
      transform: 'translate(-78px, 95px)',
      position: 'fixed'
    } as React.CSSProperties

    var bracersBgStyle = {
      transform: 'translate(-78px, 150px)',
      position: 'fixed'
    } as React.CSSProperties

    var glovesBgStyle = {
      transform: 'translate(-78px, 205px)',
      position: 'fixed'
    } as React.CSSProperties

    var ringBgStyle = {
      transform: 'translate(-78px, 260px)',
      position: 'fixed'
    } as React.CSSProperties

    var meleeBgStyle = {
      transform: 'translate(-215px, 280px)',
      position: 'fixed'
    } as React.CSSProperties

    var offHandBgStyle = {
      transform: 'translate(-160px, 280px)',
      position: 'fixed'
    } as React.CSSProperties

    const transferY = getHalfPanelOffsetMarginTop(130);
    const infoY = getHalfPanelOffsetMarginTop(180);

    const zIndex = Global.zIndexManager.getTop() + 3;

    const transferStyle = {
      top: '50%',
      left: '50%',
      marginTop: transferY,
      marginLeft: '-25px',
      position: 'fixed',
      zIndex: zIndex
    } as React.CSSProperties

    const infoStyle = {
      top: '50%',
      left: '50%',
      marginTop: infoY,
      marginLeft: '-25px',
      position: 'fixed',
      zIndex: zIndex
    } as React.CSSProperties

    const wideFrameStyle = {
      top: '50%',
      left: '50%',
      marginTop: infoY,
      marginLeft: '-30px',
      position: 'fixed',
      transform: 'translate(-270px, -155px)',
      zIndex: zIndex
    } as React.CSSProperties

    const leftEquipItemNameStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '-25px',
      position: 'fixed',
      transform: 'translate(-175px, -95px)',
      zIndex: zIndex,
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '200px'
    } as React.CSSProperties

    const rightEquipItemNameStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '-25px',
      position: 'fixed',
      transform: 'translate(25px, -95px)',
      zIndex: zIndex,  
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '200px'
    } as React.CSSProperties

    const okButtonStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '-25px',
      position: 'fixed',
      transform: 'translate(-26px, 135px)',
      zIndex: zIndex,
    } as React.CSSProperties

    const cancelButtonStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '-25px',
      position: 'fixed',
      transform: 'translate(26px, 135px)',
      zIndex: zIndex,
    } as React.CSSProperties

    const rightItemNameStyle = {
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px',
      zIndex: zIndex,
      top: '50%',
      left: '50%',
      marginTop: '125px'
    } as React.CSSProperties

    const leftTransferStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '-25px',
      position: 'fixed',
      transform: 'translate(-100px, -60px)',
      zIndex: zIndex,
    } as React.CSSProperties

    const arrowStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '-25px',
      position: 'fixed',
      transform: 'translate(0px, -60px)',
      zIndex: zIndex,
    } as React.CSSProperties

    const rightTransferStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '-25px',
      position: 'fixed',
      transform: 'translate(100px, -60px)',
      zIndex: zIndex,
    } as React.CSSProperties

    const rightItemAttrsStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '-25px',
      position: 'fixed',
      textAlign: 'left',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '200px',
      transform: 'translate(50px, 0px)',
      zIndex: zIndex,
      userSelect: 'none'
    } as React.CSSProperties

    const leftItemAttrsStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '-25px',
      position: 'fixed',
      textAlign: 'left',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '200px',
      transform: 'translate(-150px, 0px)',
      zIndex: zIndex,
      userSelect: 'none'
    } as React.CSSProperties

    return (
      <div>
        <HalfPanel left={true}
          panelType={'equip'}
          hideExitButton={true}>


          <img src={imagePath} style={imageStyle} />
          <span style={spanNameStyle}>{this.props.equipData.name}</span>

          <img src={helmbg} style={helmBgStyle} />
          <img src={itemframeborder} style={helmBgStyle} />

          {helmEquiped &&

            <InventoryItem
              ownerId={helmItem.ownerId}
              itemId={helmItem.id}
              itemName={helmItem.name}
              image={helmItem.image}
              quantity={helmItem.quantity}
              index={"helm"}
              xPos={24}
              yPos={-319}
              handleSelect={this.handleEquipSelect} />
          }

          <img src={shoulderbg} style={shoulderBgStyle} />
          <img src={itemframeborder} style={shoulderBgStyle} />

          {shoulderEquiped &&
            <InventoryItem
              ownerId={shoulderItem.ownerId}
              itemId={shoulderItem.id}
              itemName={shoulderItem.name}
              image={shoulderItem.image}
              quantity={shoulderItem.quantity}
              index={"shoulder"}
              xPos={24}
              yPos={-269}
              handleSelect={this.handleEquipSelect} />
          }

          <img src={chestbg} style={chestBgStyle} />
          <img src={itemframeborder} style={chestBgStyle} />

          {chestEquiped &&
            <InventoryItem
              ownerId={chestItem.ownerId}
              itemId={chestItem.id}
              itemName={chestItem.name}
              image={chestItem.image}
              quantity={chestItem.quantity}
              index={"chest"}
              xPos={24}
              yPos={-209}
              handleSelect={this.handleEquipSelect} />
          }

          <img src={pantsbg} style={pantsBgStyle} />
          <img src={itemframeborder} style={pantsBgStyle} />

          {pantsEquiped &&
            <InventoryItem
              ownerId={pantsItem.ownerId}
              itemId={pantsItem.id}
              itemName={pantsItem.name}
              image={pantsItem.image}
              quantity={pantsItem.quantity}
              index={"pants"}
              xPos={24}
              yPos={-154}
              handleSelect={this.handleEquipSelect} />
          }

          <img src={bootsbg} style={bootsBgStyle} />
          <img src={itemframeborder} style={bootsBgStyle} />

          {bootsEquiped &&
            <InventoryItem
              ownerId={bootsItem.ownerId}
              itemId={bootsItem.id}
              itemName={bootsItem.name}
              image={bootsItem.image}
              quantity={bootsItem.quantity}
              index={"boots"}
              xPos={24}
              yPos={-154}
              handleSelect={this.handleEquipSelect} />
          }

          <img src={neckbg} style={neckBgStyle} />
          <img src={itemframeborder} style={neckBgStyle} />

          <img src={backbg} style={backBgStyle} />
          <img src={itemframeborder} style={backBgStyle} />

          <img src={bracersbg} style={bracersBgStyle} />
          <img src={itemframeborder} style={bracersBgStyle} />

          <img src={glovesbg} style={glovesBgStyle} />
          <img src={itemframeborder} style={glovesBgStyle} />

          {bracersEquiped &&
            <InventoryItem
              ownerId={bracersItem.ownerId}
              itemId={bracersItem.id}
              itemName={bracersItem.name}
              image={bracersItem.image}
              quantity={bracersItem.quantity}
              index={"bracers"}
              xPos={24}
              yPos={-104}
              handleSelect={this.handleEquipSelect} />
          }

          <img src={ringbg} style={ringBgStyle} />
          <img src={itemframeborder} style={ringBgStyle} />

          <img src={meleebg} style={meleeBgStyle} />
          <img src={itemframeborder} style={meleeBgStyle} />

          {mainHandEquiped &&

            <InventoryItem
              ownerId={mainHandItem.ownerId}
              itemId={mainHandItem.id}
              itemName={mainHandItem.name}
              image={mainHandItem.image}
              quantity={mainHandItem.quantity}
              index={"mainhand"}
              xPos={107}
              yPos={-79}
              handleSelect={this.handleEquipSelect} />
          }

          <img src={offhandbg} style={offHandBgStyle} />
          <img src={itemframeborder} style={offHandBgStyle} />

          {offHandEquiped &&
            <InventoryItem
              ownerId={offHandItem.ownerId}
              itemId={offHandItem.id}
              itemName={offHandItem.name}
              image={offHandItem.image}
              quantity={offHandItem.quantity}
              index={"offhand"}
              xPos={162}
              yPos={-79}
              handleSelect={this.handleEquipSelect} />
          }

          {this.state.showSelectItemBorder &&
            <img src={selectitemborder} style={this.state.selectItemStyle} />
          }

        </HalfPanel>
        <BaseInventoryPanel left={false}
          id={this.props.equipData.id}
          items={this.props.equipData.items}
          panelType={'equip'}
          hideExitButton={false}
          hideSelect={this.state.hideRightSelect}
          showEquippedOnly={true}
          handleSelect={this.handleSelect}
          selectedItemId={this.state.inventorySelectedItemId} />

        {!isVillager &&
          <SmallButton handler={this.handleItemTransferClick}
            imageName="transferbutton"
            style={transferStyle} />}

        <SmallButton handler={this.handleItemInfoClick}
          imageName="infobutton"
          style={infoStyle} />

        {this.state.showSelectedName &&
          <span style={rightItemNameStyle}>{selectedItem.name}</span>}

        {!isVillager && this.state.showItemTransferPanel &&
          <div style={wideFrameStyle}>
            <img src={wideframe} />

            {selectedItem.equipped &&
              <div>

                <span style={leftEquipItemNameStyle}>{selectedItem.name}</span>

                <img src={'/static/art/items/' + selectedItem.image + '.png'} style={leftTransferStyle} />

                <table style={leftItemAttrsStyle}>
                  <tbody>
                    {selectedItemAttrs}
                  </tbody>
                </table>

                <img src={rightarrow} style={arrowStyle} />
                <img src={itemframe} style={rightTransferStyle} />

              </div>
            }

            {!selectedItem.equipped &&
              <div>
                {equipedItemMatchingSlot &&
                  <img src={'/static/art/items/' + equipedItemMatchingSlot.image + '.png'} style={leftTransferStyle} />
                }

                {equipedItemMatchingSlot &&
                  <span style={leftEquipItemNameStyle}>{equipedItemMatchingSlot.name}</span>
                }

                <table style={leftItemAttrsStyle}>
                  <tbody>
                    {equipedItemMatchingSlotAttrs}
                  </tbody>
                </table>

                {!equipedItemMatchingSlot &&
                  <div>
                    {selectedItem.slot == 'Helm' &&
                      <img src={helmbg} style={leftTransferStyle} />
                    }

                    {selectedItem.slot == 'Shoulder' &&
                      <img src={shoulderbg} style={leftTransferStyle} />
                    }

                    {selectedItem.slot == 'Chest' &&
                      <img src={chestbg} style={leftTransferStyle} />
                    }

                    {selectedItem.slot == 'Pants' &&
                      <img src={pantsbg} style={leftTransferStyle} />
                    }

                    {selectedItem.slot == 'Boots' &&
                      <img src={bootsbg} style={leftTransferStyle} />
                    }

                    {selectedItem.slot == 'Main Hand' &&
                      <img src={meleebg} style={leftTransferStyle} />
                    }

                    {selectedItem.slot == 'Off Hand' &&
                      <img src={offhandbg} style={leftTransferStyle} />
                    }
                  </div>
                }

                <img src={leftarrow} style={arrowStyle} />
                <span style={rightEquipItemNameStyle}>{selectedItem.name}</span>
                <img src={'/static/art/items/' + selectedItem.image + '.png'} style={rightTransferStyle} />
                <table style={rightItemAttrsStyle}>
                  <tbody>
                    {selectedItemAttrs}
                  </tbody>
                </table>
              </div>
            }

            <img src={okbutton} style={okButtonStyle} onClick={this.handleOkClick} />
            <img src={cancelbutton} style={cancelButtonStyle} onClick={this.handleCancelClick} />
          </div>
        }


      </div>
    );
  }
}
