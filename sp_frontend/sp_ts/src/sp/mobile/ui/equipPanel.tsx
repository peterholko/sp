import * as React from "react";
import { Global } from "../../core/global";
import MobilePanelScreen from "./mobilePanelScreen";
import MobileInventoryGrid from "./mobileInventoryGrid";

interface EquipPanelProps {
  equipData
}

export default class EquipPanel extends React.Component<EquipPanelProps, any> {
  constructor(props) {
    super(props);
    this.state = {
      selectedItemId: -1,
    };

    this.handleSelect = this.handleSelect.bind(this);
    this.handleEquipClick = this.handleEquipClick.bind(this);
    this.handleItemInfoClick = this.handleItemInfoClick.bind(this);
  }

  selectedItem() {
    return (this.props.equipData.items || []).find(item => item.id == this.state.selectedItemId);
  }

  isVillager() {
    const objState = Global.objectStates[this.props.equipData.id];
    return objState && objState.subclass == 'villager';
  }

  handleSelect(eventData) {
    Global.selectedItemOwnerId = eventData.ownerId;
    Global.selectedItemId = eventData.itemId;
    Global.selectedItemName = eventData.itemName;
    this.setState({ selectedItemId: eventData.itemId });
  }

  handleEquipClick() {
    if (this.isVillager()) return;

    const selectedItem = this.selectedItem();
    if (!selectedItem) return;

    Global.network.sendEquip(this.props.equipData.id, selectedItem.id, !selectedItem.equipped);
    Global.selectedItemOwnerId = -1;
    Global.selectedItemId = -1;
    Global.selectedItemName = '';
    this.setState({ selectedItemId: -1 });
  }

  handleItemInfoClick() {
    const selectedItem = this.selectedItem();
    if (!selectedItem) return;

    Global.infoItemAction = selectedItem.equipped ? 'inventory' : 'equip';
    Global.network.sendInfoItem(this.props.equipData.id, selectedItem.id, "None");
  }

  renderSlot(slotName: string, item) {
    const selected = item && item.id == this.state.selectedItemId;
    const slotStyle: React.CSSProperties = {
      minHeight: '58px',
      border: selected ? '2px solid #c9aa71' : '1px solid rgba(201, 170, 113, 0.28)',
      borderRadius: '5px',
      background: selected ? 'rgba(201, 170, 113, 0.18)' : 'rgba(255,255,255,0.05)',
      color: '#f2e7cf',
      display: 'grid',
      gridTemplateColumns: '42px 1fr',
      alignItems: 'center',
      gap: '7px',
      padding: '6px',
      textAlign: 'left',
      boxSizing: 'border-box',
    };

    const slotLabelStyle: React.CSSProperties = {
      color: '#8fb7d9',
      fontSize: '9px',
      fontWeight: 'bold',
      textTransform: 'uppercase',
      lineHeight: 1.2,
    };

    const itemNameStyle: React.CSSProperties = {
      color: item ? '#f2e7cf' : '#777d82',
      fontSize: '10px',
      lineHeight: 1.2,
      wordBreak: 'break-word',
    };

    const imageStyle: React.CSSProperties = {
      width: '36px',
      height: '36px',
      objectFit: 'contain',
      imageRendering: 'pixelated',
      opacity: item ? 1 : 0.35,
    };

    return (
      <button
        key={slotName}
        type="button"
        style={slotStyle}
        disabled={!item}
        onClick={() => item && this.handleSelect({
          ownerId: this.props.equipData.id,
          itemId: item.id,
          itemName: item.name,
          index: slotName,
        })}
      >
        <img
          src={item ? '/static/art/items/' + item.image + '.png' : '/static/art/ui/itemframe.png'}
          style={imageStyle}
        />
        <div>
          <div style={slotLabelStyle}>{slotName}</div>
          <div style={itemNameStyle}>{item ? item.name : 'Empty'}</div>
        </div>
      </button>
    );
  }

  renderSelectedDetails(selectedItem) {
    if (!selectedItem) return null;

    const detailStyle: React.CSSProperties = {
      border: '1px solid rgba(201, 170, 113, 0.28)',
      borderRadius: '5px',
      padding: '9px',
      background: 'rgba(255,255,255,0.05)',
      marginTop: '12px',
      color: '#d4d4d4',
      fontSize: '11px',
      lineHeight: 1.35,
    };

    const titleStyle: React.CSSProperties = {
      color: '#f2e7cf',
      fontSize: '13px',
      fontWeight: 'bold',
      marginBottom: '5px',
    };

    const attrs = [];
    if (selectedItem.attrs) {
      for (const attrKey in selectedItem.attrs) {
        let attrValue = selectedItem.attrs[attrKey];
        if (typeof attrValue === "number") {
          attrValue = attrValue < 0 ? '-' + String(attrValue) : '+' + String(attrValue);
        } else {
          attrValue = String(attrValue);
        }
        attrs.push(<div key={attrKey}>{attrValue} {attrKey}</div>);
      }
    }

    return (
      <div style={detailStyle}>
        <div style={titleStyle}>{selectedItem.name}</div>
        <div>{selectedItem.slot}</div>
        {attrs}
      </div>
    );
  }

  render() {
    const isVillager = this.isVillager();
    const items = this.props.equipData.items || [];
    const selectedItem = this.selectedItem();
    const slots = ['Helm', 'Shoulder', 'Chest', 'Pants', 'Boots', 'Bracers', 'Main Hand', 'Off Hand'];
    const equippedBySlot = {};
    items.forEach(item => {
      if (item.equipped) equippedBySlot[item.slot] = item;
    });

    const inventoryItems = items.filter(item =>
      !item.equipped &&
      (item.class == "Weapon" || item.class == "Armor" || item.class == "Tool" || item.class == "Torch")
    );

    const sectionTitleStyle: React.CSSProperties = {
      color: '#c9aa71',
      fontSize: '11px',
      fontWeight: 'bold',
      textTransform: 'uppercase',
      margin: '12px 0 8px',
    };

    const slotsStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: 'repeat(2, minmax(0, 1fr))',
      gap: '8px',
    };

    const footerStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: isVillager ? '1fr' : '1fr 1fr',
      gap: '8px',
    };

    const footerButtonStyle: React.CSSProperties = {
      minHeight: '44px',
      border: '1px solid rgba(201, 170, 113, 0.55)',
      borderRadius: '4px',
      background: selectedItem ? '#25282b' : 'rgba(37, 40, 43, 0.5)',
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '12px',
      opacity: selectedItem ? 1 : 0.45,
    };

    return (
      <MobilePanelScreen
        panelType="equip"
        title="Equipment"
        footer={
          <div style={footerStyle}>
            {!isVillager &&
              <button type="button" style={footerButtonStyle} disabled={!selectedItem} onClick={this.handleEquipClick}>
                {selectedItem && selectedItem.equipped ? 'Unequip' : 'Equip'}
              </button>}
            <button type="button" style={footerButtonStyle} disabled={!selectedItem} onClick={this.handleItemInfoClick}>
              Info
            </button>
          </div>
        }
      >
        <div style={sectionTitleStyle}>Equipped</div>
        <div style={slotsStyle}>
          {slots.map(slot => this.renderSlot(slot, equippedBySlot[slot]))}
        </div>

        <div style={sectionTitleStyle}>Inventory</div>
        <MobileInventoryGrid
          ownerId={this.props.equipData.id}
          items={inventoryItems}
          selectedItemId={this.state.selectedItemId}
          onSelect={this.handleSelect}
        />

        {this.renderSelectedDetails(selectedItem)}
      </MobilePanelScreen>
    );
  }
}
