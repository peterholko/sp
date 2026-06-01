
import * as React from "react";
import styles from "./../ui.module.css";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";

interface ResItemProps {
  resourceName,
  resourceImage,
  yieldLabel?,
  quantityLabel?,
  quantity,
  currentQuantity?,
  index,
  showQuantity,
  properties?,
  fixedPos?,
  xPos?,
  yPos?,
  spaceDistance?,
  addHeight?,
  insufficient?
}

export default class ResourceItem extends React.Component<ResItemProps, any> {
  constructor(props) {
    super(props);

    this.handleClick = this.handleClick.bind(this)
  }

  handleClick = () => {
    const eventData = {
      name: this.props.resourceName,
      image: this.props.resourceImage,
      yieldLabel: this.props.yieldLabel,
      quantityLabel: this.props.quantityLabel,
      properties: this.props.properties,
      index: this.props.index,
      xPos: this.props.xPos,
      yPos: this.props.yPos
    }

    Global.gameEmitter.emit(GameEvent.RESOURCE_CLICK, eventData);
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
    
    var spaceDistance = 55;

    if(this.props.spaceDistance) {
      spaceDistance = this.props.spaceDistance;
    }
    
    var xPos = this.props.index * spaceDistance;
    var yPos = 0;
    var formattedQuantity = this.formatQuantity(this.props.quantity);
    var quantityStr = formattedQuantity;
    let fixedPos = this.props.fixedPos != null ? 'static' : 'fixed';

    if(this.props.currentQuantity != null) {
      var currentQuantity = this.formatQuantity(this.props.currentQuantity)
      quantityStr = currentQuantity + '/' + formattedQuantity;
    }

    if(this.props.xPos != null) {
      xPos = this.props.xPos;
      yPos = this.props.yPos;
    }
    

    //31px -286px
    const divStyle = {    
      transform: 'translate(' + xPos + 'px,  ' + yPos + 'px)',
      position: fixedPos
    } as React.CSSProperties

    if (this.props.addHeight) {
      divStyle.height = '50px';
    }

    const itemStyle = {
      transform: 'translate(0px, 0px)',
      position: 'fixed'
    } as React.CSSProperties

    //const imageName = this.props.resourceName.replace(/\s/g, '').toLowerCase();

    return (
      <div style={divStyle} onClick={this.handleClick}>
        <img src={'/static/art/items/' + this.props.resourceImage + '.png'}
            style={itemStyle} />
        {this.props.showQuantity &&
          <span id="itemquantity" className={styles.itemquantity}
                style={this.props.insufficient ? { color: '#ff6b6b' } : undefined}>{quantityStr}</span>}
      </div>
    );
  }
}

