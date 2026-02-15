import * as React from "react";
import 'overlayscrollbars/overlayscrollbars.css';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';

import HalfPanel from "./halfPanel";
import { Global } from "../global";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import craftbutton from "ui_comp/craftbutton.png";
import addqueuebutton from "ui_comp/addqueuebutton.png";
import cancelbutton from "ui_comp/exitbutton.png";
import { NetworkEvent } from "../networkEvent";
import { GameEvent } from "../gameEvent";
import { TRIGGER_STRUCTURE_CRAFTING_ITEM, WEAPON } from "../config";
import unitframe from "ui_comp/itemframe.png";
import wideframe from "ui_comp/wide_frame2.png";

import ResourceItem from "./resourceItem";
import BaseInventoryPanel from "./baseInventoryPanel";
import SmallButton from "./smallButton";

interface StructureCraftPanelProp {
  structureId,
  structureInventory,
  recipesData,
  craftingItem,
}

export default class StructureCraftPanel extends React.Component<StructureCraftPanelProp, any> {
  private timer;

  constructor(props) {
    super(props);

    var maxProgress = -1;
    var progress = -1;

    if (this.props.craftingItem && this.props.craftingItem.progress) {
      console.log('Craft Panel Constructor Crafting Item ' + JSON.stringify(this.props.craftingItem));
      maxProgress = this.props.craftingItem.crafting_time;
      progress = this.props.craftingItem.progress;
    }

    this.state = {
      recipe: this.props.recipesData[0],
      index: 0,
      maxProgress: maxProgress,
      progress: progress,
    };

    this.handleSelect = this.handleSelect.bind(this);

    this.handleLeftClick = this.handleLeftClick.bind(this);
    this.handleRightClick = this.handleRightClick.bind(this);
    this.handleCraftClick = this.handleCraftClick.bind(this);
    this.handleCraftQueueClick = this.handleCraftQueueClick.bind(this);

    this.startTimer = this.startTimer.bind(this)
    this.stopTimer = this.stopTimer.bind(this)

    Global.gameEmitter.on(NetworkEvent.CRAFT, this.handleCraft, this);
    Global.gameEmitter.on(NetworkEvent.INFO_STRUCTURE_CRAFT, this.handleInfoStructureCraft, this);
  
  }

  componentDidMount() {
    console.log('componentDidMount craft panel ' + JSON.stringify(this.props.craftingItem));
    if (this.state.progress > -1) {
      this.startTimer();
    }
  }

  componentWillUnmount() {
    console.log('******* componentWillUnmount craft panel');
    if (this.timer) {
      console.log('Stop Timer Craft Panel');
      clearInterval(this.timer);
      this.timer = null;
    }
    Global.gameEmitter.removeListener(NetworkEvent.CRAFT, this.handleCraft);
    Global.gameEmitter.removeListener(NetworkEvent.INFO_STRUCTURE_CRAFT, this.handleInfoStructureCraft);
  }

  handleCraft(eventData) {
    console.log('handleCraft ' + JSON.stringify(eventData));
    this.setState({
      maxProgress: eventData.craft_time,
    });

    this.startTimer();
  }

  handleInfoStructureCraft(message) {

    if (message.crafting_item && message.crafting_item.progress == 0) {
      this.setState({
        progress: 0,
      });
    }

    if (message.crafting_item == null) {
      this.stopTimer();

      this.setState({
        progress: -1,
        maxProgress: -1,
      });
    }
  }

  handleSelect(eventData) {
    console.log('handleSelect ' + JSON.stringify(eventData));
    Global.infoItemAction = TRIGGER_STRUCTURE_CRAFTING_ITEM;
    Global.network.sendInfoItem(eventData.itemId, "None");
  }

  handleLeftClick(event) {
    if (this.state.index != 0) {
      const newIndex = this.state.index - 1;
      this.setState({
        recipe: this.props.recipesData[newIndex],
        index: newIndex
      })
    }
  }

