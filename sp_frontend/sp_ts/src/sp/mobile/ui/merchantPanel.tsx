
import * as React from "react";
import BaseInventoryPanel from "./baseInventoryPanel";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { Network } from "../../core/network";
import tradebutton from "ui_comp/tradebutton.png";
import buybutton from "ui_comp/buybutton.png";
import sellbutton from "ui_comp/sellbutton.png";
import hirebutton from "ui_comp/assignbutton.png";
import WantedInventoryPanel from "./wantedInventoryPanel";
import ToggleLinkedButton from "./toggleLinkedButton";
import styles from "./../ui.module.css";

interface MPProps {
  leftInventoryData,
  rightInventoryData
  merchantWantedItems,
}

export default class MerchantPanel extends React.Component<MPProps, any> {
  constructor(props) {
    super(props);

    Global.selectedItemId = -1;
    Global.selectedItemOwnerId = -1;
    Global.merchantSellTarget = this.props.rightInventoryData.id;

    this.state = {
      hideLeftSelect: true,
      hideRightSelect: true,
      action: 'items_for_sale',
      showBuyButtonSelected: false,
      showSellButtonSelected: false,
      leftSelectedItemId: -1,
      rightSelectedItemId: -1
    };

    this.handleSelect = this.handleSelect.bind(this);
    this.handleBuyClick = this.handleBuyClick.bind(this);
    this.handleSellClick = this.handleSellClick.bind(this);
    this.handleInfoHireClick = this.handleInfoHireClick.bind(this);
  }

  handleSelect(eventData) {
    console.log(eventData);

    if (this.state.action == 'items_for_sale') {
      if (Global.selectedItemOwnerId == this.props.leftInventoryData.id) {
        this.setState({
          hideLeftSelect: false,
          hideRightSelect: true,
          leftSelectedItemId: eventData.itemId,
          rightSelectedItemId: -1
        });

        Global.infoItemAction = 'player_selling_item';

        Global.network.sendInfoItem(this.props.leftInventoryData.id, Global.selectedItemId, Global.infoItemAction);
      } else {
        this.setState({
          hideLeftSelect: true,
          hideRightSelect: false,
          leftSelectedItemId: -1,
          rightSelectedItemId: eventData.itemId
        });
        Global.infoItemAction = 'player_buying_item';

        Global.network.sendInfoItem(this.props.rightInventoryData.id, Global.selectedItemId, Global.infoItemAction);
      }

    } else if (this.state.action == 'items_wanted') {
      if (eventData.ownerId == this.props.leftInventoryData.id) {
        this.setState({
          hideLeftSelect: false,
          hideRightSelect: true,
          leftSelectedItemId: eventData.itemId,
          rightSelectedItemId: -1
        });

        Global.infoItemAction = 'player_selling_item';

        Global.network.sendInfoItem(this.props.leftInventoryData.id, Global.selectedItemId, Global.infoItemAction);
      } else {
        this.setState({
          hideLeftSelect: true,
          hideRightSelect: false,
          leftSelectedItemId: -1,
          rightSelectedItemId: eventData.itemId
        });

        Global.gameEmitter.emit(GameEvent.MERCHANT_WANTED_ITEM_CLICK, {});
      }
    } else {
      console.log("Invalid action: " + this.state.action);
    }
  }

  handleBuyClick() {
    this.setState({
      action: 'items_for_sale',
      showBuyButtonSelected: true,
      showSellButtonSelected: false,
    });
  }

  handleSellClick() {
    this.setState({
      action: 'items_wanted',
      showSellButtonSelected: true,
      showBuyButtonSelected: false
    });
  }


  handleInfoHireClick() {
    Global.network.sendInfoHire(Global.merchantSellTarget);
  }

  render() {
    console.log('this.props.leftInventoryData.id: ' + this.props.leftInventoryData.id);
    console.log('this.props.rightInventoryData.id: ' + this.props.rightInventoryData.id);

    const windowHeight = window.innerHeight;
    const isLargeWindow = windowHeight > 700;

    const buySmallY = '-25px';
    const buyLargeY = '235px';

    const sellSmallY = '-25px';
    const sellLargeY = '235px';

    const hireSmallY = '-25px';
    const hireLargeY = '235px';

    //const itemNameSmallY = '125px';
    //const itemNameLargeY = '385px';

    /*const actionNameStyle = {
      transform: 'translate(-323px, 90px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    const buySellButtonStyle = {
      transform: 'translate(116px, 90px)',
      position: 'fixed'
    } as React.CSSProperties

    const cancelButtonStyle = {
      transform: 'translate(166px, 90px)',
      position: 'fixed'
    } as React.CSSProperties*/

    const buyStyle = {
      top: '50%',
      left: '50%',
      marginTop: isLargeWindow ? buyLargeY : buySmallY,
      marginLeft: '-25px',
      position: 'fixed',
      transform: 'translate(108px, 135px)',
      zIndex: 6
    } as React.CSSProperties

    const sellStyle = {
      top: '50%',
      left: '50%',
      marginTop: isLargeWindow ? sellLargeY : sellSmallY,
      marginLeft: '-25px',
      position: 'fixed',
      transform: 'translate(161px, 135px)',
      zIndex: 6
    } as React.CSSProperties

    const hireStyle = {
      top: '50%',
      left: '50%',
      marginTop: isLargeWindow ? hireLargeY : hireSmallY,
      marginLeft: '-25px',
      position: 'fixed',
      transform: 'translate(214px, 135px)',
      zIndex: 6
    } as React.CSSProperties

    console.log("this.state.action: " + this.state.action);

    return (
      <div>
        <BaseInventoryPanel left={true}
          id={this.props.leftInventoryData.id}
          items={this.props.leftInventoryData.items}
          panelType={'merchant'}
          hideExitButton={true}
          hideSelect={this.state.hideLeftSelect}
          handleSelect={this.handleSelect}
          selectedItemId={this.state.leftSelectedItemId} />

        {(this.state.action == 'items_for_sale') &&
          <BaseInventoryPanel left={false}
            id={this.props.rightInventoryData.id}
            items={this.props.rightInventoryData.items}
            panelType={'merchant'}
            hideExitButton={false}
            hideSelect={this.state.hideRightSelect}
            handleSelect={this.handleSelect}
            selectedItemId={this.state.rightSelectedItemId} />
        }

        {(this.state.action == 'items_wanted') &&
          <WantedInventoryPanel left={false}
            id={this.props.rightInventoryData.id}
            items={this.props.merchantWantedItems}
            panelType={'merchant'}
            hideExitButton={false}
            hideSelect={this.state.hideRightSelect}
            handleSelect={this.handleSelect} />
        }

        <ToggleLinkedButton handler={this.handleBuyClick}
          imageName="buybutton"
          style={buyStyle}
          toggleIconBorder={this.state.showBuyButtonSelected} />

        <ToggleLinkedButton handler={this.handleSellClick}
          imageName="sellbutton"
          style={sellStyle}
          toggleIconBorder={this.state.showSellButtonSelected} />

        <img src={hirebutton}
          style={hireStyle}
          onClick={this.handleInfoHireClick} />
      </div>
    );
  }
}

