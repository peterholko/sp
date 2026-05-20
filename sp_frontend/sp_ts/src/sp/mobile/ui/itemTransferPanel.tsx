import * as React from "react";
import transferbutton from "ui_comp/transferbutton.png";
import buildbutton from "ui_comp/buildbutton.png";
import upgradebutton from "ui_comp/upgradebutton.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { STRUCTURE, FOUNDED, PLANNING_UPGRADE } from "../../core/config";
import { Util } from "../../core/util";
import MobilePanelScreen from "./mobilePanelScreen";
import MobileInventoryGrid from "./mobileInventoryGrid";

interface ITPProps {
  leftInventoryData,
  rightInventoryData,
  reqs
}

export default class ItemTransferPanel extends React.Component<ITPProps, any> {
  constructor(props) {
    super(props);

    Global.selectedItemId = -1;
    Global.selectedItemOwnerId = -1;
    Global.selectedItemName = '';

    this.state = {
      leftSelectedItemId: -1,
      rightSelectedItemId: -1,
      selectedItemName: '',
      leftPage: 0,
      rightPage: 0,
    };

    this.handleSelect = this.handleSelect.bind(this);
    this.handleItemTransferClick = this.handleItemTransferClick.bind(this);
    this.handleBuildClick = this.handleBuildClick.bind(this);
    this.handleUpgradeClick = this.handleUpgradeClick.bind(this);
  }

  handleSelect(eventData) {
    Global.selectedItemOwnerId = eventData.ownerId;
    Global.selectedItemId = eventData.itemId;
    Global.selectedItemName = eventData.itemName;

    if (eventData.ownerId == this.props.leftInventoryData.id) {
      this.setState({
        leftSelectedItemId: eventData.itemId,
        rightSelectedItemId: -1,
        selectedItemName: eventData.itemName,
      });
    } else {
      this.setState({
        leftSelectedItemId: -1,
        rightSelectedItemId: eventData.itemId,
        selectedItemName: eventData.itemName,
      });
    }
  }

  handleItemTransferClick() {
    if (Global.selectedItemId != -1) {
      let sourceId;
      let targetId;

      if (Global.selectedItemOwnerId == this.props.leftInventoryData.id) {
        sourceId = this.props.leftInventoryData.id;
        targetId = this.props.rightInventoryData.id;
      } else {
        sourceId = this.props.rightInventoryData.id;
        targetId = this.props.leftInventoryData.id;
      }

      Global.network.sendItemTransfer(Global.selectedItemId, sourceId, targetId);

      Global.selectedItemId = -1;
      Global.selectedItemOwnerId = -1;
      Global.selectedItemName = '';

      this.setState({
        leftSelectedItemId: -1,
        rightSelectedItemId: -1,
        selectedItemName: '',
      });
    }
  }

  handleBuildClick() {
    Global.network.sendBuild(Global.heroId, this.props.rightInventoryData.id);
    Global.gameEmitter.emit(GameEvent.START_BUILD_CLICK, {});
  }

  handleUpgradeClick() {
    Global.network.sendUpgrade(Global.heroId, this.props.rightInventoryData.id);
    Global.gameEmitter.emit(GameEvent.START_UPGRADE_CLICK, {});
  }

  objectSummary(inventoryData) {
    const objState = Global.objectStates[inventoryData.id];
    if (!objState) {
      return { name: 'Inventory', imageName: '' };
    }

    const imageName = Util.isSprite(objState.image)
      ? objState.image + '_single.png'
      : objState.image + '.png';

    return { name: objState.name, imageName };
  }

  pagedItems(items, page, pageSize) {
    const filtered = (items || []);
    const totalPages = Math.max(1, Math.ceil(filtered.length / pageSize));
    const safePage = Math.min(page, totalPages - 1);
    return {
      items: filtered.slice(safePage * pageSize, (safePage + 1) * pageSize),
      totalPages,
      page: safePage,
    };
  }

  ownerCanEquip(inventoryData) {
    const ownerState = Global.objectStates[inventoryData.id];
    return ownerState && (ownerState.subclass == 'hero' || ownerState.subclass == 'villager');
  }

