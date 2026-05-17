
import * as React from "react";
import merchantquantitypanel from "ui_comp/errorframe.png";
import buybutton from "ui_comp/buybutton.png";
import sellbutton from "ui_comp/sellbutton.png";
import InventoryItem from "./inventoryItem";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import cancelbutton from "ui_comp/exitbutton.png";
import transferbutton from "ui_comp/transferbutton.png";
import { Network } from "../../core/network";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { MOBILE_DIALOG_Z } from "./mobileLayers";

interface MQPProps {
  itemData,
  action,
  targetId?
}

export default class MerchantQuantityPanel extends React.Component<MQPProps, any> {
  constructor(props) {
    super(props);

    let item = Object.assign({}, this.props.itemData);

    this.state = {
      item : item,
      goldcoins: 0,
    };
   
    this.handleBuyClick = this.handleBuyClick.bind(this);
    this.handleSellClick = this.handleSellClick.bind(this);
    this.handleLeftClick = this.handleLeftClick.bind(this);
    this.handleRightClick = this.handleRightClick.bind(this);
    this.handleCancelClick = this.handleCancelClick.bind(this);
  }

  handleLeftClick() {
    this.state.item.quantity = this.state.item.quantity - 1;
    this.setState({item: this.state.item});
  }

  handleRightClick() {
    this.state.item.quantity = this.state.item.quantity + 1;
    this.setState({item: this.state.item});
  }

  handleBuyClick() {
    Global.network.sendBuyItem(Global.merchantSellTarget, this.props.itemData.id, this.state.item.quantity);
    Global.gameEmitter.emit(GameEvent.MERCHANT_QUANTITY_CANCEL, {});
  }

  handleSellClick() {
    Global.network.sendSellItem(this.props.itemData.id, Global.merchantSellTarget, this.state.item.quantity);
    Global.gameEmitter.emit(GameEvent.MERCHANT_QUANTITY_CANCEL, {});
  }

  handleCancelClick() {
    Global.gameEmitter.emit(GameEvent.MERCHANT_QUANTITY_CANCEL, {});
  }

  render() {
    const hideLeft = this.state.item.quantity == 1;
    const hideRight = this.state.item.quantity == this.props.itemData.quantity;

    let xPosItem;
    let xPosGoldCoins;

    if(this.props.action == 'buy') {
      xPosItem = 80;
      xPosGoldCoins = 200;
    } else {
      xPosItem = 200;
      xPosGoldCoins = 80;
    }

    const merchantStyle = {
      top: '50%',
      left: '50%',
      width: '333px',
      height: '119px',
      marginTop: '-59px',
      marginLeft: '-166px',
      position: 'fixed',
      zIndex: MOBILE_DIALOG_Z
    } as React.CSSProperties

    const merchantPanelStyle = {
      position: 'fixed'
    } as React.CSSProperties

    const spanNameStyle = {
      transform: 'translate(15px, 20px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '14px',
      width: '300px'
    } as React.CSSProperties

    const leftStyle = {
      transform: 'translate(20px, 30px)',
      position: 'fixed'
    } as React.CSSProperties

    const rightStyle = {
      transform: 'translate(265px, 30px)',
      position: 'fixed'
    } as React.CSSProperties

    const transferStyle = {
      transform: 'translate(140px, 30px)',
      position: 'fixed'
    } as React.CSSProperties

    const buySellButtonStyle = {
      transform: 'translate(116px, 90px)',
      position: 'fixed'
    } as React.CSSProperties

    const cancelButtonStyle = {
      transform: 'translate(166px, 90px)',
      position: 'fixed'
    } as React.CSSProperties

    return (
      <div style={merchantStyle}>
        <img src={merchantquantitypanel} style={merchantPanelStyle}/>

        <InventoryItem key={'item'}
                       ownerId={this.state.item.owner}
                       itemId={this.state.item.id} 
                       itemName={this.state.item.itemName} 
                       image={this.state.item.image} 
                       quantity={this.state.item.quantity}
                       xPos={xPosItem}
                       yPos={30} />

        <InventoryItem key={'coins'}
                       ownerId={-1}
                       itemId={-1} 
                       itemName={"Gold Coins"} 
                       image={"goldcoins"} 
                       quantity={this.state.item.price * this.state.item.quantity}
                       xPos={xPosGoldCoins}
                       yPos={30} />                       

        <img src={transferbutton} style={transferStyle} />
        {!hideLeft && <img src={leftbutton} style={leftStyle} onClick={this.handleLeftClick} />}
        {!hideRight && <img src={rightbutton} style={rightStyle} onClick={this.handleRightClick} />}

        <img src={cancelbutton} style={cancelButtonStyle} onClick={this.handleCancelClick} />

        {(this.props.action == 'buy') &&
        <img src={buybutton} style={buySellButtonStyle} onClick={this.handleBuyClick}/> }

        {(this.props.action == 'sell') &&
        <img src={sellbutton} style={buySellButtonStyle} onClick={this.handleSellClick}/> }
      </div>
    );
  }
}
