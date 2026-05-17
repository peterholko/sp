
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
import MobilePanelScreen from "./mobilePanelScreen";
import MobileInventoryGrid from "./mobileInventoryGrid";

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
      rightSelectedItemId: -1,
      leftPage: 0,
      rightPage: 0,
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

    console.log("this.state.action: " + this.state.action);

    const pageSize = 12;
    const leftItems = (this.props.leftInventoryData.items || []).filter((item) => item.equipped == false);
    const saleItems = (this.props.rightInventoryData.items || []).filter((item) => item.equipped == false);
    const wantedItems = (this.props.merchantWantedItems || []).map((item, index) => {
      let itemName = item.name || item.subclass || item.class || 'Wanted Item';
      return {
        id: `wanted-${index}`,
        name: itemName,
        image: itemName.toLowerCase().replace(/\s/g, ''),
        quantity: item.quantity,
        price: item.price,
        wantedSource: item,
      };
    });

    const rightItems = this.state.action == 'items_for_sale' ? saleItems : wantedItems;
    const leftPages = Math.max(1, Math.ceil(leftItems.length / pageSize));
    const rightPages = Math.max(1, Math.ceil(rightItems.length / pageSize));
    const leftPage = Math.min(this.state.leftPage, leftPages - 1);
    const rightPage = Math.min(this.state.rightPage, rightPages - 1);
    const leftPageItems = leftItems.slice(leftPage * pageSize, (leftPage + 1) * pageSize);
    const rightPageItems = rightItems.slice(rightPage * pageSize, (rightPage + 1) * pageSize);

    const setPage = (key, page, totalPages) => {
      this.setState({ [key]: Math.max(0, Math.min(page, totalPages - 1)) });
    };

    const handleInventorySelect = (eventData) => {
      Global.selectedItemOwnerId = eventData.ownerId;
      Global.selectedItemId = eventData.itemId;
      Global.selectedItemName = eventData.itemName;
      this.handleSelect(eventData);
    };

    const handleWantedSelect = (eventData) => {
      Global.selectedItemOwnerId = eventData.ownerId;
      Global.selectedItemId = -1;
      Global.selectedItemName = eventData.itemName;
      Global.wantedItemName = eventData.itemName;
      this.handleSelect({ ...eventData, itemId: -1 });
    };

    const columnStyle: React.CSSProperties = {
      minWidth: 0,
      display: 'flex',
      flexDirection: 'column',
      gap: '8px',
      border: '1px solid rgba(201, 170, 113, 0.24)',
      borderRadius: '6px',
      background: 'rgba(255,255,255,0.04)',
      padding: '8px',
      boxSizing: 'border-box',
    };

    const titleStyle: React.CSSProperties = {
      color: '#c9aa71',
      fontFamily: 'Cinzel, Verdana, serif',
      fontSize: '13px',
      fontWeight: 'bold',
      lineHeight: 1.2,
    };

    const gridWrapStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)',
      gap: '8px',
    };

    const pagerStyle: React.CSSProperties = {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      gap: '6px',
      minHeight: '32px',
      color: '#9aa0a6',
      fontSize: '10px',
    };

    const buttonStyle: React.CSSProperties = {
      minHeight: '36px',
      border: '1px solid rgba(201, 170, 113, 0.45)',
      borderRadius: '4px',
      background: '#25282b',
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '11px',
    };

    const footerStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: '1fr 1fr 1fr',
      gap: '8px',
    };

    const pager = (page, totalPages, key) => (
      <div style={pagerStyle}>
        <button type="button" style={buttonStyle} disabled={page == 0} onClick={() => setPage(key, page - 1, totalPages)}>Prev</button>
        <span>{page + 1}/{totalPages}</span>
        <button type="button" style={buttonStyle} disabled={page >= totalPages - 1} onClick={() => setPage(key, page + 1, totalPages)}>Next</button>
      </div>
    );

    const footer = (
      <div style={footerStyle}>
        <button type="button" style={buttonStyle} onClick={this.handleBuyClick}>
          For Sale
        </button>
        <button type="button" style={buttonStyle} onClick={this.handleSellClick}>
          Wanted
        </button>
        <button type="button" style={buttonStyle} onClick={this.handleInfoHireClick}>
          Hire
        </button>
      </div>
    );

    return (
      <MobilePanelScreen panelType="merchant" title="Merchant" footer={footer}>
        <div style={gridWrapStyle}>
          <div style={columnStyle}>
            <div style={titleStyle}>Your Items</div>
            <MobileInventoryGrid
              ownerId={this.props.leftInventoryData.id}
              items={leftPageItems}
              selectedItemId={this.state.leftSelectedItemId}
              onSelect={handleInventorySelect}
              compact={true}
            />
            {leftPages > 1 && pager(leftPage, leftPages, 'leftPage')}
          </div>
          <div style={columnStyle}>
            <div style={titleStyle}>{this.state.action == 'items_for_sale' ? 'For Sale' : 'Wanted'}</div>
            <MobileInventoryGrid
              ownerId={this.props.rightInventoryData.id}
              items={rightPageItems}
              selectedItemId={this.state.rightSelectedItemId}
              onSelect={this.state.action == 'items_for_sale' ? handleInventorySelect : handleWantedSelect}
              compact={true}
            />
            {rightPages > 1 && pager(rightPage, rightPages, 'rightPage')}
          </div>
        </div>
      </MobilePanelScreen>
    );
  }
}
