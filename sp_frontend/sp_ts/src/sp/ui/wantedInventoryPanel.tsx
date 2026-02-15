import * as React from "react";
import HalfPanel from "./halfPanel";
import WantedInventoryItem from "./wantedInventoryItem";
import { Global } from "../global";

import itemframe from "ui_comp/itemframe.png";
import selectitemborder from "ui_comp/selectitemborder.png";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";

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

    const selectItemStyle = {
      position: "fixed"
    } as React.CSSProperties

    this.state = {
      selectItemStyle: selectItemStyle,
      page: 0
    };

    this.handleSelect = this.handleSelect.bind(this)
    this.handleLeftClick = this.handleLeftClick.bind(this);
    this.handleRightClick = this.handleRightClick.bind(this);
  }

  handleSelect(eventData) {
    console.log('handleSelect ' + eventData);
    var xPos = -293 + ((eventData.index % 5) * 53);
    var yPos = 73 + (Math.floor(eventData.index / 5) * 53);

    const selectItemStyle = {
      transform: 'translate(' + xPos + 'px, ' + yPos + 'px)',
      position: 'fixed'
    }

    Global.wantedItemName = eventData.itemName;

    this.setState({ selectItemStyle: selectItemStyle });

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
    const itemFrames = []
    const items = []
    const reqs = []

    var imageName;
    var selectItemStyle = this.state.selectItemStyle;
    var hideLeftButton = false;
    var hideRightButton = false;

    var itemsData = this.props.items;

    if (Global.objectStates[objId]) {
      imageName = Global.objectStates[objId].image + '.png';
    } else {
      imageName = 'unknownunit.png';
    }

    for (var i = 0; i < 20; i++) {
      var xPos = -293 + ((i % 5) * 53);
      var yPos = 73 + (Math.floor(i / 5) * 53);

      var itemFrameStyle = {
        transform: 'translate(' + xPos + 'px, ' + yPos + 'px)',
        position: 'fixed'
      } as React.CSSProperties

      itemFrames.push(<img src={itemframe} key={i} style={itemFrameStyle} />)
    }

    var anyItemSelected = false;
    var maxItemIndex = (this.state.page + 1) * 20;

    if (maxItemIndex > (itemsData.length - 1)) {
      maxItemIndex = itemsData.length;
    }

    console.log("maxItemIndex: " + maxItemIndex);
    console.log("state page: " + this.state)

    var itemPageIndex = 0;

    for (var itemIndex = this.state.page * 20; itemIndex < maxItemIndex; itemIndex++) {
      console.log('Item: ' + JSON.stringify(itemsData[itemIndex]));
      var itemName;

      if(itemsData[itemIndex].name) {
        itemName = itemsData[itemIndex].name;
      } else if(itemsData[itemIndex].subclass) {
        itemName = itemsData[itemIndex].subclass;
      } else if(itemsData[itemIndex].class) {
        itemName = itemsData[itemIndex].class;
      }

      var image = itemName.toLowerCase().replace(/\s/g, '');
      var quantity = itemsData[itemIndex].quantity;
      var price = itemsData[itemIndex].price;

      var xPos = 31 + ((itemPageIndex % 5) * 53);
      var yPos = -286 + (Math.floor(itemPageIndex / 5) * 53);

      items.push(<WantedInventoryItem key={itemPageIndex}
        ownerId={objId}
        itemName={itemName}
        image={image}
        quantity={quantity}
        price={price}
        index={itemPageIndex}
        xPos={xPos}
        yPos={yPos}
        handleSelect={this.handleSelect} />);

      if (Global.wantedItemName == itemName) {
        var xPos = -293 + ((itemPageIndex % 5) * 53);
        var yPos = 73 + (Math.floor(itemPageIndex / 5) * 53);

        const style = {
          transform: 'translate(' + xPos + 'px, ' + yPos + 'px)',
          position: 'fixed'
        };

        selectItemStyle = style;
        anyItemSelected = true;
      }

      itemPageIndex++;
    }

    if (this.state.page == 0) {
      hideLeftButton = true;
    }

    if (itemsData.length == 0) {
      hideRightButton = true;
    } else if ((Math.ceil(itemsData.length / 20) - 1) == this.state.page) {
      hideRightButton = true;
    }

    const spriteStyle = {
      transform: 'translate(-290px, 5px)',
      position: 'fixed'
    } as React.CSSProperties

    const spanNameStyle = {
      transform: 'translate(-225px, 20px)',
      position: 'fixed',
      textAlign: 'left',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
    } as React.CSSProperties

    const leftStyle = {
      transform: 'translate(-305px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const rightStyle = {
      transform: 'translate(-65px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    return (
      <HalfPanel left={this.props.left}
        panelType={this.props.panelType}
        hideExitButton={this.props.hideExitButton}>
        <img src={'/static/art/' + imageName} style={spriteStyle} />
        <span style={spanNameStyle}>
          {Global.objectStates[objId].name}
        </span>

        {itemFrames}
        {items}
        {reqs}
        {anyItemSelected &&
          <img src={selectitemborder} style={selectItemStyle} />
        }
        {!hideLeftButton && <img src={leftbutton} style={leftStyle} onClick={this.handleLeftClick} />}
        {!hideRightButton && <img src={rightbutton} style={rightStyle} onClick={this.handleRightClick} />}
      </HalfPanel>
    );
  }
}

