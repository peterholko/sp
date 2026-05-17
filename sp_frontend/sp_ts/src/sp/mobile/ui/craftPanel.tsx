import * as React from "react";
import 'overlayscrollbars/overlayscrollbars.css';
import { OverlayScrollbarsComponent } from 'overlayscrollbars-react';

import { Global } from "../../core/global";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import craftbutton from "ui_comp/craftbutton.png";
import cancelbutton from "ui_comp/exitbutton.png";
import wideframe from "ui_comp/wide_frame2.png";

import { Network } from "../../core/network";
import { GameEvent } from "../../core/gameEvent";
import { WEAPON } from "../../core/config";

import ResourceItem from "./resourceItem";
import BaseInventoryPanel from "./baseInventoryPanel";
import SmallButton from "./smallButton";
import { NetworkEvent } from "../../core/networkEvent";
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

interface CraftPanelProps {
  crafterId,
  structureId,
  items,
  recipesData,
  craftingItem,
}

export default class CraftPanel extends React.Component<CraftPanelProps, any> {
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



    this.handleLeftClick = this.handleLeftClick.bind(this);
    this.handleRightClick = this.handleRightClick.bind(this);
    this.handleCraftClick = this.handleCraftClick.bind(this);
    this.handleSelect = this.handleSelect.bind(this);
    this.handleInventorySwitch = this.handleInventorySwitch.bind(this);
    this.handleCraft = this.handleCraft.bind(this);
    this.handleInfoCraft = this.handleInfoCraft.bind(this);
    this.handleCancelClick = this.handleCancelClick.bind(this);

    this.startTimer = this.startTimer.bind(this)
    this.stopTimer = this.stopTimer.bind(this)

    Global.gameEmitter.on(NetworkEvent.INFO_CRAFT, this.handleInfoCraft, this);
    Global.gameEmitter.on(NetworkEvent.CRAFT, this.handleCraft, this);
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
    Global.gameEmitter.removeListener(NetworkEvent.INFO_CRAFT, this.handleInfoCraft);
    Global.gameEmitter.removeListener(NetworkEvent.CRAFT, this.handleCraft);
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

  handleSelect(eventData) {
    console.log('handleSelect ' + JSON.stringify(eventData));
    Global.infoItemAction = 'craft';
    Global.network.sendInfoItem(eventData.itemId, "None");
  }

  handleInventorySwitch() {
    Global.network.sendInfoItemTransfer(Global.heroId, this.props.structureId);
  }

  handleCraftClick() {
    Global.network.sendCraft(this.state.recipe.name);
    //Global.gameEmitter.emit(GameEvent.CRAFT_CLICK, {});
  }

  handleCraft(eventData) {
    console.log('handleCraft ' + JSON.stringify(eventData));
    this.setState({
      maxProgress: eventData.craft_time,
    });

    this.startTimer();
  }

  handleCancelClick() {
    Global.network.sendCancelAction();

    this.setState({
      progress: -1,
      maxProgress: -1,
    });

    this.stopTimer();
  }

  handleInfoCraft(message) {

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


    var inventoryItems = this.props.items;
    var inventoryOwner;

    if (this.props.structureId) {
      inventoryOwner = this.props.structureId;
    } else {
      inventoryOwner = this.props.crafterId;
    }

    if (this.props.craftingItem) {
      craftingItemName = this.props.craftingItem.name;
      craftingItemImage = this.props.craftingItem.image;
    } else {
      craftingItemName = this.state.recipe.name;
      craftingItemImage = this.state.recipe.image;
    }

    var showCraftingItemPanel = this.state.progress > -1;

    console.log("Render Craft Panel maxProgress: " + this.state.maxProgress);

    const handleMobileSelect = (eventData) => {
      Global.selectedItemOwnerId = eventData.ownerId;
      Global.selectedItemId = eventData.itemId;
      Global.selectedItemName = eventData.itemName;
      this.handleSelect(eventData);
    };

    const recipeHeaderStyle: React.CSSProperties = {
      display: 'flex',
      alignItems: 'center',
      gap: '10px',
      marginBottom: '10px',
    };

    const recipeImageStyle: React.CSSProperties = {
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
        <div style={recipeHeaderStyle}>
          <img src={'/static/art/items/' + craftingItemImage + '.png'} style={recipeImageStyle} />
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
        <div style={headingStyle}>Inventory</div>
        <div style={{ marginTop: '8px' }}>
          <MobileInventoryGrid
            ownerId={inventoryOwner}
            items={(inventoryItems || []).filter((item) => item.equipped == false)}
            onSelect={handleMobileSelect}
            compact
          />
        </div>
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
      <MobilePanelScreen panelType="craft" title="Craft">
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
