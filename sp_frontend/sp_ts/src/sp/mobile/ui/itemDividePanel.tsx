
import * as React from "react";
import dividepanel from "ui_comp/errorframe.png";
import okbutton from "ui_comp/okbutton.png";
import InventoryItem from "./inventoryItem";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import { Network } from "../../core/network";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { MOBILE_DIALOG_Z } from "./mobileLayers";

interface ItemDivideProps {
  itemData,
}

export default class ItemDividePanel extends React.Component<ItemDivideProps, any> {
  constructor(props) {
    super(props);

    const leftItem = Object.assign({}, this.props.itemData);
    var rightItem = Object.assign({}, this.props.itemData);
    rightItem.quantity = 0;

    this.state = {
      leftItem : leftItem,
      rightItem : rightItem
    };
   
    this.handleOkClick = this.handleOkClick.bind(this);
    this.handleLeftClick = this.handleLeftClick.bind(this);
    this.handleRightClick = this.handleRightClick.bind(this);
  }

  handleLeftClick(event) { 
    this.state.leftItem.quantity = this.state.leftItem.quantity + 1;
    this.state.rightItem.quantity = this.state.rightItem.quantity - 1;

    this.setState({leftItem: this.state.leftItem,
                   rightItem: this.state.rightItem});
  }

  handleRightClick(event) {
    this.state.leftItem.quantity = this.state.leftItem.quantity - 1;
    this.state.rightItem.quantity = this.state.rightItem.quantity + 1;

    this.setState({leftItem: this.state.leftItem,
                   rightItem: this.state.rightItem});
  }

  handleOkClick() {
    Global.network.sendItemSplit(
      this.state.leftItem.owner,
      this.state.rightItem.id, 
      this.state.rightItem.quantity
    );

    Global.gameEmitter.emit(GameEvent.ITEM_DIVIDE_OK_CLICK, {});
  }

  render() {
    const divideStyle = {
      top: '50%',
      left: '50%',
      width: 'min(333px, calc(100vw - 20px - env(safe-area-inset-left, 0px) - env(safe-area-inset-right, 0px)))',
      height: '119px',
      transform: 'translate(-50%, -50%)',
      position: 'fixed',
      zIndex: MOBILE_DIALOG_Z
    } as React.CSSProperties

    const dividePanelStyle = {
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
      left: '38px',
      top: '30px',
      position: 'absolute',
    } as React.CSSProperties

    const rightStyle = {
      right: '38px',
      top: '30px',
      position: 'absolute',
    } as React.CSSProperties

    const okButtonStyle = {
      left: '50%',
      bottom: '3px',
      transform: 'translateX(-50%)',
      position: 'absolute',
    } as React.CSSProperties

    return (
      <div style={divideStyle}>
        <img src={dividepanel} style={dividePanelStyle}/>
        <InventoryItem key={'left'}
                       ownerId={this.state.leftItem.owner}
                       itemId={this.state.leftItem.id} 
                       itemName={this.state.leftItem.name}
                       image={this.state.leftItem.image} 
                       quantity={this.state.leftItem.quantity}
                       xPos={110}
                       yPos={30} />

        <InventoryItem key={'right'}
                       ownerId={this.state.rightItem.owner}
                       itemId={this.state.rightItem.id} 
                       itemName={this.state.rightItem.name} 
                       image={this.state.rightItem.image} 
                       quantity={this.state.rightItem.quantity}
                       xPos={175}
                       yPos={30} />  

        <img src={leftbutton} style={leftStyle} onClick={this.handleLeftClick} />
        <img src={rightbutton} style={rightStyle} onClick={this.handleRightClick} />
        <img src={okbutton} style={okButtonStyle} onClick={this.handleOkClick}/>
      </div>
    );
  }
}
