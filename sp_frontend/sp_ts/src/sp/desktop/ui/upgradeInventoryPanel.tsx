import * as React from "react";
import HalfPanel from "./halfPanel";
import InventoryItem from "./inventoryItem";
import { Global } from "../../core/global";

import itemframe from "ui_comp/itemframe.png";
import selectitemborder from "ui_comp/selectitemborder.png";
import { Util } from "../../core/util";
import ResourceItem from "./resourceItem";
import { STRUCTURE, FOUNDED } from "../../core/config";
import SmallButton from "./smallButton";
import { Network } from "../../core/network";
import { GameEvent } from "../../core/gameEvent";

interface UpgradeInventoryProps {
  id: integer,
  items: any,
  reqs: any,
  panelType: string,
  hideExitButton: boolean,
  hideSelect: boolean,
  handleSelect: Function,
}

export default class UpgradeInventoryPanel extends React.Component<UpgradeInventoryProps, any> {
  constructor(props) {
    super(props);

    const selectItemStyle = {
      position: "fixed"
    } as React.CSSProperties

    this.state = {
      selectItemStyle: selectItemStyle
    };

    this.handleSelect = this.handleSelect.bind(this);
    this.handleUpgradeClick = this.handleUpgradeClick.bind(this);
  }

  handleSelect(eventData) {
    console.log('handleSelect ' + eventData);
    var xPos = -293 + ((eventData.index % 5) * 53);
    var yPos = 232 + (Math.floor(eventData.index / 5) * 53);

    const selectItemStyle = {
      transform: 'translate(' + xPos + 'px, ' + yPos + 'px)',
      position: 'fixed'
    }

    Global.selectedItemOwnerId = eventData.ownerId;
    Global.selectedItemId = eventData.itemId;

    this.setState({ selectItemStyle: selectItemStyle });

    this.props.handleSelect(eventData);
  }

  handleUpgradeClick() {
    Global.network.sendUpgrade(Global.heroId, this.props.id);
    Global.gameEmitter.emit(GameEvent.START_UPGRADE_CLICK, {});
  }

  render() {
    const objId = this.props.id;
    const itemFrames = []

    const items = []
    const reqs = []

    var showUpgradeButton = true;

    const spriteStyle = {
      transform: 'translate(-200px, 5px)',
      position: 'fixed'
    } as React.CSSProperties

    const reqStyle = {
      transform: 'translate(-295px, 75px)',
      position: 'fixed',
      textAlign: 'left',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '295px'
    } as React.CSSProperties

    const materialStyle = {
      transform: 'translate(-295px, 210px)',
      position: 'fixed',
      textAlign: 'left',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '295px'
    } as React.CSSProperties

    const upgradeStyle = {
      transform: 'translate(-187px, 155px)',
      position: 'fixed',
      zIndex: 6
    } as React.CSSProperties


    if (Util.isSprite(Global.objectStates[objId].image)) {
      var imageName = Global.objectStates[objId].image + '_single.png';
    } else {
      var imageName = Global.objectStates[objId].image + '.png';
    }

    for (var i = 0; i < this.props.reqs.length; i++) {
      var xPos = 25 + ((i % 5) * 53);
      var yPos = -265 + (Math.floor(i / 5) * 53);

      if (this.props.reqs[i].cquantity != 0) {
        showUpgradeButton = false;
      }

      var resourceImage = this.props.reqs[i].type.toLowerCase().replace(/\s/g, '');

      reqs.push(
        <ResourceItem key={i}
          index={i}
          resourceName={this.props.reqs[i].type}
          resourceImage={resourceImage}
          quantity={this.props.reqs[i].quantity}
          currentQuantity={this.props.reqs[i].cquantity}
          showQuantity={true}
          xPos={xPos}
          yPos={yPos} />
      )
    }

    for (var i = 0; i < 10; i++) {
      var xPos = -293 + ((i % 5) * 53);
      var yPos = 232 + (Math.floor(i / 5) * 53);

      var itemFrameStyle = {
        transform: 'translate(' + xPos + 'px, ' + yPos + 'px)',
        position: 'fixed'
      } as React.CSSProperties

      itemFrames.push(<img src={itemframe} key={i} style={itemFrameStyle} />)
    }

    for (var i = 0; i < this.props.items.length; i++) {
      console.log('Item: ' + this.props.items[i]);
      var itemId = this.props.items[i].id;
      var itemName = this.props.items[i].name;
      var image = this.props.items[i].image;
      var quantity = this.props.items[i].quantity;

      var xPos = 31 + ((i % 5) * 53);
      var yPos = -127 + (Math.floor(i / 5) * 53);

      items.push(<InventoryItem key={i}
        ownerId={objId}
        itemId={itemId}
        itemName={itemName}
        image={image}
        quantity={quantity}
        index={i}
        xPos={xPos}
        yPos={yPos}
        handleSelect={this.handleSelect} />);
    }

    return (
      <HalfPanel left={false}
        panelType={this.props.panelType}
        hideExitButton={this.props.hideExitButton}>
        <img src={'/static/art/' + imageName} style={spriteStyle} />
        <span style={reqStyle}>Requirements:</span>
        {reqs}
        <span style={materialStyle}>Materials:</span>
        {itemFrames}
        {items}
        {!this.props.hideSelect &&
          <img src={selectitemborder} style={this.state.selectItemStyle} />
        }
        {showUpgradeButton &&
          <SmallButton handler={this.handleUpgradeClick}
            imageName="upgradebutton"
            style={upgradeStyle} />}
      </HalfPanel>
    );
  }
}

