
import * as React from "react";
import styles from "./../ui.module.css";
import { itemRarity, rarityBorderColor, rarityTooltip } from "../../core/itemRarity";

interface InvItemProps {
  ownerId,
  itemName,
  itemId,
  image,
  quantity,
  attrs?,
  xPos,
  yPos,
  index?,
  handleSelect?,
  disabled?,
  highlighted?
}

export default class InventoryItem extends React.Component<InvItemProps, any> {
  constructor(props) {
    super(props);

    this.handleClick = this.handleClick.bind(this)
  }

  handleClick = () => {
    const eventData = {
      ownerId: this.props.ownerId,
      itemId: this.props.itemId,
      itemName: this.props.itemName,
      index: this.props.index,
    }
    console.log('inventoryItem handleClick ' + JSON.stringify(eventData));
    this.props.handleSelect(eventData)
  }

  formatQuantity(quantity) {
    if(quantity > 1000000) {
      return (quantity / 1000000).toFixed(2) + 'M';
    } else if(quantity > 1000) {
      return (quantity / 1000).toFixed(2) + 'K';
    } else {
      return quantity;
    }
  }

  render() {
    var quantityStr = this.formatQuantity(this.props.quantity);

    //31px -286px
    const divStyle = {
      transform: 'translate(' + this.props.xPos + 'px, ' + this.props.yPos + 'px)',
      position: 'fixed'
    } as React.CSSProperties

    const itemStyle = {
      transform: 'translate(0px, 0px)',
      position: 'fixed'
    } as React.CSSProperties

    const rarity = itemRarity({ attrs: this.props.attrs });
    const borderColor = rarityBorderColor(rarity);
    const rarityFrameStyle = {
      width: '50px',
      height: '50px',
      position: 'relative',
      boxSizing: 'border-box',
      boxShadow: borderColor ? `inset 0 0 0 2px ${borderColor}` : 'none',
      cursor: (this.props.handleSelect != null && !this.props.disabled) ? 'pointer' : 'default',
    } as React.CSSProperties;


    return (
      <div style={divStyle}
        title={rarityTooltip({ name: this.props.itemName, attrs: this.props.attrs })}
        onClick={(this.props.handleSelect != null && !this.props.disabled) ? this.handleClick : null}>
        <div style={rarityFrameStyle}>
          <img src={'/static/art/items/' + this.props.image + '.png'}
              style={itemStyle}/>
          <span id="itemquantity" className={styles.itemquantity}>{quantityStr}</span>
          {this.props.disabled && <img src={'/static/art/ui/itemdisabled.png'} style={itemStyle} />}
          {this.props.highlighted &&
            <span className={styles.tutorialItemHighlight} aria-hidden="true" />}
        </div>
      </div>
    );
  }
}
