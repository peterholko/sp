import * as React from "react";
import 'overlayscrollbars/overlayscrollbars.css';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';

import { Global } from "../../core/global";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import craftbutton from "ui_comp/craftbutton.png";
import addqueuebutton from "ui_comp/addqueuebutton.png";
import cancelbutton from "ui_comp/exitbutton.png";
import { NetworkEvent } from "../../core/networkEvent";
import { GameEvent } from "../../core/gameEvent";
import { TRIGGER_STRUCTURE_CRAFTING_ITEM, WEAPON } from "../../core/config";
import unitframe from "ui_comp/itemframe.png";
import wideframe from "ui_comp/wide_frame2.png";

import ResourceItem from "./resourceItem";
import BaseInventoryPanel from "./baseInventoryPanel";
import SmallButton from "./smallButton";
import MobilePanelScreen from "./mobilePanelScreen";
import MobileInventoryGrid from "./mobileInventoryGrid";
import {
  MobileCard,
  MobilePanelActions,
  MobileRequirementGrid,
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
} from "./mobilePanelLayout";

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

    const handleMobileSelect = (eventData) => {
      Global.selectedItemOwnerId = eventData.ownerId;
      Global.selectedItemId = eventData.itemId;
      Global.selectedItemName = eventData.itemName;
      this.handleSelect(eventData);
    };

    const headerStyle: React.CSSProperties = {
      display: 'flex',
      alignItems: 'center',
      gap: '10px',
      marginBottom: '10px',
    };

    const imageMobileStyle: React.CSSProperties = {
      width: '48px',
      height: '48px',
      objectFit: 'contain',
      imageRendering: 'pixelated',
      flex: '0 0 auto',
    };

    const headingStyle: React.CSSProperties = {
      color: '#c9aa71',
      fontFamily: 'Cinzel, Verdana, serif',
      fontSize: '15px',
      fontWeight: 'bold',
      lineHeight: 1.2,
    };

    const recipeStats = [
      { label: 'Class', value: this.state.recipe.class },
      { label: 'Subclass', value: this.state.recipe.subclass },
      { label: 'Slot', value: this.state.recipe.slot, hidden: !this.state.recipe.slot },
      { label: 'Damage', value: this.state.recipe.damage, hidden: !this.state.recipe.damage },
      { label: 'Speed', value: this.state.recipe.speed, hidden: !this.state.recipe.speed },
      { label: 'Skill Req', value: this.state.recipe.skill_req, hidden: !this.state.recipe.skill_req },
      { label: 'Stamina Req', value: this.state.recipe.stamina_req, hidden: !this.state.recipe.stamina_req },
      { label: 'Armor', value: this.state.recipe.armor, hidden: !this.state.recipe.armor },
    ];

    const progressCard = showCraftingItemPanel ? (
      <MobileCard compact>
        <div style={headerStyle}>
          <img src={'/static/art/items/' + craftingItemImage + '.png'} style={imageMobileStyle} />
          <div style={headingStyle}>{craftingItemName}</div>
        </div>
        <progress style={{ width: '100%' }} max={this.state.maxProgress} value={this.state.progress}>{this.state.progress}</progress>
        <MobilePanelActions
          compact
          actions={[{
            key: 'cancel',
            label: 'Cancel',
            icon: cancelbutton,
            onClick: this.handleCancelClick,
          }]}
        />
      </MobileCard>
    ) : null;

    const inventoryCard = (
      <MobileCard compact>
        <div style={headingStyle}>Structure Inventory</div>
        <div style={{ color: '#9aa0a6', fontSize: '11px', marginTop: '2px', marginBottom: '8px' }}>
          {this.props.structureInventory.tw}/{this.props.structureInventory.cap} lbs
        </div>
        <MobileInventoryGrid
          ownerId={this.props.structureId}
          items={(this.props.structureInventory.items || []).filter((item) => item.equipped == false)}
          onSelect={handleMobileSelect}
          compact
        />
      </MobileCard>
    );

    const recipeActions = (
      <MobilePanelActions
        compact
        actions={[
          {
            key: 'prev',
            label: 'Previous',
            icon: leftbutton,
            onClick: this.handleLeftClick,
            disabled: this.state.index == 0,
          },
          {
            key: 'craft',
            label: 'Craft',
            icon: craftbutton,
            onClick: this.handleCraftClick,
          },
          {
            key: 'queue',
            label: 'Queue',
            icon: addqueuebutton,
            onClick: this.handleCraftQueueClick,
          },
          {
            key: 'next',
            label: 'Next',
            icon: rightbutton,
            onClick: this.handleRightClick,
            disabled: this.state.index == this.props.recipesData.length - 1,
          },
        ]}
      />
    );

    return (
      <MobilePanelScreen panelType="structure_craft" title="Structure Craft">
        <MobileSplitPanelLayout
          left={
            <React.Fragment>
              {progressCard}
              {inventoryCard}
            </React.Fragment>
          }
          right={
            <React.Fragment>
              <MobileSummaryCard
                imageSrc={'/static/art/items/' + imageName}
                title={this.state.recipe.name}
                subtitle={`Recipe ${this.state.index + 1} of ${this.props.recipesData.length}`}
              />
              <MobileStatsList rows={recipeStats} compact />
              <MobileRequirementGrid title="Materials" requirements={this.state.recipe.req || []} />
              {recipeActions}
            </React.Fragment>
          }
        />
      </MobilePanelScreen>
    );
  }
}