  handleRightClick(event) {
    if (this.state.index != (this.props.recipesData.length - 1)) {
      const newIndex = this.state.index + 1;
      this.setState({
        recipe: this.props.recipesData[newIndex],
        index: newIndex
      })
    }
  }

  handleCraftClick() {
    Global.network.sendStructureCraft(this.props.structureId, this.state.recipe.name);
  }

  handleCraftQueueClick() {
    Global.network.sendAddCraftingEntry(this.props.structureId, this.state.recipe.name);
  }

  handleCancelClick() {
    Global.network.sendCancelAction();

    this.setState({
      progress: -1,
      maxProgress: -1,
    });

    this.stopTimer();
  }

  startTimer() {
    console.log('Start Timer Craft Panel');
    this.timer = setInterval(() => {
      console.log('Internal Timer Function Execution Craft Panel');
      console.log("progress: " + this.state.progress);
      console.log("maxProgress: " + this.state.maxProgress);

      if (this.state.progress >= this.state.maxProgress) {
        console.log('progress >>> maxProgress');
        this.stopTimer();
      } else {
        this.setState({ progress: this.state.progress + 1 });
      }
    }, 1000);
  }

  stopTimer() {
    console.log('Stop Timer Craft Panel');
    clearInterval(this.timer)
    this.timer = null;
  }

  render() {
    var imageName = this.state.recipe.image + '.png';

    const windowHeight = window.innerHeight;
    const isLargeWindow = windowHeight > 700;

    const infoSmallY = '0px';
    const infoLargeY = '260px';

    const zIndex = Global.zIndexManager.getTop() + 1;

    var craftingItemName = '';
    var craftingItemImage = '';

    if (this.props.craftingItem) {
      craftingItemName = this.props.craftingItem.name;
      craftingItemImage = this.props.craftingItem.image;
    } else {
      craftingItemName = this.state.recipe.name;
      craftingItemImage = this.state.recipe.image;
    }

    var showCraftingItemPanel = this.state.progress > -1;

    const reqs = [];

    for (var i = 0; i < this.state.recipe.req.length; i++) {
      var req = this.state.recipe.req[i];
      var resourceImage = req.type.toLowerCase().replace(/\s/g, '');

      var addHeight = i == this.state.recipe.req.length - 1;

      reqs.push(
        <ResourceItem key={i}
          resourceName={req.type}
          resourceImage={resourceImage}
          quantity={req.quantity}
          index={i}
          showQuantity={true}
          fixedPos={true}
          addHeight={addHeight} />
      );
    }



    const imageStyle = {
      transform: 'translate(-187px, 35px)',
      position: 'fixed'
    } as React.CSSProperties

    const spanNameStyle = {
      transform: 'translate(-323px, 100px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    const tableStyle = {
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const tableStyle2 = {
      transform: 'translate(-80px, 10px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const leftStyle = {
      transform: 'translate(15px, -215px)',
      position: 'fixed'
    } as React.CSSProperties

    const rightStyle = {
      transform: 'translate(259px, -215px)',
      position: 'fixed'
    } as React.CSSProperties

    const craftStyle = {
      transform: 'translate(110px, -215px)',
      position: 'fixed'
    } as React.CSSProperties

    const craftQueueStyle = {
      transform: 'translate(160px, -215px)',
      position: 'fixed'
    } as React.CSSProperties

    const simpleStyle = {
      transform: 'translate(20px, -230px)',
      width: '280px',
      height: '150px',
      maxHeight: '150px'
    } as React.CSSProperties

    const wideFrameStyle = {
      top: '50%',
      left: '50%',
      marginTop: isLargeWindow ? infoLargeY : infoSmallY,
      marginLeft: '-30px',
      position: 'fixed',
      transform: 'translate(-223px, -155px)',
      zIndex: zIndex + 1
    } as React.CSSProperties

    const craftingItemNameStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '0px',
      position: 'fixed',
      transform: 'translate(-150px, -95px)',
      zIndex: zIndex + 1,
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '300px'
    } as React.CSSProperties

    const craftingItemStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '0px',
      position: 'fixed',
      transform: 'translate(-24px, -60px)',
      zIndex: zIndex + 1,
    } as React.CSSProperties

