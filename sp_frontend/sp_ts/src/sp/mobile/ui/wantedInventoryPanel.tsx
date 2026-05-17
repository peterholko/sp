import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import { Global } from "../../core/global";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import {
  MobileCard,
  MobilePanelActions,
  MobileSplitPanelLayout,
  MobileSummaryCard,
  formatMobileQuantity,
  isLandscapeMobile,
} from "./mobilePanelLayout";

interface WantedInventoryProps {
  left: boolean,
  id: integer,
  items: any,
  panelType: string,
  hideExitButton: boolean,
  hideSelect: boolean,
  handleSelect: Function,
}

export default class WantedInventoryPanel extends React.Component<WantedInventoryProps, any> {
  constructor(props) {
    super(props);

    this.state = {
      selectedItemName: Global.wantedItemName,
      page: 0
    };

    this.handleSelect = this.handleSelect.bind(this)
    this.handleLeftClick = this.handleLeftClick.bind(this);
    this.handleRightClick = this.handleRightClick.bind(this);
  }

  handleSelect(eventData) {
    console.log('handleSelect ' + eventData);

    Global.wantedItemName = eventData.itemName;

    this.setState({ selectedItemName: eventData.itemName });

    this.props.handleSelect(eventData);
  }

  handleLeftClick(event) {
    console.log("Left Click - page: " + this.state.page);
    if (this.state.page != 0) {
      const newPage = this.state.page - 1;
      this.setState({ page: newPage })
    }
  }

  handleRightClick(event) {
    console.log("Right Click - page: " + this.state.page);
    if (this.state.page != (Math.ceil(this.props.items.length / 20) - 1)) {
      const newPage = this.state.page + 1;
      this.setState({ page: newPage })
    }
  }

  render() {
    const objId = this.props.id;
    var imageName;
    var hideLeftButton = false;
    var hideRightButton = false;

    var itemsData = this.props.items;
    const landscape = isLandscapeMobile();

    if (Global.objectStates[objId]) {
      imageName = Global.objectStates[objId].image + '.png';
    } else {
      imageName = 'unknownunit.png';
    }

    var maxItemIndex = (this.state.page + 1) * 20;

    if (maxItemIndex > (itemsData.length - 1)) {
      maxItemIndex = itemsData.length;
    }

    console.log("maxItemIndex: " + maxItemIndex);
    console.log("state page: " + this.state)

    const pageItems = itemsData.slice(this.state.page * 20, maxItemIndex);

    if (this.state.page == 0) {
      hideLeftButton = true;
    }

    if (itemsData.length == 0) {
      hideRightButton = true;
    } else if ((Math.ceil(itemsData.length / 20) - 1) == this.state.page) {
      hideRightButton = true;
    }

    const tileSize = landscape ? 58 : 64;

    const gridStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: `repeat(auto-fill, ${tileSize}px)`,
      gridAutoRows: `${tileSize}px`,
      gap: landscape ? '6px' : '8px',
      justifyContent: 'start',
      alignItems: 'start',
    };

    const buttonStyle = (selected: boolean): React.CSSProperties => ({
      width: `${tileSize}px`,
      height: `${tileSize}px`,
      minHeight: `${tileSize}px`,
      position: 'relative',
      border: selected ? '2px solid #c9aa71' : '1px solid rgba(201, 170, 113, 0.24)',
      borderRadius: '4px',
      background: selected ? 'rgba(201, 170, 113, 0.18)' : 'rgba(255,255,255,0.05)',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      padding: '3px',
      boxSizing: 'border-box',
    });

    const imageStyle: React.CSSProperties = {
      width: '48px',
      height: '48px',
      objectFit: 'contain',
      imageRendering: 'pixelated',
    };

    const badgeStyle: React.CSSProperties = {
      position: 'absolute',
      right: '2px',
      bottom: '1px',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '9px',
      WebkitTextStroke: '0.5px black',
      fontWeight: 'bold',
    };

    const priceStyle: React.CSSProperties = {
      ...badgeStyle,
      left: '2px',
      right: 'auto',
      color: '#c9aa71',
    };

    return (
      <MobilePanelScreen
        panelType={this.props.panelType}
        title={this.props.panelType}
        hideExitButton={this.props.hideExitButton}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={<MobileSummaryCard imageSrc={'/static/art/' + imageName} title={Global.objectStates[objId].name} subtitle={`Page ${this.state.page + 1} / ${Math.max(1, Math.ceil(itemsData.length / 20))}`} imageSize={landscape ? 58 : 82} />}
          right={
            <>
              <MobileCard compact={landscape}>
                <div style={gridStyle}>
                  {pageItems.map((item, itemPageIndex) => {
                    let itemName;

                    if(item.name) {
                      itemName = item.name;
                    } else if(item.subclass) {
                      itemName = item.subclass;
                    } else if(item.class) {
                      itemName = item.class;
                    }

                    const image = itemName.toLowerCase().replace(/\s/g, '');
                    const selected = this.state.selectedItemName == itemName;
                    const handleClick = () => {
                      Global.wantedItemData = {
                        ownerId: objId,
                        itemName,
                        quantity: item.quantity,
                        price: item.price,
                      };
                      this.handleSelect({ ownerId: objId, itemName, index: itemPageIndex });
                    };

                    return (
                      <button key={itemPageIndex} type="button" style={buttonStyle(selected)} onClick={handleClick} title={itemName}>
                        <img src={'/static/art/items/' + image + '.png'} style={imageStyle} />
                        <span style={priceStyle}>{formatMobileQuantity(item.price)}</span>
                        <span style={badgeStyle}>{formatMobileQuantity(item.quantity)}</span>
                      </button>
                    );
                  })}
                </div>
              </MobileCard>
              <MobilePanelActions actions={[
                { key: 'previous', label: 'Previous page', icon: leftbutton, onClick: this.handleLeftClick, disabled: hideLeftButton },
                { key: 'next', label: 'Next page', icon: rightbutton, onClick: this.handleRightClick, disabled: hideRightButton },
              ]} />
            </>
          } />
      </MobilePanelScreen>
    );
  }
}
