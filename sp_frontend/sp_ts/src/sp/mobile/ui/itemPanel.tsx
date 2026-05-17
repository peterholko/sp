
import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import dividebutton from "ui_comp/dividebutton.png";
import buybutton from "ui_comp/buybutton.png";
import sellbutton from "ui_comp/sellbutton.png";
import refineorebutton from "ui_comp/refinebutton.png";
import refinewoodbutton from "ui_comp/refinewoodbutton.png";
import refinestonebutton from "ui_comp/refinestonebutton.png";
import refinegameanimalbutton from "ui_comp/refinegameanimalbutton.png";
import deletebutton from "ui_comp/deletebutton.png";
import usebutton from "ui_comp/usebutton.png";

import { GameEvent } from "../../core/gameEvent";
import { Global } from "../../core/global";
import { TRIGGER_PLAYER_SELLING_ITEM, TRIGGER_INVENTORY, TRIGGER_PLAYER_BUYING_ITEM, TRIGGER_REFINING_ITEM, TRIGGER_STRUCTURE_REFINING_ITEM } from "../../core/config";
import {
  MobileCard,
  MobilePanelActions,
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

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

        attrs.push({ key: attrKey, value: attrValue });
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

        effects.push(value);
      }
    }

    if (this.props.itemData.hasOwnProperty('produces')) {
      hasProduces = true;


      for (var i = 0; i < this.props.itemData.produces.length; i++) {
        produces.push(this.props.itemData.produces[i]);
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
    const actionButtons = [];

    if (showDivideButton) {
      actionButtons.push({ key: 'divide', label: 'Divide', icon: dividebutton, onClick: this.handleDivideClick });
    }

    if (showBuyButton) {
      actionButtons.push({ key: 'buy', label: 'Buy', icon: buybutton, onClick: this.handleBuyClick });
    }

    if (showSellButton) {
      actionButtons.push({ key: 'sell', label: 'Sell', icon: sellbutton, onClick: this.handleSellClick });
    }

    if (showUseButton) {
      actionButtons.push({ key: 'use', label: 'Use', icon: usebutton, onClick: this.handleUseClick });
    }

    if (hasProduces) {
      actionButtons.push({ key: 'refine', label: 'Refine', icon: refineItemIcon, onClick: this.handleRefineClick });
    }

    if (showDeleteButton) {
      actionButtons.push({ key: 'delete', label: 'Delete', icon: deletebutton, onClick: this.handleDeleteClick });
    }

    const landscape = isLandscapeMobile();
    const weightTotal = this.props.itemData.quantity * this.props.itemData.weight;

    const coreRows = [
      { label: 'Class', value: this.props.itemData.class },
      { label: 'Subclass', value: this.props.itemData.subclass, hidden: this.props.itemData.subclass == null },
      { label: 'Quantity', value: this.props.itemData.quantity },
      { label: 'Weight', value: `${this.props.itemData.weight} each / ${weightTotal} total` },
      { label: 'Equipped', value: this.props.itemData.equipped ? 'Yes' : 'No', hidden: !hasEquipable },
      { label: 'Durability', value: this.props.itemData.durability, hidden: !hasDurability },
      { label: 'Price', value: this.props.itemData.price, hidden: !hasPrice },
    ];

    const sectionTitleStyle: React.CSSProperties = {
      color: '#c9aa71',
      fontFamily: 'Verdana',
      fontSize: '11px',
      fontWeight: 'bold',
      textTransform: 'uppercase',
      marginBottom: '7px',
    };

    const chipGridStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: landscape
        ? 'repeat(auto-fit, minmax(112px, 1fr))'
        : 'repeat(auto-fit, minmax(128px, 1fr))',
      gap: '6px',
    };

    const chipStyle: React.CSSProperties = {
      border: '1px solid rgba(201, 170, 113, 0.24)',
      borderRadius: '4px',
      background: 'rgba(255,255,255,0.05)',
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '11px',
      lineHeight: 1.2,
      padding: '6px 7px',
      overflowWrap: 'anywhere',
    };

    const emptyStyle: React.CSSProperties = {
      color: '#777d82',
      fontFamily: 'Verdana',
      fontSize: '11px',
      lineHeight: 1.35,
    };

    const attrsCard = attrs.length > 0 ? (
      <MobileCard compact>
        <div style={sectionTitleStyle}>Properties</div>
        <div style={chipGridStyle}>
          {attrs.map(attr =>
            <div key={attr.key} style={chipStyle}>{attr.key}: {attr.value}</div>
          )}
        </div>
      </MobileCard>
    ) : null;

    const effectsCard = hasEffects ? (
      <MobileCard compact>
        <div style={sectionTitleStyle}>Effects</div>
        <div style={chipGridStyle}>
          {effects.map((effect, index) =>
            <div key={index} style={chipStyle}>{effect}</div>
          )}
        </div>
      </MobileCard>
    ) : null;

    const producesCard = hasProduces ? (
      <MobileCard compact>
        <div style={sectionTitleStyle}>Produces</div>
        <div style={chipGridStyle}>
          {produces.map((produce, index) =>
            <div key={index} style={chipStyle}>{produce}</div>
          )}
        </div>
      </MobileCard>
    ) : null;

    const actionsCard = actionButtons.length > 0 ? (
      <MobileCard compact>
        <div style={sectionTitleStyle}>Actions</div>
        <MobilePanelActions actions={actionButtons} compact align={landscape ? 'start' : 'center'} />
      </MobileCard>
    ) : null;

    const secondaryInfo = attrsCard || effectsCard || producesCard ? (
      <React.Fragment>
        {attrsCard}
        {effectsCard}
        {producesCard}
      </React.Fragment>
    ) : (
      <MobileCard compact>
        <div style={sectionTitleStyle}>Details</div>
        <div style={emptyStyle}>No special properties.</div>
      </MobileCard>
    );

    return (
      <MobilePanelScreen
        panelType={'item'}
        title={'Item'}
        hideExitButton={false}
        zIndexBonus={topLevel ? 3 : 0}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={
            <React.Fragment>
              <MobileSummaryCard
                imageSrc={'/static/art/items/' + imageName}
                title={itemName}
                subtitle={`Quantity ${this.props.itemData.quantity}`}
                imageSize={48}
              />
              <MobileStatsList rows={coreRows} compact />
              {actionsCard}
            </React.Fragment>
          }
          right={secondaryInfo}
        />
      </MobilePanelScreen>
    );
  }
}