  renderPager(side: 'left' | 'right', page: number, totalPages: number) {
    if (totalPages <= 1) return null;

    const pagerStyle: React.CSSProperties = {
      display: 'flex',
      justifyContent: 'space-between',
      alignItems: 'center',
      gap: '4px',
      marginTop: '8px',
      color: '#9aa0a6',
      fontSize: '10px',
    };

    const buttonStyle: React.CSSProperties = {
      minHeight: '34px',
      minWidth: '44px',
      border: '1px solid rgba(201, 170, 113, 0.45)',
      borderRadius: '4px',
      background: '#25282b',
      color: '#f2e7cf',
      fontSize: '11px',
    };

    const pageKey = side == 'left' ? 'leftPage' : 'rightPage';

    return (
      <div style={pagerStyle}>
        <button
          type="button"
          style={buttonStyle}
          disabled={page == 0}
          onClick={() => this.setState({ [pageKey]: Math.max(page - 1, 0) })}
        >
          Prev
        </button>
        <span>{page + 1}/{totalPages}</span>
        <button
          type="button"
          style={buttonStyle}
          disabled={page >= totalPages - 1}
          onClick={() => this.setState({ [pageKey]: Math.min(page + 1, totalPages - 1) })}
        >
          Next
        </button>
      </div>
    );
  }

  renderRequirements(isPlanningUpgrade: boolean) {
    const reqs = this.props.reqs || [];
    if (reqs.length == 0) return null;

    const complete = reqs.every(req => req.cquantity == 0);

    const wrapStyle: React.CSSProperties = {
      border: '1px solid rgba(201, 170, 113, 0.22)',
      borderRadius: '4px',
      padding: '8px',
      marginBottom: '10px',
      background: 'rgba(255,255,255,0.04)',
    };

    const titleStyle: React.CSSProperties = {
      color: '#c9aa71',
      fontSize: '10px',
      fontWeight: 'bold',
      marginBottom: '6px',
      textTransform: 'uppercase',
    };

    const reqGridStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: 'repeat(auto-fill, 56px)',
      gridAutoRows: '56px',
      gap: '6px',
      justifyContent: 'start',
      alignItems: 'start',
    };

    const reqStyle: React.CSSProperties = {
      position: 'relative',
      width: '56px',
      height: '56px',
      minHeight: '56px',
      border: '1px solid rgba(255,255,255,0.12)',
      borderRadius: '4px',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      background: 'rgba(0,0,0,0.18)',
    };

    const qtyStyle: React.CSSProperties = {
      position: 'absolute',
      right: '2px',
      bottom: '1px',
      color: 'white',
      fontSize: '9px',
      fontWeight: 'bold',
      WebkitTextStroke: '0.5px black',
    };

    const actionButtonStyle: React.CSSProperties = {
      width: '100%',
      minHeight: '44px',
      marginTop: '8px',
      border: '1px solid rgba(201, 170, 113, 0.45)',
      borderRadius: '4px',
      background: complete ? '#25282b' : 'rgba(37, 40, 43, 0.55)',
      color: '#f2e7cf',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      opacity: complete ? 1 : 0.5,
    };

