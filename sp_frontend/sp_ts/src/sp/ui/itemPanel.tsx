
import * as React from "react";
import HalfPanel from "./halfPanel";
import dividebutton from "ui_comp/dividebutton.png";
import buybutton from "ui_comp/buybutton.png";
import sellbutton from "ui_comp/sellbutton.png";
import equipbutton from "ui_comp/equipbutton.png";
import refineorebutton from "ui_comp/refinebutton.png";
import refinewoodbutton from "ui_comp/refinewoodbutton.png";
import refinestonebutton from "ui_comp/refinestonebutton.png";
import refinegameanimalbutton from "ui_comp/refinegameanimalbutton.png";
import deletebutton from "ui_comp/deletebutton.png";
import usebutton from "ui_comp/usebutton.png";

import { GameEvent } from "../gameEvent";
import { Global } from "../global";
import { TRIGGER_PLAYER_SELLING_ITEM, TRIGGER_INVENTORY, TRIGGER_PLAYER_BUYING_ITEM, FALSE, TRIGGER_EQUIP, TRIGGER_REFINING_ITEM, TRIGGER_STRUCTURE_REFINING_ITEM } from "../config";
import { Network } from "../network";

interface ItemPanelProps {
  triggerAction,
  itemData,
}

export default class ItemPanel extends React.Component<ItemPanelProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };

    this.handleDivideClick = this.handleDivideClick.bind(this);
    this.handleBuyClick = this.handleBuyClick.bind(this);
    this.handleSellClick = this.handleSellClick.bind(this);
    this.handleEquipClick = this.handleEquipClick.bind(this);
    this.handleRefineClick = this.handleRefineClick.bind(this);
    this.handleUseClick = this.handleUseClick.bind(this);
    this.handleDeleteClick = this.handleDeleteClick.bind(this);
    this.handleSendDelete = this.handleSendDelete.bind(this);

    Global.gameEmitter.on(GameEvent.CONFIRM_OK_CLICK, this.handleSendDelete, this);
  }

  componentWillUnmount() {
    Global.gameEmitter.removeListener(GameEvent.CONFIRM_OK_CLICK, this.handleSendDelete);
  }

  handleDivideClick() {
    Global.gameEmitter.emit(GameEvent.ITEM_DIVIDE_CLICK, this.props.itemData);
  }

  handleBuyClick() {
    const eventData = {
      'itemData': this.props.itemData,
      'action': 'buy'
    };

    Global.gameEmitter.emit(GameEvent.MERCHANT_BUYSELL_CLICK, eventData);
  }

  handleSellClick() {
    //Network.sendSellItem(this.props.itemData.id, this.props.itemData.quantity);
    const eventData = {
      'itemData': this.props.itemData,
      'action': 'sell'
    };

    Global.gameEmitter.emit(GameEvent.MERCHANT_BUYSELL_CLICK, eventData);
  }

  handleEquipClick() {
    if (this.props.itemData.equipped == false) {
      Global.network.sendEquip(this.props.itemData.owner, this.props.itemData.id, true);
    } else {
      Global.network.sendEquip(this.props.itemData.owner, this.props.itemData.id, false);
    }
  }

  handleUseClick() {
    const eventData = {
      'itemData': this.props.itemData,
      'action': 'use'
    };

    Global.network.sendUse(this.props.itemData.owner, this.props.itemData.id);
    Global.gameEmitter.emit(GameEvent.ITEM_USE_CLICK, eventData);
  }

  handleRefineClick() {
    Global.isStructureRefining = false;
    Global.network.sendRefine(this.props.itemData.id);
  }

  handleDeleteClick() {
    console.log('Delete Item');
    const event = {
      msg: 'Remove the item?',
      type: 'delete_item'
    };
    Global.gameEmitter.emit(GameEvent.CONFIRMATION, event);


    //Global.network.sendDeleteItem(this.props.itemData.id);
  }

  handleSendDelete() {
    console.log('Send Delete Item');
    const eventData = {
      'itemData': this.props.itemData,
      'action': 'delete'
    };

    Global.network.sendDeleteItem(this.props.itemData.owner, this.props.itemData.id);
    Global.gameEmitter.emit(GameEvent.ITEM_DELETE_CLICK, {});
  }

  render() {
    const itemName = this.props.itemData.name;
    const imageName = this.props.itemData.image + '.png'
    const effects = [];
    const produces = [];

    const isLeftPanel =
      (this.props.triggerAction == TRIGGER_PLAYER_BUYING_ITEM) ||
      (this.props.triggerAction == TRIGGER_EQUIP) ||
      (this.props.triggerAction == TRIGGER_STRUCTURE_REFINING_ITEM);

    const isMiddlePanel =
      (this.props.triggerAction == TRIGGER_REFINING_ITEM);

    const showDivideButton = (this.props.itemData.quantity > 1) &&
      (this.props.triggerAction == TRIGGER_INVENTORY) &&
      (this.props.triggerAction != TRIGGER_STRUCTURE_REFINING_ITEM) &&
      (this.props.triggerAction != TRIGGER_REFINING_ITEM);

    const showBuyButton =
      (this.props.triggerAction == TRIGGER_PLAYER_BUYING_ITEM) &&
      (this.props.triggerAction != TRIGGER_STRUCTURE_REFINING_ITEM) &&
      (this.props.triggerAction != TRIGGER_REFINING_ITEM);

    const showSellButton =
      (this.props.triggerAction == TRIGGER_PLAYER_SELLING_ITEM) &&
      (this.props.triggerAction != TRIGGER_STRUCTURE_REFINING_ITEM) &&
      (this.props.triggerAction != TRIGGER_REFINING_ITEM);

    const showUseButton = ((this.props.itemData.class == "Potion") ||
      (this.props.itemData.class == "Deed") ||
      (this.props.itemData.class == "Food") ||
      (this.props.itemData.class == "Drink") ||
      (this.props.itemData.subclass == "Bucket") ||
      (this.props.itemData.subclass == "Fishing Rod") ||
      (this.props.itemData.subclass == "Waterskin") ||
      (this.props.itemData.subclass == "Bedroll")) &&
      (this.props.triggerAction == TRIGGER_INVENTORY) &&
      (this.props.triggerAction != TRIGGER_STRUCTURE_REFINING_ITEM) &&
      (this.props.triggerAction != TRIGGER_REFINING_ITEM);

    const showDeleteButton = (this.props.triggerAction != TRIGGER_STRUCTURE_REFINING_ITEM) && (this.props.triggerAction != TRIGGER_REFINING_ITEM);

    const topLevel = this.props.triggerAction == TRIGGER_REFINING_ITEM;

    const hasEquipable = (this.props.itemData.attrs && this.props.itemData.attrs.hasOwnProperty('Equipable'));
    const hasPrice = this.props.itemData.hasOwnProperty('price');

    var hasDurability = false;
    var hasProduces = false;
    var hasEffects = false;

    var attrs = [];

    if (this.props.itemData.hasOwnProperty('durability')) {
      hasDurability = true;
    }

    if (this.props.itemData.hasOwnProperty('attrs')) {
      for (var attrKey in this.props.itemData.attrs) {
        var attrValue = this.props.itemData.attrs[attrKey];

        if (typeof attrValue === "number") {
          if (attrValue < 0) {
            attrValue = '-' + String(attrValue);
          } else {
            attrValue = '+' + String(attrValue);
          }
        } else {
          attrValue = String(attrValue);
        }

        attrs.push(<tr key={attrKey}>
          <td colSpan={2}>{attrValue} {attrKey}</td>
        </tr>)
      }
    }

    if (this.props.itemData.hasOwnProperty('effects')) {
      hasEffects = true;

      for (var i = 0; i < this.props.itemData.effects.length; i++) {
        var effect = this.props.itemData.effects[i];
        var type = ''
        var value = ''

        if (effect.type.indexOf('%') != -1) {
          type = effect.type.replace('%', '');

          if (effect.value > 0) {
            value = type + '+' + (effect.value * 100) + '%';
          } else {
            value = type + (effect.value * 100) + '%';
          }

        } else {
          value = type + effect.value;
        }

        effects.push(<tr key={i}>
          <td>{value}</td>
        </tr>)
      }
    }

    if (this.props.itemData.hasOwnProperty('produces')) {
      hasProduces = true;


      for (var i = 0; i < this.props.itemData.produces.length; i++) {
        produces.push(<tr key={i}>
          <td>{this.props.itemData.produces[i]}</td>
        </tr>)
      }
    }

    let refineItemIcon = null;

    if (this.props.itemData.class == 'Ore') {
      refineItemIcon = refineorebutton;
    } else if (this.props.itemData.class == 'Log') {
      refineItemIcon = refinewoodbutton;
    } else if (this.props.itemData.class == 'Stone') {
      refineItemIcon = refinestonebutton;
    } else if (this.props.itemData.class == 'Game Animal') {
      refineItemIcon = refinegameanimalbutton;
    } else {
      refineItemIcon = refineorebutton;
    }
    const itemStyle = {
      transform: 'translate(-185px, 25px)',
      position: 'fixed'
    } as React.CSSProperties

    const spanNameStyle = {
      transform: 'translate(-323px, 85px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    const tableStyle = {
      transform: 'translate(20px, -250px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '300px'
    } as React.CSSProperties

    const tableStyle2 = {
      transform: 'translate(-50px, 15px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const tableStyleProduces = {
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const divideStyle = {
      transform: 'translate(-137px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const buyStyle = {
      transform: 'translate(-187px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const sellStyle = {
      transform: 'translate(-187px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const equipStyle = {
      transform: 'translate(-137px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const refineStyle = {
      transform: 'translate(-187px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const useStyle = {
      transform: 'translate(-187px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const deleteStyle = {
      transform: 'translate(-87px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    return (
      <HalfPanel left={isLeftPanel}
        panelType={'item'}
        hideExitButton={false}
        middle={isMiddlePanel}
        zIndexBonus={topLevel ? 3 : 0}>

        <img src={'/static/art/items/' + imageName} style={itemStyle} />
        <span style={spanNameStyle}>
          {itemName} x {this.props.itemData.quantity}
        </span>
        <table style={tableStyle}>
          <tbody>
            {hasEquipable &&
              <tr>
                <td>Equipped: </td>
                <td>{String(this.props.itemData.equipped)}</td>
              </tr>
            }
            <tr>
              <td>Class: </td>
              <td>{this.props.itemData.subclass} ({this.props.itemData.class})</td>
            </tr>
            <tr>
              <td>Weight: </td>
              <td>
                {this.props.itemData.weight} per unit
                ({this.props.itemData.quantity * this.props.itemData.weight})
              </td>
            </tr>

            {hasDurability &&
              <tr>
                <td>Durability: </td>
                <td>{this.props.itemData.durability}</td>
              </tr>
            }

            {attrs}

            {hasProduces &&
              <tr>
                <td>Produces: </td>
                <td>
                  <table style={tableStyleProduces}>
                    <tbody>
                      {produces}
                    </tbody>
                  </table>
                </td>
              </tr>
            }
            {hasEffects &&
              <tr>
                <td>Effects: </td>
                <td>
                  <table style={tableStyle2}>
                    <tbody>
                      {effects}
                    </tbody>
                  </table>
                </td>
              </tr>
            }
            {hasPrice &&
              <tr>
                <td>Price: </td>
                <td>{this.props.itemData.price}</td>
              </tr>
            }
          </tbody>
        </table>

        {showDivideButton &&
          <img src={dividebutton}
            style={divideStyle}
            onClick={this.handleDivideClick} />}

        {showBuyButton &&
          <img src={buybutton}
            style={buyStyle}
            onClick={this.handleBuyClick} />}

        {showSellButton &&
          <img src={sellbutton}
            style={sellStyle}
            onClick={this.handleSellClick} />}

        {showUseButton &&
          <img src={usebutton}
            style={useStyle}
            onClick={this.handleUseClick} />}

        {hasProduces &&
          <img src={refineItemIcon}
            style={refineStyle}
            onClick={this.handleRefineClick} />}

        {showDeleteButton &&
          <img src={deletebutton}
            style={deleteStyle}
            onClick={this.handleDeleteClick} />}

      </HalfPanel>
    );
  }
}

