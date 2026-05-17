import * as React from "react";
import { Global } from "../../core/global";
import { Util } from "../../core/util";
import MobilePanelScreen from "./mobilePanelScreen";
import MobileInventoryGrid from "./mobileInventoryGrid";

interface BaseInventoryProps {
  left: boolean,
  id: integer,
  items: any,
  capacity?: integer,
  totalWeight?: integer,
  panelType: string,
  hideExitButton: boolean,
  hideSelect: boolean,
  showEquippedOnly?: boolean
  handleSelect: Function,
  selectedItemId?: integer,
  disabledItems?: any,
  footer?: React.ReactNode,
}

export default class BaseInventoryPanel extends React.Component<BaseInventoryProps, any> {
  constructor(props) {
    super(props);
    this.state = {
      page: 0,
      selectedItemId: this.props.selectedItemId
    };

    this.handleSelect = this.handleSelect.bind(this);
    this.handleLeftClick = this.handleLeftClick.bind(this);
    this.handleRightClick = this.handleRightClick.bind(this);
  }

  handleSelect(eventData) {
    Global.selectedItemOwnerId = eventData.ownerId;
    Global.selectedItemId = eventData.itemId;
    Global.selectedItemName = eventData.itemName;

    this.setState({ selectedItemId: eventData.itemId });
    this.props.handleSelect(eventData);
  }

  handleLeftClick() {
    if (this.state.page != 0) {
      this.setState({ page: this.state.page - 1 });
    }
  }

  handleRightClick(totalPages?: number) {
    const maxPage = Math.max((totalPages || 1) - 1, 0);
    if (this.state.page < maxPage) {
      this.setState({ page: this.state.page + 1 });
    }
  }

  render() {
    const objId = this.props.id;
    let imageName = '';
    let name = '';

    if (Global.objectStates[objId]) {
      if (Util.isSprite(Global.objectStates[objId].image)) {
        imageName = Global.objectStates[objId].image + '_single.png';
      } else {
        imageName = Global.objectStates[objId].image + '.png';
      }
      name = Global.objectStates[objId].name;
    }

    let itemsData = (this.props.items || []).filter((item) => item.equipped == false);
    if (this.props.showEquippedOnly) {
      itemsData = itemsData.filter((item) =>
        item.class == "Weapon" || item.class == "Armor" || item.class == "Tool" || item.class == "Torch"
      );
    }

    const pageSize = 30;
    const totalPages = Math.max(1, Math.ceil(itemsData.length / pageSize));
    const page = Math.min(this.state.page, totalPages - 1);
    const pageItems = itemsData.slice(page * pageSize, (page + 1) * pageSize);
    const selectedItemId = this.props.selectedItemId ?? this.state.selectedItemId;

    const capacityText = this.props.capacity != null && this.props.totalWeight != null
      ? this.props.totalWeight + '/' + this.props.capacity + ' lbs'
      : '';

    const summaryStyle: React.CSSProperties = {
      display: 'flex',
      alignItems: 'center',
      gap: '10px',
      marginBottom: '12px',
      minHeight: '54px',
    };

    const spriteStyle: React.CSSProperties = {
      width: '46px',
      height: '46px',
      objectFit: 'contain',
      imageRendering: 'pixelated',
      flex: '0 0 auto',
    };

    const nameStyle: React.CSSProperties = {
      color: '#f2e7cf',
      fontFamily: 'Cinzel, Verdana, serif',
      fontSize: '16px',
      fontWeight: 'bold',
      lineHeight: 1.2,
    };

    const capacityStyle: React.CSSProperties = {
      color: '#9aa0a6',
      fontSize: '11px',
      marginTop: '3px',
    };

    const pagerStyle: React.CSSProperties = {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      gap: '10px',
      marginTop: '12px',
      color: '#9aa0a6',
      fontSize: '11px',
    };

    const pagerButtonStyle: React.CSSProperties = {
      minHeight: '40px',
      minWidth: '74px',
      border: '1px solid rgba(201, 170, 113, 0.45)',
      borderRadius: '4px',
      background: '#25282b',
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '12px',
    };

    return (
      <MobilePanelScreen
        panelType={this.props.panelType}
        title={name || 'Inventory'}
        hideExitButton={this.props.hideExitButton}
        footer={this.props.footer}
      >
        <div style={summaryStyle}>
          {imageName && <img src={'/static/art/' + imageName} style={spriteStyle} />}
          <div>
            <div style={nameStyle}>{name || 'Inventory'}</div>
            {capacityText && <div style={capacityStyle}>{capacityText}</div>}
          </div>
        </div>

        <MobileInventoryGrid
          ownerId={objId}
          items={pageItems}
          selectedItemId={selectedItemId}
          disabledItems={this.props.disabledItems}
          onSelect={this.handleSelect}
        />

        {totalPages > 1 &&
          <div style={pagerStyle}>
            <button
              type="button"
              style={pagerButtonStyle}
              disabled={page == 0}
              onClick={this.handleLeftClick}
            >
              Prev
            </button>
            <span>Page {page + 1} of {totalPages}</span>
            <button
              type="button"
              style={pagerButtonStyle}
              disabled={page >= totalPages - 1}
              onClick={() => this.handleRightClick(totalPages)}
            >
              Next
            </button>
          </div>}
      </MobilePanelScreen>
    );
  }
}