    return (
      <div style={wrapStyle}>
        <div style={titleStyle}>{isPlanningUpgrade ? 'Upgrade Requirements' : 'Build Requirements'}</div>
        <div style={reqGridStyle}>
          {reqs.map((req, index) => {
            const resourceImage = req.type.toLowerCase().replace(/\s/g, '');
            return (
              <div key={index} style={reqStyle} title={req.type}>
                <img src={'/static/art/items/' + resourceImage + '.png'} style={{ width: '48px', height: '48px', objectFit: 'contain', imageRendering: 'pixelated' }} />
                <span style={qtyStyle}>{req.cquantity}/{req.quantity}</span>
              </div>
            );
          })}
        </div>
        <button
          type="button"
          style={actionButtonStyle}
          disabled={!complete}
          onClick={isPlanningUpgrade ? this.handleUpgradeClick : this.handleBuildClick}
        >
          <img src={isPlanningUpgrade ? upgradebutton : buildbutton} style={{ width: '40px', height: '40px' }} />
        </button>
      </div>
    );
  }

  renderColumn(label: string, inventoryData, selectedItemId, side: 'left' | 'right', special?: React.ReactNode) {
    const pageSize = 12;
    const pageState = side == 'left' ? this.state.leftPage : this.state.rightPage;
    const pageData = this.pagedItems(inventoryData.items, pageState, pageSize);
    const disabledItems = this.ownerCanEquip(inventoryData)
      ? pageData.items
        .filter(item => item.equipped == true)
        .map(item => item.id)
      : [];
    const summary = this.objectSummary(inventoryData);
    const capacityText = inventoryData.cap != null && inventoryData.tw != null
      ? inventoryData.tw + '/' + inventoryData.cap + ' lbs'
      : '';

    const columnStyle: React.CSSProperties = {
      minWidth: 0,
      border: '1px solid rgba(201, 170, 113, 0.28)',
      borderRadius: '5px',
      padding: '8px',
      background: 'rgba(255,255,255,0.05)',
      boxSizing: 'border-box',
    };

    const headerStyle: React.CSSProperties = {
      display: 'flex',
      alignItems: 'center',
      gap: '7px',
      minHeight: '78px',
      marginBottom: '8px',
    };

    const imageStyle: React.CSSProperties = {
      width: '72px',
      height: '72px',
      objectFit: 'contain',
      imageRendering: 'pixelated',
      flex: '0 0 auto',
    };

    const eyebrowStyle: React.CSSProperties = {
      color: '#8fb7d9',
      fontSize: '9px',
      fontWeight: 'bold',
      textTransform: 'uppercase',
    };

    const nameStyle: React.CSSProperties = {
      color: '#f2e7cf',
      fontSize: '11px',
      lineHeight: 1.15,
      fontWeight: 'bold',
      wordBreak: 'break-word',
    };

    const capStyle: React.CSSProperties = {
      color: '#9aa0a6',
      fontSize: '9px',
      marginTop: '2px',
    };

    return (
      <div style={columnStyle}>
        <div style={headerStyle}>
          {summary.imageName && <img src={'/static/art/' + summary.imageName} style={imageStyle} />}
          <div style={{ minWidth: 0 }}>
            <div style={eyebrowStyle}>{label}</div>
            <div style={nameStyle}>{summary.name}</div>
            {capacityText && <div style={capStyle}>{capacityText}</div>}
          </div>
        </div>
        {special}
        <MobileInventoryGrid
          ownerId={inventoryData.id}
          items={pageData.items}
          selectedItemId={selectedItemId}
          disabledItems={disabledItems}
          onSelect={this.handleSelect}
          compact={true}
        />
        {this.renderPager(side, pageData.page, pageData.totalPages)}
      </div>
    );
  }

  render() {
    const objState = Global.objectStates[this.props.rightInventoryData.id];
    const isFounded = objState && objState.class == STRUCTURE && objState.state == FOUNDED;
    const isPlanningUpgrade = objState && objState.class == STRUCTURE && objState.state == PLANNING_UPGRADE;
    const selected = Global.selectedItemId != -1;

    const columnsStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: 'minmax(0, 1fr) minmax(0, 1fr)',
      gap: '8px',
      alignItems: 'start',
    };

    const footerStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: '1fr 56px',
      gap: '8px',
      alignItems: 'center',
    };

    const selectedStyle: React.CSSProperties = {
      minWidth: 0,
      color: selected ? '#f2e7cf' : '#9aa0a6',
      fontSize: '12px',
      lineHeight: 1.25,
      overflow: 'hidden',
      textOverflow: 'ellipsis',
      whiteSpace: 'nowrap',
    };

    const transferStyle: React.CSSProperties = {
      width: '56px',
      height: '48px',
      border: '1px solid rgba(201, 170, 113, 0.55)',
      borderRadius: '4px',
      background: selected ? '#25282b' : 'rgba(37, 40, 43, 0.5)',
      opacity: selected ? 1 : 0.45,
    };

    return (
      <MobilePanelScreen
        panelType="itemTransfer"
        title="Transfer Items"
        footer={
          <div style={footerStyle}>
            <div style={selectedStyle}>
              {selected ? this.state.selectedItemName : 'Select an item from either inventory'}
            </div>
            <button type="button" style={transferStyle} disabled={!selected} onClick={this.handleItemTransferClick}>
              <img src={transferbutton} style={{ width: '40px', height: '40px' }} />
            </button>
          </div>
        }
      >
        <div style={columnsStyle}>
          {this.renderColumn('Source', this.props.leftInventoryData, this.state.leftSelectedItemId, 'left')}
          {this.renderColumn(
            'Target',
            this.props.rightInventoryData,
            this.state.rightSelectedItemId,
            'right',
            (isFounded || isPlanningUpgrade) ? this.renderRequirements(Boolean(isPlanningUpgrade)) : null
          )}
        </div>
      </MobilePanelScreen>
    );
  }
}
