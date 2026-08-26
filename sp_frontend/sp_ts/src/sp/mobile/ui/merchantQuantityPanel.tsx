
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
      width: 'min(333px, calc(100vw - 20px - env(safe-area-inset-left, 0px) - env(safe-area-inset-right, 0px)))',
      height: '119px',
      transform: 'translate(-50%, -50%)',
      position: 'fixed',
      zIndex: MOBILE_DIALOG_Z
    } as React.CSSProperties

    const merchantPanelStyle = {
      position: 'absolute',
      inset: 0,
      width: '100%',
      height: '100%',
      objectFit: 'fill',
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
      left: '18px',
      top: '30px',
      position: 'absolute',
    } as React.CSSProperties

    const rightStyle = {
      right: '18px',
      top: '30px',
      position: 'absolute',
    } as React.CSSProperties

    const transferStyle = {
      left: '50%',
      top: '30px',
      transform: 'translateX(-50%)',
      position: 'absolute',
    } as React.CSSProperties

    const buySellButtonStyle = {
      left: 'calc(50% - 50px)',
      bottom: '3px',
      position: 'absolute',
    } as React.CSSProperties

    const cancelButtonStyle = {
      left: '50%',
      bottom: '3px',
      position: 'absolute',
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