    const craftingItemTableStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '0px',
      position: 'fixed',
      textAlign: 'left',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '200px',
      transform: 'translate(-135px, 0px)',
      zIndex: zIndex + 1,
      userSelect: 'none'
    } as React.CSSProperties

    const cancelButtonStyle = {
      top: '50%',
      left: '50%',
      marginTop: '-25px',
      marginLeft: '0px',
      position: 'fixed',
      transform: 'translate(-24px, 95px)',
      zIndex: zIndex + 1,
    } as React.CSSProperties
    return (
      <div>
        <BaseInventoryPanel left={true}
          id={this.props.structureId}
          items={this.props.structureInventory.items}
          capacity={this.props.structureInventory.cap}
          totalWeight={this.props.structureInventory.tw}
          panelType={'structure_craft'}
          hideExitButton={true}
          hideSelect={false}
          handleSelect={this.handleSelect} />

        <HalfPanel left={false}
          panelType={'structure_craft'}
          hideExitButton={false}>
          <img src={'/static/art/items/' + imageName} style={imageStyle} />
          <span style={spanNameStyle}>
            {this.state.recipe.name}
          </span>
          <OverlayScrollbarsComponent style={simpleStyle}>

            <table style={tableStyle}>
              <tbody>
                <tr>
                  <td>Class:</td>
                  <td>{this.state.recipe.class}</td>
                </tr>
                <tr>
                  <td>Subclass:</td>
                  <td>{this.state.recipe.subclass}</td>
                </tr>

                {this.state.recipe.slot &&
                  <tr>
                    <td>Slot: </td>
                    <td>{this.state.recipe.slot}</td>
                  </tr>
                }

                {this.state.recipe.damage &&
                  <tr>
                    <td>Damage:</td>
                    <td>{this.state.recipe.damage}</td>
                  </tr>
                }

                {this.state.recipe.speed &&
                  <tr>
                    <td>Speed:</td>
                    <td>{this.state.recipe.speed}</td>
                  </tr>
                }

                {this.state.recipe.skill_req &&
                  <tr>
                    <td>Skill Req:</td>
                    <td>{this.state.recipe.skill_req}</td>
                  </tr>
                }

                {this.state.recipe.stamina_req &&
                  <tr>
                    <td>Stamina Req:</td>
                    <td>{this.state.recipe.stamina_req}</td>
                  </tr>
                }

                {this.state.recipe.armor &&
                  <tr>
                    <td>Armor:</td>
                    <td>{this.state.recipe.armor}</td>
                  </tr>
                }

                <tr>
                  <td>Requirements:</td>
                  <td></td>
                </tr>
                <tr>
                  <td colSpan={2}>
                    {reqs}
                  </td>
                </tr>

              </tbody>
            </table>

          </OverlayScrollbarsComponent>
          <img src={leftbutton} style={leftStyle} onClick={this.handleLeftClick} />
          <img src={rightbutton} style={rightStyle} onClick={this.handleRightClick} />
          <img src={craftbutton} style={craftStyle} onClick={this.handleCraftClick} />
          <img src={addqueuebutton} style={craftQueueStyle} onClick={this.handleCraftQueueClick} />
        </HalfPanel>

        {showCraftingItemPanel &&
          <div style={wideFrameStyle}>
            <img src={wideframe} />

            <div>

              <span style={craftingItemNameStyle}>{craftingItemName}</span>

              <img src={'/static/art/items/' + craftingItemImage + '.png'} style={craftingItemStyle} />

              <table style={craftingItemTableStyle}>
                <tbody>
                  <tr>
                    <td>Crafting Progress: </td>
                    <td><progress max={this.state.maxProgress} value={this.state.progress}>{this.state.progress}</progress></td>
                  </tr>
                </tbody>
              </table>
              <img src={cancelbutton}
                style={cancelButtonStyle}
                onClick={this.handleCancelClick} />
            </div>
          </div>
        }
      </div>
    );
  }
}
