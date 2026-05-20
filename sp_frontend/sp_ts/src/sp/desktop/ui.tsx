
import { Network } from '../core/network';
import { Util } from '../core/util';
import { Global } from '../core/global';
import { ObjectState } from '../core/objectState';
import { GameEvent } from '../core/gameEvent';
import { Tile } from '../core/objects/tile';

import * as React from "react";
import styles from "./ui.module.css";

import SingleInventoryPanel from './ui/singleInventoryPanel';
import ItemPanel from './ui/itemPanel';

import explorebutton from "ui/explorebutton.png";


import movecompass from "ui/movecompass.png";
import movecompass_click from "ui/movecompass_click.png"

import bracebutton from "ui/bracebutton.png";
import parrybutton from "ui/parrybutton.png";
import dodgebutton from "ui/dodgebutton.png";

import { Obj } from '../core/obj';
import { NetworkEvent } from '../core/networkEvent';
import HeroDeathOverlay from '../core/heroDeathOverlay';
import {
  TRIGGER_INVENTORY,
  QUICK,
  PRECISE,
  FIERCE,
  OBJ,
  TILE
} from '../core/config';
import TargetActionPanel from './ui/targetActionPanel';
import ItemTransferPanel from './ui/itemTransferPanel';
import HeroPanel from './ui/heroPanel';
import VillagerPanel from './ui/villagerPanel';
import AttrsPanel from './ui/attrsPanel';
import EquipPanel from './ui/equipPanel';
import SkillsPanel from './ui/skillsPanel';
import TilePanel from './ui/tilePanel';
import TileResourcesPanel from './ui/tileResourcesPanel';
import TerrainFeaturePanel from './ui/terrainFeaturePanel';
import GatherPanel from './ui/gatherPanel';
import BuildPanel from './ui/buildPanel';
import StructurePanel from './ui/structurePanel';
import StructureUpgradePanel from './ui/structureUpgradePanel';
import ErrorPanel from './ui/errorPanel';
import ConfirmPanel from './ui/confirmPanel';
import SelectPanel from './ui/selectPanel';
import AssignPanel from './ui/assignPanel';
import StructureCraftPanel from './ui/structureCraftPanel';
import ItemDividePanel from './ui/itemDividePanel';
import MerchantPanel from './ui/merchantPanel';
import MerchantQuantityPanel from './ui/merchantQuantityPanel';
import ResourcePanel from './ui/resourcePanel';
import HeroFrame from './ui/heroFrame';
import MerchantHirePanel from './ui/merchantHirePanel';
import ExperimentPanel from './ui/experimentPanel';
import ActionButton from './ui/actionButton';
import NPCPanel from './ui/npcPanel';
import HeroAdvancePanel from './ui/heroAdvancePanel';
import NoticePanel from './ui/noticePanel';
import SmallButtonClassName from './ui/smallButtonClassName';
import AttacksPanel from './ui/attacksPanel';
import ToggleButton from './ui/toggleButton';
import CooldownButton from './ui/cooldownButton';
import GatherButton from './ui/gatherButton';
import WantedItemPanel from './ui/wantedItemPanel';
import WorkQueuePanel from './ui/workQueuePanel';
import WorkQueueEntryPanel from './ui/workQueueEntryPanel';
import ObjPanel from './ui/objPanel';
import WorldPanel from './ui/worldPanel';
import IntroPanel from './ui/introPanel';
import LoadingPanel from './ui/loadingPanel';
import TrueDeathPanel from './ui/trueDeathPanel';
import RefinePanel from './ui/refinePanel';
import StructureRefinePanel from './ui/structureRefinePanel';
import CraftPanel from './ui/craftPanel';
import ZoomButton from './ui/zoomButton';
import ObjectivesPanel from './ui/objectivesPanel';

interface UIState {
  selectBoxes: [],
  inventoryPanels: [],
  hideLoadingPanel: boolean,
  hideIntroPanel: boolean,
  hideSelectPanel: boolean,
  hideTargetActionPanel: boolean,
  hideAttacksPanel: boolean,
  hideGatherPanel: boolean,
  hideInventoryPanel: boolean,
  hideItemTransferPanel: boolean,
  hideItemDividePanel: boolean,
  hideItemPanel: boolean,
  hideHeroPanel: boolean,
  hideVillagerPanel: boolean,
  hideNPCPanel: boolean,
  hideObjPanel: boolean,
  hideAttrsPanel: boolean,
  hideEquipPanel: boolean,
  hideSkillsPanel: boolean,
  hideAdvancePanel: boolean,
  hideTilePanel: boolean,
  hideTileResourcesPanel: boolean,
  hideTerrainFeaturePanel: boolean,
  hideBuildPanel: boolean,
  hideStructurePanel: boolean,
  hideStructureUpgradePanel: boolean,
  hideErrorPanel: boolean,
  hideConfirmPanel: boolean,
  hideNoticePanel: boolean,
  hideAssignPanel: boolean,
  hideCraftPanel: boolean,
  hideStructureCraftPanel: boolean,
  hideWorkQueuePanel: boolean,
  hideWorkQueueEntryPanel: boolean,
  hideMerchantPanel: boolean,
  hideMerchantQuantityPanel: boolean,
  hideMerchantHirePanel: boolean,
  hideWantedItemPanel: boolean,
  hideResourcePanel: boolean,
  hideExperimentPanel: boolean,
  hideTrueDeathPanel: boolean,
  hideRefinePanel: boolean,
  hideStructureRefinePanel: boolean,
  leftInventoryId: integer,
  leftInventoryData: any,
  rightInventoryId: integer,
  rightInventoryData: any,
  inventoryData: any,
  structureInventoryData: any,
  inventoryReqs: [], //Currently used for structure inventory reqs
  itemData: any,
  heroDetailedData: any,
  npcData: any,
  objData: any,
  villagerData: any,
  assignData: any,
  attrsData: any,
  skillsData: any,
  advanceData: any,
  tileData: any,
  tileResourcesData: any,
  structuresData: any, //Structures list
  structureData: any, //Structure data
  structureUpgradeData: any
  recipesData: any,
  workQueueData: any,
  workQueueEntryData: any,
  crafter: any,
  itemDivideData: any,
  itemMerchantQuantityData: any,
  merchantWantedItems: any,
  wantedItemData: any,
  resourceData: any,
  hireData: any,
  expData: any,
  refineData: any,
  refineItemData: any,
  refineTime: number,
  producedItemData: any,
  craftData: any
  merchantAction: any,
  selectedTile: Tile,
  objIdsOnTile: any,
  createdObjOnTile: any
  selectedBoxPos: integer,
  selectedKey: any,
  infoItemAction: string,
  resourcesIconBorder: boolean,
  errmsg: string,
  noticemsg: string,
  noticeExpiry: integer,
  confirmMsg: string,
  confirmData: any,
  showMoveCompassClick: boolean
  activityData: any,
  needsData: any,
  worldData: any,
  showLeftInventoryPanel: boolean
  trueDeathData: any,
  heroDeathData: any,
  heroStats: any,
  hungerStatus: string,
  thirstStatus: string,
  fatigueStatus: string
  infoRefineItemTriggered: boolean,
  combatState: any,
  inCombatZoom: boolean,
}

export default class UI extends React.Component<any, UIState> {
  private compassRef = React.createRef<HTMLImageElement>();
  private heroDeathOverlayTimer: any = null;

  constructor(props) {
    super(props);

    this.state = {
      selectBoxes: [],
      inventoryPanels: [],
      hideLoadingPanel: false,
      hideIntroPanel: false,
      hideSelectPanel: true,
      hideTargetActionPanel: true,
      hideAttacksPanel: true,
      hideGatherPanel: true,
      hideInventoryPanel: true,
      hideItemTransferPanel: true,
      hideItemDividePanel: true,
      hideItemPanel: true,
      hideHeroPanel: true,
      hideVillagerPanel: true,
      hideNPCPanel: true,
      hideObjPanel: true,
      hideAttrsPanel: true,
      hideEquipPanel: true,
      hideSkillsPanel: true,
      hideAdvancePanel: true,
      hideTilePanel: true,
      hideTileResourcesPanel: true,
      hideTerrainFeaturePanel: true,
      hideBuildPanel: true,
      hideStructurePanel: true,
      hideStructureUpgradePanel: true,
      hideAssignPanel: true,
      hideCraftPanel: true,
      hideStructureCraftPanel: true,
      hideWorkQueuePanel: true,
      hideErrorPanel: true,
      hideNoticePanel: true,
      hideConfirmPanel: true,
      hideMerchantPanel: true,
      hideMerchantQuantityPanel: true,
      hideMerchantHirePanel: true,
      hideWantedItemPanel: true,
      hideResourcePanel: true,
      hideExperimentPanel: true,
      hideTrueDeathPanel: true,
      hideRefinePanel: true,
      hideStructureRefinePanel: true,
      hideWorkQueueEntryPanel: true,
      refineTime: 0,
      leftInventoryId: -1,
      leftInventoryData: [],
      rightInventoryId: -1,
      rightInventoryData: [],
      inventoryData: [],
      structureInventoryData: [],
      inventoryReqs: [],
      itemData: {},
      heroDetailedData: {},
      npcData: {},
      objData: {},
      villagerData: {},
      assignData: {},
      attrsData: {},
      skillsData: {},
      advanceData: {},
      tileData: {},
      tileResourcesData: {},
      structuresData: {},
      structureData: {},
      structureUpgradeData: {},
      recipesData: {},
      workQueueData: {},
      workQueueEntryData: {},
      crafter: -1,
      itemDivideData: {},
      itemMerchantQuantityData: {},
      merchantWantedItems: [],
      wantedItemData: {},
      resourceData: {},
      hireData: {},
      expData: {},
      refineData: {},
      refineItemData: {},
      producedItemData: [],
      craftData: {},
      merchantAction: 'buy',
      selectedTile: null,
      objIdsOnTile: [],
      createdObjOnTile: [],
      selectedBoxPos: 0,
      selectedKey: { type: '', id: -1 },
      infoItemAction: TRIGGER_INVENTORY,
      resourcesIconBorder: false,
      errmsg: '',
      noticemsg: '',
      noticeExpiry: 5000,
      confirmMsg: '',
      confirmData: {},
      showMoveCompassClick: false,
      activityData: {},
      needsData: false,
      worldData: {},
      showLeftInventoryPanel: true,
      trueDeathData: {},
      heroDeathData: null,
      heroStats: {},
      hungerStatus: '',
      thirstStatus: '',
      fatigueStatus: '',
      infoRefineItemTriggered: false,
      combatState: null,
      inCombatZoom: false,
    }

    this.handleMoveClick = this.handleMoveClick.bind(this);
    this.handleTileClick = this.handleTileClick.bind(this);

    this.handleHeroAttrsClick = this.handleHeroAttrsClick.bind(this);
    this.handleHeroInventoryClick = this.handleHeroInventoryClick.bind(this);
    this.handleHeroExploreClick = this.handleHeroExploreClick.bind(this);
    this.handleHeroBuildClick = this.handleHeroBuildClick.bind(this);
    this.handleHeroGatherClick = this.handleHeroGatherClick.bind(this);
    this.handleHeroSleepClick = this.handleHeroSleepClick.bind(this);
    this.handleHeroEquipClick = this.handleHeroEquipClick.bind(this);

    this.handleQuickAttack = this.handleQuickAttack.bind(this);
    this.handlePreciseAttack = this.handlePreciseAttack.bind(this);
    this.handleFierceAttack = this.handleFierceAttack.bind(this);

    this.handleTransitionEnd = this.handleTransitionEnd.bind(this);

    this.hideMoveCompassClick = this.hideMoveCompassClick.bind(this);

    Global.gameEmitter.on(GameEvent.LOADING_FINISHED, this.handleLoadingFinished, this);
    Global.gameEmitter.on(GameEvent.TILE_CLICK, this.handleTileClick, this);
    Global.gameEmitter.on(GameEvent.SELECTBOX_CLICK, this.handleSelectBoxClick, this);
    Global.gameEmitter.on(GameEvent.SELECT_PANEL_CLICK, this.handleSelectPanelClick, this);
    Global.gameEmitter.on(GameEvent.EXIT_HALFPANEL_CLICK, this.handleExitHalfPanelClick, this);
    Global.gameEmitter.on(GameEvent.TAP_CLICK, this.handleTargetActionPanelClick, this);
    Global.gameEmitter.on(GameEvent.VILLAGER_GATHER_CLICK, this.handleVillagerGatherClick, this);
    Global.gameEmitter.on(GameEvent.RESOURCE_GATHER_CLICK, this.handleResourceGatherClick, this);
    Global.gameEmitter.on(GameEvent.START_BUILD_CLICK, this.handleStartBuildClick, this);
    Global.gameEmitter.on(GameEvent.START_UPGRADE_CLICK, this.handleStartUpgradeClick, this);
    Global.gameEmitter.on(GameEvent.ASSIGN_CLICK, this.handleAssignClick, this);
    Global.gameEmitter.on(GameEvent.OPERATE_CLICK, this.handleOperateClick, this);
    Global.gameEmitter.on(GameEvent.REFINE_CLICK, this.handleRefineClick, this);
    Global.gameEmitter.on(GameEvent.GET_RECIPES_CLICK, this.handleGetRecipesClick, this);
    Global.gameEmitter.on(GameEvent.ERROR_OK_CLICK, this.handleErrorOkClick, this);
    Global.gameEmitter.on(GameEvent.ITEM_DIVIDE_CLICK, this.handleItemDivideClick, this);
    Global.gameEmitter.on(GameEvent.ITEM_DIVIDE_OK_CLICK, this.handleItemDivideOkClick, this);
    Global.gameEmitter.on(GameEvent.ITEM_USE_CLICK, this.handleItemUseClick, this);
    Global.gameEmitter.on(GameEvent.ITEM_DELETE_CLICK, this.handleItemDeleteClick, this);
    Global.gameEmitter.on(GameEvent.MERCHANT_BUYSELL_CLICK, this.handleMerchantBuySellClick, this);
    Global.gameEmitter.on(GameEvent.MERCHANT_QUANTITY_CANCEL, this.handleMerchantQuantityCancel, this);
    Global.gameEmitter.on(GameEvent.MERCHANT_WANTED_ITEM_CLICK, this.handleMerchantWantedItemClick, this);
    Global.gameEmitter.on(GameEvent.MERCHANT_HIRE_CLICK, this.handleMerchantHireClick, this);
    Global.gameEmitter.on(GameEvent.RESOURCE_CLICK, this.handleResourceClick, this);
    Global.gameEmitter.on(GameEvent.CRAFT_CLICK, this.handleCraftClick, this);
    Global.gameEmitter.on(GameEvent.STRUCTURE_CRAFT_CLICK, this.handleStructureCraftClick, this);
    Global.gameEmitter.on(GameEvent.CRAFT_QUEUE_CLICK, this.handleCraftQueueClick, this);
    Global.gameEmitter.on(GameEvent.NOTICE_EXPIRE, this.handleNoticeExpire, this);
    Global.gameEmitter.on(GameEvent.DELETE_STRUCTURE_CLICK, this.handleDeleteStructureClick, this);
    Global.gameEmitter.on(GameEvent.CONFIRMATION, this.handleConfirmation, this);
    Global.gameEmitter.on(GameEvent.CONFIRM_OK_CLICK, this.handleConfirmOkClick, this);
    Global.gameEmitter.on(GameEvent.RESOURCE_BUTTON_CLICK, this.handleResourceButtonClick, this);
    Global.gameEmitter.on(GameEvent.OBJ_CREATED, this.handleObjCreated, this);
    Global.gameEmitter.on(GameEvent.OBJ_DELETED, this.handleObjDeleted, this);
    Global.gameEmitter.on(GameEvent.OBJ_MOVED, this.handleObjMoved, this);
    Global.gameEmitter.on(GameEvent.OBJ_UPDATE, this.handleObjUpdate, this);
    Global.gameEmitter.on(GameEvent.HERO_DEAD, this.handleHeroDead, this);
    Global.gameEmitter.on(GameEvent.HERO_STATS_UPDATE, this.handleHeroStatsUpdate, this);
    Global.gameEmitter.on(GameEvent.CANCEL_STRUCTURE_REFINE_CLICK, this.handleCancelStructureRefine, this);
    Global.gameEmitter.on(GameEvent.CANCEL_REFINE_CLICK, this.handleCancelRefineClick, this);
    Global.gameEmitter.on(GameEvent.REFINE_OK_CLICK, this.handleRefineOkClick, this);

    //Global.gameEmitter.on(NetworkEvent.SERVER_OFFLINE, this.handleServerOffline, this);
    //Global.gameEmitter.on(NetworkEvent.NETWORK_ERROR, this.handleNetworkError, this);

    Global.gameEmitter.on(NetworkEvent.ERROR, this.handleError, this);
    Global.gameEmitter.on(NetworkEvent.NOTICE, this.handleNotice, this);
    Global.gameEmitter.on(NetworkEvent.WORLD, this.handleWorld, this);
    Global.gameEmitter.on(NetworkEvent.HERO_INIT, this.handleHeroInit, this);
    Global.gameEmitter.on(NetworkEvent.INFO_HERO, this.handleInfoHero, this);
    Global.gameEmitter.on(NetworkEvent.INFO_VILLAGER, this.handleInfoVillager, this);
    Global.gameEmitter.on(NetworkEvent.INFO_STRUCTURE, this.handleInfoStructure, this);
    Global.gameEmitter.on(NetworkEvent.INFO_NPC, this.handleInfoNPC, this);
    Global.gameEmitter.on(NetworkEvent.INFO_MONOLITH, this.handleInfoMonolith, this);
    Global.gameEmitter.on(NetworkEvent.INFO_POI, this.handleInfoPoi, this);
    Global.gameEmitter.on(NetworkEvent.INFO_OBJ, this.handleInfoObj, this);
    Global.gameEmitter.on(NetworkEvent.INFO_TILE, this.handleInfoTile, this);
    Global.gameEmitter.on(NetworkEvent.INFO_TILE_RESOURCES, this.handleInfoTileResources, this);
    Global.gameEmitter.on(NetworkEvent.INFO_ITEM, this.handleInfoItem, this);
    Global.gameEmitter.on(NetworkEvent.INFO_INVENTORY, this.handleInfoInventory, this);
    Global.gameEmitter.on(NetworkEvent.INFO_INVENTORY_SNAPSHOT, this.handleInfoInventorySnapshot, this);
    Global.gameEmitter.on(NetworkEvent.INFO_EQUIP, this.handleInfoEquip, this);
    Global.gameEmitter.on(NetworkEvent.INFO_ITEM_TRANSFER, this.handleInfoItemTransfer, this);
    Global.gameEmitter.on(NetworkEvent.INFO_ITEMS_UPDATE, this.handleInfoItemsUpdate, this);
    Global.gameEmitter.on(NetworkEvent.INFO_ACTIVITY_UPDATE, this.handleActivityUpdate, this);
    Global.gameEmitter.on(NetworkEvent.INFO_NEEDS_UPDATE, this.handleInfoNeedsUpdate, this);
    Global.gameEmitter.on(NetworkEvent.INFO_HUNGER_UPDATE, this.handleInfoHungerUpdate, this);
    Global.gameEmitter.on(NetworkEvent.INFO_THIRST_UPDATE, this.handleInfoThirstUpdate, this);
    Global.gameEmitter.on(NetworkEvent.INFO_TIREDNESS_UPDATE, this.handleInfoTirednessUpdate, this);
    Global.gameEmitter.on(NetworkEvent.INFO_ATTRS, this.handleInfoAttrs, this);
    Global.gameEmitter.on(NetworkEvent.INFO_SKILLS, this.handleInfoSkills, this);
    Global.gameEmitter.on(NetworkEvent.INFO_ADVANCE, this.handleInfoAdvance, this);
    Global.gameEmitter.on(NetworkEvent.INFO_STRUCTURE_UPGRADE, this.handleInfoUpgrade, this);
    Global.gameEmitter.on(NetworkEvent.INFO_MERCHANT, this.handleInfoMerchant, this);
    Global.gameEmitter.on(NetworkEvent.INFO_HIRE, this.handleInfoHire, this);
    Global.gameEmitter.on(NetworkEvent.INFO_EXPERIMENT, this.handleInfoExperiment, this);
    Global.gameEmitter.on(NetworkEvent.INFO_EXPERIMENT_STATE, this.handleInfoExperimentState, this);
    Global.gameEmitter.on(NetworkEvent.INFO_CROP, this.handleInfoCrop, this);
    Global.gameEmitter.on(NetworkEvent.INFO_TRUE_DEATH, this.handleInfoTrueDeath, this);
    Global.gameEmitter.on(NetworkEvent.HERO_DEATH_STATE, this.handleHeroDeathState, this);
    Global.gameEmitter.on(NetworkEvent.ITEM_TRANSFER, this.handleItemTransfer, this);
    Global.gameEmitter.on(NetworkEvent.INFO_REFINE, this.handleInfoRefine, this);
    Global.gameEmitter.on(NetworkEvent.INFO_STRUCTURE_REFINE, this.handleInfoStructureRefine, this);
    Global.gameEmitter.on(NetworkEvent.INFO_REFINE_ITEM, this.handleInfoRefineItem, this);
    Global.gameEmitter.on(NetworkEvent.INFO_CRAFT, this.handleInfoCraft, this);
    Global.gameEmitter.on(NetworkEvent.INFO_STRUCTURE_CRAFT, this.handleInfoStructureCraft, this);
    Global.gameEmitter.on(NetworkEvent.INFO_STRUCTURE_QUEUE, this.handleInfoStructureQueue, this);
    Global.gameEmitter.on(NetworkEvent.INFO_WORK_QUEUE_ENTRY, this.handleInfoWorkQueueEntry, this);
    Global.gameEmitter.on(NetworkEvent.BUY_ITEM, this.handleBuyItem, this);
    Global.gameEmitter.on(NetworkEvent.SELL_ITEM, this.handleSellItem, this);
    Global.gameEmitter.on(NetworkEvent.STRUCTURE_LIST, this.handleStructureList, this);
    Global.gameEmitter.on(NetworkEvent.INFO_ASSIGN, this.handleInfoAssign, this);
    Global.gameEmitter.on(NetworkEvent.ATTACK, this.handleAttack, this);
    Global.gameEmitter.on(NetworkEvent.ABILITY, this.handleAbility, this);
    Global.gameEmitter.on(NetworkEvent.COMBAT_STATE, this.handleCombatState, this);
    Global.gameEmitter.on(NetworkEvent.ADVANCE, this.handleAdvance, this);
    Global.gameEmitter.on(NetworkEvent.START_UPGRADE, this.handleStartUpgrade, this);
    Global.gameEmitter.on(NetworkEvent.UPGRADE, this.handleUpgrade, this);
    Global.gameEmitter.on(NetworkEvent.NEW_ITEMS, this.handleNewItems, this);
    Global.gameEmitter.on(NetworkEvent.DMG, this.handleDamage, this);
    Global.gameEmitter.on(NetworkEvent.STATS, this.handleStats, this);
  }

  handleMoveClick(event: React.MouseEvent) {
    this.setState({ showMoveCompassClick: true });
    setTimeout(this.hideMoveCompassClick, 100);

    const compass = this.compassRef.current!;

    var pocX = event.nativeEvent.offsetX - compass.naturalWidth / 2;
    var pocY = event.nativeEvent.offsetY - compass.naturalHeight / 2;

    var angleRads = Math.atan2(pocX, pocY);
    var angleDegrees = ((angleRads * 180) / Math.PI) + 180;

    console.log(Global.objectStates);
    console.log(Global.heroId);
    var heroObj = Global.objectStates[Global.heroId] as ObjectState;

    if (angleDegrees < 30 || angleDegrees >= 330) {
      console.log('N');
      var nextPos = Util.nextPosByDirection(heroObj.x, heroObj.y, 'N');
      Global.network.sendMove(nextPos.q, nextPos.r);
    } else if (angleDegrees < 90 && angleDegrees >= 30) {
      console.log('NW');
      var nextPos = Util.nextPosByDirection(heroObj.x, heroObj.y, 'NW');
      Global.network.sendMove(nextPos.q, nextPos.r);
    } else if (angleDegrees < 150 && angleDegrees >= 90) {
      console.log('SW');
      var nextPos = Util.nextPosByDirection(heroObj.x, heroObj.y, 'SW');
      Global.network.sendMove(nextPos.q, nextPos.r);
    } else if (angleDegrees < 210 && angleDegrees >= 150) {
      console.log('S');
      var nextPos = Util.nextPosByDirection(heroObj.x, heroObj.y, 'S');
      Global.network.sendMove(nextPos.q, nextPos.r);
    } else if (angleDegrees < 270 && angleDegrees >= 210) {
      console.log('SE');
      var nextPos = Util.nextPosByDirection(heroObj.x, heroObj.y, 'SE');
      Global.network.sendMove(nextPos.q, nextPos.r);
    } else if (angleDegrees < 330 && angleDegrees >= 270) {
      console.log('NE');
      var nextPos = Util.nextPosByDirection(heroObj.x, heroObj.y, 'NE');
      Global.network.sendMove(nextPos.q, nextPos.r);
    }
  }

  handleLoadingFinished() {
    this.setState({ hideLoadingPanel: true });
  }

  handleTileClick(gameObject) {
    console.log("Tile clicked: " + JSON.stringify(gameObject));

    if (Global.resourceLayerVisible) {
      Global.network.sendInfoTile(gameObject.hexX, gameObject.hexY);
    } else {

      var objIdsOnTile = Obj.getObjsAt(gameObject.hexX, gameObject.hexY);
      console.log("ObjIdsOnTile: " + JSON.stringify(objIdsOnTile));

      var pos;
      var selectedKey;

      if (objIdsOnTile.length > 0) {
        pos = objIdsOnTile.length;
        selectedKey = {
          type: OBJ,
          id: Number(objIdsOnTile[objIdsOnTile.length - 1])
        }
      } else {
        pos = 0;
        selectedKey = {
          type: TILE,
          x: gameObject.hexX,
          y: gameObject.hexY
        }
      }

      Global.selectedKey = selectedKey;

      this.setState({
        selectedTile: gameObject,
        objIdsOnTile: objIdsOnTile,
        hideSelectPanel: false,
        hideTargetActionPanel: false,
        selectedBoxPos: pos,
        selectedKey: selectedKey
      })

      //Global.gameEmitter.emit(GameEvent.SELECTBOX_CLICK, eventData);

      /*this.setState({
        selectedTile: gameObject,
        objIdsOnTile: objIdsOnTile,
        hideSelectPanel: false,
        hideTargetActionPanel: true
      });*/
    }
  }

  handleSelectBoxClick(eventData) {
    console.log('SelectBoxClick');

    Global.selectedKey = eventData.selectedKey;

    this.setState({
      hideTargetActionPanel: false,
      selectedBoxPos: eventData.pos,
      selectedKey: eventData.selectedKey
    })
  }

  handleSelectPanelClick() {
    this.setState({ hideTargetActionPanel: true });
  }

  handleExitHalfPanelClick(event) {
    console.log('ExitHalfPanel');

    if (event.panelType == 'inventory') {
      this.setState({ hideInventoryPanel: true });
      Global.network.sendInfoExit(this.state.inventoryData.id, "inventory");
    } else if (event.panelType == 'itemTransfer') {
      this.setState({ hideItemTransferPanel: true })
    } else if (event.panelType == 'merchant') {
      this.setState({ hideMerchantPanel: true })
    } else if (event.panelType == 'wanteditempanel') {
      this.setState({ hideWantedItemPanel: true });
    } else if (event.panelType == 'item') {
      this.setState({ hideItemPanel: true });
    } else if (event.panelType == 'hero') {
      this.setState({ hideHeroPanel: true });
    } else if (event.panelType == 'villager') {
      this.setState({ hideVillagerPanel: true });
      Global.network.sendInfoExit(this.state.villagerData.id, "villager");
    } else if (event.panelType == 'npc') {
      this.setState({ hideNPCPanel: true });
    } else if (event.panelType == 'obj') {
      this.setState({ hideObjPanel: true });
    } else if (event.panelType == 'attrs') {
      this.setState({ hideAttrsPanel: true });
    } else if (event.panelType == 'equip') {
      this.setState({ hideEquipPanel: true });
      Global.network.sendInfoExit(Global.equipPanelObjId, "equip");
    } else if (event.panelType == 'skills') {
      this.setState({ hideSkillsPanel: true });
    } else if (event.panelType == 'advance') {
      this.setState({ hideAdvancePanel: true });
    } else if (event.panelType == 'upgrade') {
      this.setState({ hideStructureUpgradePanel: true });
    } else if (event.panelType == 'tile') {
      this.setState({ hideTilePanel: true });
    } else if (event.panelType == 'tile_resources') {
      this.setState({ hideTileResourcesPanel: true });
    } else if (event.panelType == 'terrain_features') {
      this.setState({ hideTerrainFeaturePanel: true });
    } else if (event.panelType == 'build') {
      this.setState({ hideBuildPanel: true });
    } else if (event.panelType == 'structure') {
      this.setState({
        hideStructurePanel: true,
        hideAssignPanel: true
      });
    } else if (event.panelType == 'assign') {
      this.setState({ hideAssignPanel: true });
    } else if (event.panelType == 'refine') {
      this.setState({ hideRefinePanel: true });
      Global.network.sendInfoExit(this.state.refineData.source_id, "refine");
    } else if (event.panelType == 'craft') {
      this.setState({ hideCraftPanel: true });
      Global.network.sendInfoExit(this.state.craftData.crafter_id, "craft");
    } else if (event.panelType == 'structure_craft') {
      this.setState({ hideStructureCraftPanel: true });
    } else if (event.panelType == 'structure_refine') {
      this.setState({ hideStructureRefinePanel: true });
      Global.network.sendInfoExit(this.state.structureData.id, "structure_refine");
    } else if (event.panelType == 'workqueue') {
      this.setState({ hideWorkQueuePanel: true });
      Global.network.sendInfoExit(this.state.structureData.id, "structure_queue");
    } else if (event.panelType == 'workqueueentry') {
      this.setState({ hideWorkQueueEntryPanel: true });
    } else if (event.panelType == 'resource') {
      this.setState({ hideResourcePanel: true });
    } else if (event.panelType == 'hire') {
      this.setState({ hideMerchantHirePanel: true });
    } else if (event.panelType == 'experiment') {
      Global.network.sendInfoExit(this.state.expData.id, "experiment");
      this.setState({ hideExperimentPanel: true });
    } else if (event.panelType == 'confirm') {
      this.setState({ hideConfirmPanel: true });
    }
  }

  handleErrorOkClick() {
    if (Global.heroDead || Global.networkError || Global.serverOffline) {
      location.reload();
    }

    this.setState({ hideErrorPanel: true });
  }

  handleOperateClick() {
    this.setState({ hideStructurePanel: true });
  }

  handleRefineClick(eventData) {

  }

  handleGetRecipesClick(crafter) {
    this.setState({ crafter: crafter });
  }

  handleAssignClick() {
    this.setState({ hideAssignPanel: true });
  }

  handleItemDivideClick(itemData) {
    this.setState({
      hideItemDividePanel: false,
      itemDivideData: itemData
    });
  }

  handleItemDivideOkClick() {
    this.setState({ hideItemDividePanel: true });
  }

  handleItemUseClick() {
    this.setState({
      hideItemPanel: true,
    });
  }

  handleItemDeleteClick() {
    this.setState({
      hideItemPanel: true,
    });
  }

  handleMerchantBuySellClick(eventData) {
    this.setState({
      hideMerchantQuantityPanel: false,
      itemMerchantQuantityData: eventData.itemData,
      merchantAction: eventData.action
    });
  }

  handleMerchantHireClick() {
    this.setState({
      hideMerchantHirePanel: true,
      hideMerchantPanel: true
    });
  }

  handleMerchantWantedItemClick() {
    this.setState({
      hideWantedItemPanel: false,
      wantedItemData: Global.wantedItemData
    });
  }

  handleMerchantQuantityCancel() {
    this.setState({ hideMerchantQuantityPanel: true });
  }

  handleTargetActionPanelClick(event: React.MouseEvent) {
    //this.setState({ hideTargetActionPanel: true });
  }

  handleVillagerGatherClick(event: React.MouseEvent) {
    this.setState({ hideGatherPanel: false });
  }

  handleResourceGatherClick(event: React.MouseEvent) {
    this.setState({ hideGatherPanel: true });
  }

  handleStartBuildClick(event: React.MouseEvent) {
    this.setState({ hideBuildPanel: true });
    this.setState({ hideItemTransferPanel: true });
  }

  handleStartUpgradeClick(event: React.MouseEvent) {
    this.setState({
      hideStructureUpgradePanel: true,
      hideItemTransferPanel: true
    });
  }

  handleHeroAttrsClick(event: React.MouseEvent) {
    Global.network.sendInfoObj(Global.heroId);
  }

  handleHeroInventoryClick(event: React.MouseEvent) {
    Global.network.sendInfoInventory(Global.heroId);
  }

  handleHeroExploreClick(event: React.MouseEvent) {
    Global.network.sendExplore();
  }

  handleHeroBuildClick(event: React.MouseEvent) {
    Global.network.sendGetStructureList()
  }

  handleHeroGatherClick(event: React.MouseEvent) {
    /*this.setState({
      selectedKey: { type: OBJ, id: Global.heroId },
      hideGatherPanel: false
    });*/
    Global.network.sendGather();
  }

  handleHeroSleepClick(event: React.MouseEvent) {
    //Network.sendRest(Global.heroId);

    if (!Global.resourceLayerVisible) {
      Global.network.sendNearbyResources();
      this.setState({ resourcesIconBorder: true });
    } else {
      Global.gameEmitter.emit(GameEvent.RESOURCE_LAYER_CLICK, {});
      this.setState({ resourcesIconBorder: false });
    }
  }

  handleHeroEquipClick(event: React.MouseEvent) {
    Global.network.sendInfoEquip(Global.heroId);
  }

  handleHeroCraftClick(event: React.MouseEvent) {
    // Structure id is -1 for hero craft
    Global.network.sendInfoCraft(Global.heroId);
  }

  handleComboClick() {
    const combatState = Global.combatState || {};
    const comboType = combatState.available_finisher;
    if (!comboType) {
      return;
    }

    const targetId = combatState.target_id !== undefined
      ? combatState.target_id
      : Global.selectedKey.id;
    Global.network.sendCombo(Global.heroId, targetId, comboType);
  }

  handleQuickAttack(event: React.MouseEvent) {
    Global.network.sendAttack('quick', Global.heroId, Global.selectedKey.id);
  }

  handlePreciseAttack(event: React.MouseEvent) {
    Global.network.sendAttack('precise', Global.heroId, Global.selectedKey.id);
  }

  handleFierceAttack(event: React.MouseEvent) {
    Global.network.sendAttack('fierce', Global.heroId, Global.selectedKey.id);
  }

  handleBrace() {
    Global.network.sendBlock(Global.heroId);
  }

  handleAbilityClick(abilityId: string) {
    const targetId = Global.selectedKey && Global.selectedKey.id !== undefined
      ? Global.selectedKey.id
      : undefined;
    Global.network.sendAbility(abilityId, Global.heroId, targetId);
  }

  handleDamage(message) {
    var hideAttacks = Global.attacks.length == 0;
    this.setState({ hideAttacksPanel: hideAttacks });

    if (this.state.hideVillagerPanel == false && message.target_id == this.state.villagerData.id) {
      let newVillagerData = this.state.villagerData;
      newVillagerData.hp = newVillagerData.hp - message.dmg;
      this.setState({ villagerData: newVillagerData });
    }

    if (message.target_id == Global.heroId && !this.state.inCombatZoom) {
      Global.gameEmitter.emit(GameEvent.CAMERA_ZOOM, { zoom: 2, duration: 250 });
      this.setState({ inCombatZoom: true });
    }
  }

  handleCombatState(message) {
    const attackHistory = message && message.attack_history ? message.attack_history : [];
    const hasComboHint = message && ((message.matching_combos && message.matching_combos.length > 0) || message.available_finisher);
    const inCombat = attackHistory.length > 0 || hasComboHint;

    if (inCombat && !this.state.inCombatZoom) {
      Global.gameEmitter.emit(GameEvent.CAMERA_ZOOM, { zoom: 2, duration: 250 });
      this.setState({ inCombatZoom: true });
    } else if (!inCombat && this.state.inCombatZoom) {
      Global.gameEmitter.emit(GameEvent.CAMERA_ZOOM, { zoom: 1, duration: 350 });
      this.setState({ inCombatZoom: false });
    }

    this.setState({
      combatState: message,
      hideAttacksPanel: attackHistory.length == 0 && !hasComboHint,
    });
  }

  handleStats(message) {
    console.log('UI handleStats');
    this.setState({ heroStats: message, hungerStatus: message.hunger, thirstStatus: message.thirst, fatigueStatus: message.tiredness });
  }

  handleHeroStatsUpdate(event) {
    console.log('UI handleHeroStatsUpdate');
    this.setState({ heroStats: { ...this.state.heroStats, ...event } });
  }



  handleAttack(message) {
    Global.heroStamina = Math.max(0, Global.heroStamina - message.stamina_cost);
    Global.gameEmitter.emit(GameEvent.HERO_STATS_UPDATE, { hp: Global.heroHp, stamina: Global.heroStamina, mana: Global.heroMana });
  }

  handleAbility(message) {
    Global.heroStamina = Math.max(0, Global.heroStamina - (message.stamina_cost || 0));
    Global.heroMana = Math.max(0, Global.heroMana - (message.mana_cost || 0));
    Global.gameEmitter.emit(GameEvent.HERO_STATS_UPDATE, { hp: Global.heroHp, stamina: Global.heroStamina, mana: Global.heroMana });
  }

  handleAdvance(message) {
    console.log('handleAdvance');
    this.setState({ advanceData: message })
  }

  handleStartUpgrade(message) {
    console.log('handleStartUpgrade');
    //Network.sendInfoItemTransfer(Global.heroId, message.structure_id);
    Global.network.sendInfoObj(message.structure_id);
    this.setState({ hideStructureUpgradePanel: true, hideStructurePanel: true });

  }

  handleUpgrade(message) {
    console.log('handleUpgrade');
    this.setState({
      hideStructureUpgradePanel: true,
      hideStructurePanel: true
    });
  }

  handleNewItems(message) {
    console.log('handleNewItems');
    var sourceName = Global.objectStates[message.source_id].name;
    var msg = '';

    if (message.amount > 1) {
      if (message.action == 'crafting') {
        msg = sourceName + " has crafted (" + message.amount + "x) " + message.item_name;
      } else if (message.action == 'refining') {
        msg = sourceName + " has refined (" + message.amount + "x) " + message.item_name;
      } else if (message.action == 'gathering') {
        msg = sourceName + " has gathered (" + message.amount + "x) " + message.item_name;
      } else if (message.action == 'exploring') {
        msg = sourceName + " has discovered (" + message.amount + "x) sources of " + message.item_name;
      }
    } else {
      if (message.action == 'crafting') {
        msg = sourceName + " has crafted a " + message.item_name;
      } else if (message.action == 'refining') {
        msg = sourceName + " has refined a " + message.item_name;
      } else if (message.action == 'gathering') {
        msg = sourceName + " has gathered a " + message.item_name;
      } else if (message.action == 'exploring') {
        msg = sourceName + " has discovered a source of " + message.item_name;
      }
    }

    this.setState({
      hideNoticePanel: false,
      noticemsg: msg,
      noticeExpiry: 5000
    });
  }

  handleNoticeExpire() {
    this.setState({ hideNoticePanel: true });
  }

  handleNotice(message) {
    if (message.expiry) {
      this.setState({ noticeExpiry: message.expiry, hideNoticePanel: false, noticemsg: message.noticemsg });
    } else {
      this.setState({
        hideNoticePanel: false,
        noticemsg: message.noticemsg
      });
    }
  }

  handleError(message) {
    this.setState({
      hideErrorPanel: false,
      errmsg: message.errmsg
    });
  }

  handleConfirmation(event) {
    this.setState({
      hideConfirmPanel: false,
      confirmMsg: event.msg,
      confirmData: event.data
    });
  }

  handleConfirmOkClick(event) {
    this.setState({
      hideConfirmPanel: true,
    });
  }

  handleResourceButtonClick(event) {
    console.log('handleResourceButtonClick');
    // Show tile resource panel
    this.setState({
      hideTileResourcesPanel: false,
    });
  }

  handleServerOffline() {
    Global.serverOffline = true;

    this.setState({
      hideErrorPanel: false,
      errmsg: "The server is offline..."
    })
  }

  handleHeroInit() {
    this.setState({ selectedKey: { type: OBJ, id: Global.heroId } });
  }

  handleHeroDead() {
    Global.heroDead = true;

    this.setState({
      hideNoticePanel: false,
      noticemsg: Global.objectStates[Global.heroId].name + " has fallen.  The Monolith weighs their soul...",
      noticeExpiry: 12000
    });
  }

  handleHeroDeathState(message) {
    if (this.heroDeathOverlayTimer) {
      clearTimeout(this.heroDeathOverlayTimer);
      this.heroDeathOverlayTimer = null;
    }

    Global.heroDead = message.phase != 'resurrected';
    this.setState({ heroDeathData: message });

    if (message.phase == 'resurrected') {
      this.heroDeathOverlayTimer = setTimeout(() => {
        this.setState({ heroDeathData: null });
        this.heroDeathOverlayTimer = null;
      }, 3500);
    }
  }

  handleResourceClick(eventData) {
    this.setState({
      hideResourcePanel: false,
      resourceData: eventData
    });
  }

  handleCraftClick() {
    this.setState({
      hideStructureCraftPanel: true,
      hideStructurePanel: false
    });
  }

  handleStructureCraftClick(eventData) {
    /*var recipesData = [];
    // Populate one test recipe 
    recipesData.push({
      id: 1,
      name: 'Test Recipe',
      class: 'Test Class',
      subclass: 'Test Subclass',
      slot: 'Test Slot',
      damage: 10,
      speed: 10,
      skill_req: 10,
      stamina_req: 10,
      req: 
        { type: 'Wood', quantity: 10 },
        { type: 'Ingot', quantity: 10 }
      ]
    });


    this.setState({
      hideStructureCraftPanel: false,
      structureData: eventData.structureData,
      recipesData: recipesData
    });*/
  }

  handleCraftQueueClick() {
    this.setState({
      hideStructureCraftPanel: true,
      hideWorkQueuePanel: true,
      hideAssignPanel: true,
      hideStructurePanel: true
    });
  }


  handleDeleteStructureClick() {
    console.log('handleDeleteStructureClick')
    this.setState({ hideStructurePanel: true });
  }

  hideMoveCompassClick() {
    this.setState({ showMoveCompassClick: false });
  }

  handleObjCreated(objId) {
    console.log('Obj Created: ' + objId);

    if (this.state.selectedTile) {
      if (this.state.selectedTile.hexX == Global.objectStates[objId].x &&
        this.state.selectedTile.hexY == Global.objectStates[objId].y) {
        this.setState({ objIdsOnTile: Obj.getObjsAt(this.state.selectedTile.hexX, this.state.selectedTile.hexY) });
      }
    }
  }

  handleObjDeleted(objId) {
    console.log('Obj Deleted: ' + objId);

    if (this.state.selectedTile) {
      if (this.state.selectedTile.hexX == Global.objectStates[objId].x &&
        this.state.selectedTile.hexY == Global.objectStates[objId].y) {
        this.setState({ objIdsOnTile: Obj.getObjsAt(this.state.selectedTile.hexX, this.state.selectedTile.hexY) });
      }
    }
  }

  handleObjMoved(objId) {
    console.log('Obj Moved: ' + objId);
    // Entering current selected tile
    if (this.state.selectedTile) {
      if (this.state.selectedTile.hexX == Global.objectStates[objId].x &&
        this.state.selectedTile.hexY == Global.objectStates[objId].y) {
        this.setState({ objIdsOnTile: Obj.getObjsAt(this.state.selectedTile.hexX, this.state.selectedTile.hexY) });
      }
    }

    // Leaving current selected tile
    if (this.state.selectedTile) {
      if (this.state.selectedTile.hexX == Global.objectStates[objId].prevX &&
        this.state.selectedTile.hexY == Global.objectStates[objId].prevY) {

        // Check if moving obj is selected
        if (Global.selectedKey.id == objId) {
          var objMovedEvent = {
            hexX: Global.objectStates[objId].x,
            hexY: Global.objectStates[objId].y
          }
          console.log(objMovedEvent);
          Global.gameEmitter.emit(GameEvent.SELECTED_OBJ_MOVED, objMovedEvent);
        } else {
          this.setState({ objIdsOnTile: Obj.getObjsAt(this.state.selectedTile.hexX, this.state.selectedTile.hexY) });
        }
      }
    }
  }

  handleObjUpdate(objId) {
    console.log('Obj Update: ' + objId);

    if (this.state.hideStructurePanel == false && objId == this.state.structureData.id) {
      let newStructureData = this.state.structureData;
      newStructureData.state = Global.objectStates[objId].state;
      this.setState({ structureData: newStructureData });
    } else if (this.state.hideVillagerPanel == false && objId == this.state.villagerData.id) {
      let newVillagerData = this.state.villagerData;
      newVillagerData.state = Global.objectStates[objId].state;
      this.setState({ villagerData: newVillagerData });
    }
  }

  handleInfoMonolith(message) {
    console.log('UI handleInfoMonolith');
    this.setState({ hideObjPanel: false, objData: message });
  }

  handleInfoPoi(message) {
    console.log('UI handleInfoPoi');
    this.setState({ hideObjPanel: false, objData: message });
  }

  handleInfoObj(message) {
    console.log('UI handleInfoObj');
    this.setState({ hideObjPanel: false, objData: message });
  }

  handleInfoHero(message) {
    console.log('UI handleInfoHero');
    if (Util.isPlayerObj(message.id)) {
      Global.heroClass = message.hero_class || Global.heroClass;
      Global.heroMana = message.mana !== undefined ? message.mana : Global.heroMana;
      Global.heroMaxMana = message.base_mana !== undefined ? message.base_mana : Global.heroMaxMana;
      this.setState({ hideHeroPanel: false, heroDetailedData: message });
    }
  }

  handleInfoVillager(message) {
    console.log('UI handleInfoVillager');
    if (Util.isPlayerObj(message.id)) {
      const activityData = { ...(this.state.activityData || {}) };
      if (message.activity != null) {
        activityData[message.id] = message.activity;
      }
      this.setState({ hideVillagerPanel: false, villagerData: message, activityData });
    }
  }

  handleInfoStructure(message) {
    console.log('UI handleInfoStructure');
    if (Util.isPlayerObj(message.id)) {
      this.setState({ hideStructurePanel: false, structureData: message });
    }
  }

  handleInfoNPC(message) {
    console.log('UI handleInfoNPC');
    this.setState({ hideNPCPanel: false, npcData: message });
  }

  handleInfoTile(message) {
    console.log('UI handleInfoTile');
    this.setState({
      hideTilePanel: false,
      //hideTileResourcesPanel: false,
      hideTerrainFeaturePanel: false,
      tileData: message
    });
  }

  handleInfoTileResources(message) {
    console.log('UI handleInfoTileResources');
    this.setState({ hideTileResourcesPanel: false, tileResourcesData: message });
  }

  handleInfoItem(message) {
    console.log('UI handleInfoItem');

    this.setState({
      hideItemPanel: false,
      itemData: message,
      infoItemAction: Global.infoItemAction
    });
  }

  handleInfoInventory(message) {
    console.log('UI handleInfoInventory');

    // If showLeftInventoryPanel is true, show the left inventory panel
    if (Global.showLeftInventoryPanel) {
      this.setState({ hideInventoryPanel: false, inventoryData: message, showLeftInventoryPanel: true });
    } else {
      this.setState({ hideInventoryPanel: false, inventoryData: message, showLeftInventoryPanel: false });

      // Reset showLeftInventoryPanel back to true
      Global.showLeftInventoryPanel = true;
    }
  }

  handleInfoInventorySnapshot(message) {
    console.log('UI handleInfoInventorySnapshot');
    console.log('message.id: ' + message.id);
    console.log('this.state.inventoryData.id: ' + this.state.inventoryData.id);
    console.log('this.state.leftInventoryData.id: ' + this.state.leftInventoryData.id);
    console.log('this.state.rightInventoryData.id: ' + this.state.rightInventoryData.id);

    if (message.id == this.state.inventoryData.id) {
      this.setState({ inventoryData: message });
    }

    if (message.id == this.state.leftInventoryData.id) {
      this.setState({ leftInventoryData: message });
    }

    if (message.id == this.state.rightInventoryData.id) {
      this.setState({ rightInventoryData: message });
    }

    if (message.id == this.state.structureInventoryData.id) {
      this.setState({ structureInventoryData: message });
    }
  }

  handleInfoEquip(message) {
    console.log('UI handleInfoEquip');
    Global.equipPanelObjId = message.id;
    this.setState({ hideEquipPanel: false, rightInventoryData: message });
  }

  handleInfoItemTransfer(message) {
    console.log('UI handleInfoItemTransfer');
    console.log(message);
    console.log('leftInventoryData.id: ' + message.sourceitems.id);
    console.log('rightInventoryData.id: ' + message.targetitems.id);

    if (Global.infoItemTransferAction == 'transfer') {
      this.setState({
        hideItemTransferPanel: false,
        leftInventoryId: message.source_id,
        leftInventoryData: message.sourceitems,
        rightInventoryId: message.target_id,
        rightInventoryData: message.targetitems,
        inventoryReqs: message.reqitems
      });
    }
    // No longer needed
    /*else if (Global.infoItemTransferAction == 'merchant') {
      this.setState({
        hideMerchantPanel: false,
        leftInventoryId: message.source_id,
        leftInventoryData: message.sourceitems,
        rightInventoryId: message.target_id,
        rightInventoryData: message.targetitems
      });
    }*/
  }

  handleInfoItemsUpdate(message) {
    console.log('UI handleInfoItemUpdate');

    for (var j = 0; j < message.items_updated.length; j++) {
      if (this.state.itemData.id == message.items_updated[j].id) {
        this.setState({ itemData: message.items_updated[j] });
        break;
      }
    }

    // Single inventory panel
    if (message.id == this.state.inventoryData.id) {
      var newInventoryData: any = { ...this.state.inventoryData };

      for (var j = 0; j < message.items_updated.length; j++) {
        var item_found = false;

        //Check if updated quantity of existing item
        for (var i = 0; i < newInventoryData.items.length; i++) {
          if (newInventoryData.items[i].id == message.items_updated[j].id) {
            newInventoryData.items[i] = message.items_updated[j];
            item_found = true;
          }
        }

        //Check if it is a brand new item
        if (!item_found) {
          newInventoryData.items.push(message.items_updated[j]);
        }
      }

      //Filter out removed items
      for (var i = 0; i < message.items_removed.length; i++) {
        newInventoryData.items = newInventoryData.items.filter(item => item.id != message.items_removed[i]);
      }

      let new_total_weight = 0;

      //Recalculate total weight
      for (var i = 0; i < newInventoryData.items.length; i++) {
        var item = newInventoryData.items[i];
        new_total_weight += (item.quantity * item.weight);
      }

      // Set new total weight
      newInventoryData.tw = new_total_weight;

      this.setState({ inventoryData: newInventoryData });
    }

    // Multi inventory screens (left)
    if (message.id == this.state.leftInventoryData.id) {
      var newLeftInventoryData: any = { ...this.state.leftInventoryData };

      for (var j = 0; j < message.items_updated.length; j++) {
        var item_found = false;

        //Check if updated quantity of existing item
        for (var i = 0; i < newLeftInventoryData.items.length; i++) {
          if (newLeftInventoryData.items[i].id == message.items_updated[j].id) {
            newLeftInventoryData.items[i] = message.items_updated[j];
            item_found = true;
          }
        }

        //Check if it is a brand new item
        if (!item_found) {
          newLeftInventoryData.items.push(message.items_updated[j]);
        }
      }

      //Filter out removed items
      for (var i = 0; i < message.items_removed.length; i++) {
        newLeftInventoryData.items = newLeftInventoryData.items.filter(item => item.id != message.items_removed[i]);
      }

      let new_total_weight = 0;

      //Recalculate total weight
      for (var i = 0; i < newLeftInventoryData.items.length; i++) {
        var item = newLeftInventoryData.items[i];
        new_total_weight += (item.quantity * item.weight);
      }

      // Set new total weight
      newLeftInventoryData.tw = new_total_weight;

      this.setState({ leftInventoryData: newLeftInventoryData });
    }

    // Multi inventory screens (right)
    if (message.id == this.state.rightInventoryData.id) {
      var newRightInventoryData: any = { ...this.state.rightInventoryData };

      for (var j = 0; j < message.items_updated.length; j++) {
        var item_found = false;

        for (var i = 0; i < newRightInventoryData.items.length; i++) {
          if (newRightInventoryData.items[i].id == message.items_updated[j].id) {
            newRightInventoryData.items[i] = message.items_updated[j];
            item_found = true;
          }
        }

        if (!item_found) {
          newRightInventoryData.items.push(message.items_updated[j]);
        }
      }

      //Filter out removed items
      for (var i = 0; i < message.items_removed.length; i++) {
        newRightInventoryData.items = newRightInventoryData.items.filter(item => item.id != message.items_removed[i]);
      }

      this.setState({ rightInventoryData: newRightInventoryData });
    }
  }

  handleActivityUpdate(message) {
    const activityData = { ...(this.state.activityData || {}) };
    activityData[message.id] = message.activity;
    this.setState({ activityData });
  }

  handleInfoNeedsUpdate(message) {

    if (message.id == Global.heroId) {
      this.setState({ hungerStatus: message.hunger, thirstStatus: message.thirst, fatigueStatus: message.tiredness });
    } else {
      this.setState({ needsData: message });
    }
  }

  handleInfoHungerUpdate(message) {
    this.setState({ hungerStatus: message.hunger });
  }

  handleInfoThirstUpdate(message) {
    this.setState({ thirstStatus: message.thirst });
  }

  handleInfoTirednessUpdate(message) {
    this.setState({ fatigueStatus: message.tiredness });
  }

  handleInfoAttrs(message) {
    console.log('UI handleInfoAttrs');
    this.setState({ hideAttrsPanel: false, attrsData: message });
    // Why is this there?
    //this.setState({ hideEquipPanel: false, attrsData: message });
  }

  handleInfoSkills(message) {
    console.log('UI handleInfoSkills');
    this.setState({ hideSkillsPanel: false, skillsData: message });
  }

  handleInfoAdvance(message) {
    console.log('UI handleInfoAdvance');
    this.setState({ hideAdvancePanel: false, advanceData: message });
  }

  handleInfoUpgrade(message) {
    console.log('UI handleInfoUpgrade');
    this.setState({ hideStructureUpgradePanel: false, structureUpgradeData: message });
  }

  handleInfoMerchant(message) {
    console.log('UI handleInfoMerchant');
    console.log(message);

    this.setState({
      hideMerchantPanel: false,
      leftInventoryId: message.source_id,
      leftInventoryData: message.inventory,
      rightInventoryId: message.merchant_id,
      rightInventoryData: message.merchant_inventory,
      merchantWantedItems: message.merchant_wanted_items
    });
  }

  handleInfoHire(message) {
    console.log('UI handleInfoHire');
    if (message.data.length > 0) {
      this.setState({ hideMerchantHirePanel: false, hireData: message.data })
    } else {
      this.setState({
        hideErrorPanel: false,
        errmsg: "No hires availables"
      });
    }
  }

  handleInfoExperiment(message) {
    console.log("UI handleInfoExperiment");
    this.setState({ hideExperimentPanel: false, expData: message });
  }

  handleInfoExperimentState(message) {
    console.log("UI handleInfoExperiment");

    var newExpData = this.state.expData;
    newExpData.state = message.state;

    this.setState({ hideExperimentPanel: false, expData: newExpData });
  }

  handleInfoCrop(message) {
    console.log("UI handleInfoCrop");

    var newStructureData = this.state.structureData;
    newStructureData.crop_type = message.crop_type;
    newStructureData.crop_quantity = message.crop_quantity;
    newStructureData.crop_stage = message.crop_stage;

    this.setState({ structureData: newStructureData });
  }

  handleInfoTrueDeath(message) {
    console.log("UI handleInfoTrueDeath");
    this.setState({ hideTrueDeathPanel: false, trueDeathData: message, heroDeathData: null });
  }

  /*handleNearbyResources(message) {
    console.log("UI handleNearbyResources");
    console.log(message);
  }*/

  handleItemTransfer(message) {
    console.log('UI handleItemTransfer leftId: ' + this.state.leftInventoryId + ' rightId: ' +
      this.state.rightInventoryId + ' sourceId: ' + message.source_id + ' targetId: ' + message.target_id);

    var leftInventoryData;
    var rightInventoryData;

    if (this.state.leftInventoryId == message.source_id) {
      leftInventoryData = message.sourceitems;
    } else if (this.state.leftInventoryId == message.target_id) {
      leftInventoryData = message.targetitems;
    }

    if (this.state.rightInventoryId == message.source_id) {
      rightInventoryData = message.sourceitems;
    } else if (this.state.rightInventoryId == message.target_id) {
      rightInventoryData = message.targetitems;
    }

    this.setState({
      hideItemTransferPanel: false,
      leftInventoryData: leftInventoryData,
      rightInventoryData: rightInventoryData,
      inventoryReqs: message.reqitems
    })
  }

  handleInfoRefine(message) {
    console.log('UI handleInfoRefine');

    // Get product items data from item data and produced items id
    var producedItemData = [];
    for (var i = 0; i < message.produced_items.length; i++) {
      var itemId = message.produced_items[i][0];
      var quantity = message.produced_items[i][1];
      var itemData = structuredClone(message.refiner_items.find(item => item.id == itemId));

      // Override quantity with produced quantity
      itemData.quantity = quantity;

      producedItemData.push(itemData);
    }

    var newInventoryData = { ...this.state.inventoryData };
    newInventoryData.items = message.refiner_items;

    this.setState({ inventoryData: newInventoryData, producedItemData: producedItemData });
  }

  handleInfoStructureRefine(message) {
    console.log('UI handleInfoStructureRefine');
    this.setState({
      hideStructureRefinePanel: false,
      structureInventoryData: message.structure_inventory,
      refineData: message,
      refineItemData: message.refining_item,
      infoRefineItemTriggered: false,
    });
  }

  handleInfoRefineItem(message) {
    console.log('UI handleInfoRefineItem');
    this.setState({
      hideStructureRefinePanel: false,
      refineItemData: message,
      infoRefineItemTriggered: true
    });
  }


  handleCancelRefineClick() {
    console.log('UI handleCancelRefineClick');
    this.setState({ hideRefinePanel: true, });
  }

  handleCancelStructureRefine() {
    console.log('UI handleCancelRefine');
    this.setState({ hideStructureRefinePanel: false, refineItemData: {}, infoRefineItemTriggered: false });
  }

  handleRefineOkClick() {
    console.log('UI handleRefineOkClick');
    this.setState({ hideRefinePanel: true, });
  }



  handleBuyItem(message) {
    console.log(message);
    var leftInventoryData;
    var rightInventoryData;

    if (this.state.leftInventoryId == message.source_id) {
      leftInventoryData = message.inventory;
    }

    if (this.state.rightInventoryId == message.merchant_id) {
      rightInventoryData = message.merchant_inventory;
    }

    this.setState({
      hideMerchantPanel: false,
      hideItemPanel: true,
      leftInventoryData: leftInventoryData,
      rightInventoryData: rightInventoryData
    })

  }

  handleSellItem(message) {
    console.log(message);
    var leftInventoryData;
    var rightInventoryData;

    if (this.state.leftInventoryId == message.source_id) {
      leftInventoryData = message.inventory;
    }

    if (this.state.rightInventoryId == message.merchant_id) {
      rightInventoryData = message.merchant_inventory;
    }

    this.setState({
      hideMerchantPanel: false,
      hideItemPanel: true,
      leftInventoryData: leftInventoryData,
      rightInventoryData: rightInventoryData,
      merchantWantedItems: message.merchant_wanted_items
    })

  }

  handleStructureList(message) {
    //TODO look to fix the structures list packet
    this.setState({ hideBuildPanel: false, structuresData: message.result });
  }

  handleInfoAssign(message) {
    this.setState({ hideAssignPanel: false, assignData: message.assignments });
  }

  handleInfoCraft(message) {
    console.log('handleInfoCraft');
    console.log(JSON.stringify(message));
    this.setState({ hideCraftPanel: false, craftData: message });
  }

  handleInfoStructureCraft(message) {
    console.log('handleInfoStructureCraft');

    this.setState({
      hideStructureCraftPanel: false,
      structureInventoryData: message.structure_inventory,
      recipesData: message.recipes,
      craftData: message
    });

  }

  handleInfoStructureQueue(message) {
    console.log('UI handleInfoStructureQueue: ' + JSON.stringify(message));
    this.setState({ hideWorkQueuePanel: false, workQueueData: message.queue });
  }

  handleInfoWorkQueueEntry(message) {
    console.log('UI handleInfoWorkQueueEntry: ' + JSON.stringify(message));
    this.setState({ hideWorkQueueEntryPanel: false, workQueueEntryData: message });
  }

  /*handleBuild(message) {
    console.log('handleBuild');
    let newData = {...this.state.structureData};
    newData.state = PROGRESSING;    
    console.log(newData);
    this.setState({structureData: newData})
  }
  
  handleStructureObjUpdate(objId) {
    console.log('handleStructuredObj: ' + objId);
    console.log(Global.objectStates[objId].state);
    if(objId == this.state.structureData.id) {
      let newData = {...this.state.structureData};
      newData.state = Global.objectStates[objId].state;
      console.log(newData);
      this.setState({structureData: newData})      
    }
  }*/

  /*<img src={gatherbutton}
  id="herogatherbutton"
  className={styles.herogatherbutton}
  onClick={this.handleHeroGatherClick} /> */

  handleTransitionEnd() {
    console.log("TransitionEnd");
  }

  handleWorld(message) {
    this.setState({ worldData: message });
  }

  getAbilityHints() {
    const combatAbilities = this.state.combatState && this.state.combatState.abilities
      ? this.state.combatState.abilities.filter((ability) => ability.id != "shield_bash")
      : [];

    if (combatAbilities.length > 0) {
      return combatAbilities;
    }

    switch (Global.heroClass) {
      case "Warrior":
        return [];
      case "Ranger":
        return [
          { id: "aimed_shot", label: "Aimed Shot", cost_type: "stamina", cost: 8, range: 3, hint: "Deal reliable bow damage before enemies reach you." },
          { id: "disengage", label: "Disengage", cost_type: "stamina", cost: 8, range: 1, hint: "Step away from an adjacent enemy." },
        ];
      case "Mage":
        return [
          { id: "arcane_bolt", label: "Arcane Bolt", cost_type: "mana", cost: 20, range: 3, hint: "Spend mana for dependable ranged damage." },
          { id: "ward", label: "Ward", cost_type: "mana", cost: 15, range: 0, hint: "Raise a short magical ward." },
        ];
      default:
        return [];
    }
  }

  render() {
    console.log("styles", styles);
    const abilityHints = this.getAbilityHints();
    return (
      <div id="ui" className={styles.ui}>

        <ZoomButton />

        {!this.state.hideLoadingPanel &&
          <LoadingPanel errmsg={Global.accountName ? `Loading ${Global.accountName}...` : "Loading..."} />}

        <SmallButtonClassName handler={this.handleHeroAttrsClick}
          imageName="attrsbutton"
          className={styles.heroattrsbutton} />

        <SmallButtonClassName handler={this.handleHeroInventoryClick}
          imageName="inventorybutton"
          className={styles.heroinventorybutton} />

        <CooldownButton imageName='explorebutton'
          imageButton={explorebutton}
          handler={this.handleHeroExploreClick}
          className={styles.heroexplorebutton} />

        <GatherButton handler={this.handleHeroGatherClick}
          className={styles.herogatherbutton} />

        <SmallButtonClassName handler={this.handleHeroBuildClick}
          imageName="buildbutton"
          className={styles.herobuildbutton} />

        <ToggleButton handler={this.handleHeroSleepClick}
          imageName="resourcesbutton"
          className={styles.herosleepbutton} />

        <SmallButtonClassName handler={this.handleHeroEquipClick}
          imageName="equipbutton"
          className={styles.heroequipbutton} />

        <SmallButtonClassName handler={this.handleHeroCraftClick}
          imageName="craftbutton"
          className={styles.herocraftbutton} />

        <SmallButtonClassName handler={this.handleComboClick}
          imageName="combobutton"
          className={styles.combobutton} />

        <ActionButton type={QUICK}
          handler={this.handleQuickAttack} />

        <ActionButton type={PRECISE}
          handler={this.handlePreciseAttack} />

        <ActionButton type={FIERCE}
          handler={this.handleFierceAttack} />

        {abilityHints.length > 0 &&
          <div className={styles.abilitybar}>
            {abilityHints.map((ability) => {
              const disabled = Boolean(ability.disabled_reason);
              const title = ability.disabled_reason
                ? `${ability.label}: ${ability.disabled_reason}`
                : `${ability.label}: ${ability.hint} (${ability.cost} ${ability.cost_type})`;
              return (
                <button
                  key={ability.id}
                  type="button"
                  className={styles.abilitybutton}
                  disabled={disabled}
                  title={title}
                  onClick={() => this.handleAbilityClick(ability.id)}
                >
                  {ability.label}
                </button>
              );
            })}
          </div>
        }

        {!this.state.hideAttacksPanel &&
          <AttacksPanel attacks={Global.attacks} combatState={this.state.combatState} />}

        <img src={bracebutton}
          id="bracebutton"
          className={styles.bracebutton}
          onClick={this.handleBrace} />

        <img src={parrybutton}
          id="parrybutton"
          className={styles.parrybutton} />

        <img src={dodgebutton}
          id="dodgebutton"
          className={styles.dodgebutton} />

        <img src={movecompass}
          id="movecompass"
          ref={this.compassRef}
          className={styles.movecompass}
          onClick={this.handleMoveClick} />

        {this.state.showMoveCompassClick &&
          <img id="movecompassclick" src={movecompass_click} className={styles.movecompassclick} />}

        <HeroFrame heroStats={this.state.heroStats} hungerStatus={this.state.hungerStatus} thirstStatus={this.state.thirstStatus} fatigueStatus={this.state.fatigueStatus}></HeroFrame>

        <WorldPanel worldData={this.state.worldData} />

        <ObjectivesPanel />

        {!this.state.hideSelectPanel &&
          <SelectPanel selectedTile={this.state.selectedTile}
            objIdsOnTile={this.state.objIdsOnTile}
            selectedKey={this.state.selectedKey} />}

        {!this.state.hideTargetActionPanel &&
          <TargetActionPanel selectedBoxPos={this.state.selectedBoxPos}
            selectedKey={this.state.selectedKey} />}

        {!this.state.hideStructurePanel &&
          <StructurePanel structureData={this.state.structureData} />}

        {!this.state.hideInventoryPanel &&
          <SingleInventoryPanel left={this.state.showLeftInventoryPanel}
            inventoryData={this.state.inventoryData}
            hideExitButton={false} />}

        {!this.state.hideCraftPanel &&
          <CraftPanel crafterId={this.state.craftData.crafter_id}
            structureId={this.state.craftData.structure_id}
            items={this.state.craftData.items}
            recipesData={this.state.craftData.recipes}
            craftingItem={this.state.craftData.crafting_item} />}

        {!this.state.hideItemTransferPanel &&
          <ItemTransferPanel leftInventoryData={this.state.leftInventoryData}
            rightInventoryData={this.state.rightInventoryData}
            reqs={this.state.inventoryReqs} />}

        {!this.state.hideMerchantPanel &&
          <MerchantPanel leftInventoryData={this.state.leftInventoryData}
            rightInventoryData={this.state.rightInventoryData}
            merchantWantedItems={this.state.merchantWantedItems} />}

        {!this.state.hideEquipPanel &&
          <EquipPanel equipData={this.state.rightInventoryData} />}

        {!this.state.hideStructureRefinePanel &&
          <StructureRefinePanel 
            structureId={this.state.structureInventoryData.id}
            structureInventory={this.state.structureInventoryData}
            refineItemData={this.state.refineItemData}
            producedItemData={this.state.refineData.produced_items}
            infoRefineItemTriggered={this.state.infoRefineItemTriggered} />}

        {!this.state.hideItemPanel &&
          <ItemPanel itemData={this.state.itemData}
            triggerAction={this.state.infoItemAction}
          />}

        {!this.state.hideRefinePanel &&
          <RefinePanel refineItemData={this.state.itemData}
            refineTime={this.state.refineTime}
            producedItemData={this.state.producedItemData} />}

        {!this.state.hideHeroPanel &&
          <HeroPanel heroData={this.state.heroDetailedData} />}

        {!this.state.hideVillagerPanel &&
          <VillagerPanel villagerData={this.state.villagerData} activity={this.state.activityData} needsData={this.state.needsData} />}

        {!this.state.hideNPCPanel &&
          <NPCPanel npcData={this.state.npcData} />}

        {!this.state.hideObjPanel &&
          <ObjPanel objData={this.state.objData} />}

        {!this.state.hideAttrsPanel &&
          <AttrsPanel attrsData={this.state.attrsData} />}

        {!this.state.hideSkillsPanel &&
          <SkillsPanel skillsData={this.state.skillsData} />}

        {!this.state.hideAdvancePanel &&
          <HeroAdvancePanel advanceData={this.state.advanceData} />}

        {!this.state.hideTilePanel &&
          <TilePanel tileData={this.state.tileData} />}

        {!this.state.hideTileResourcesPanel &&
          <TileResourcesPanel tileData={this.state.tileData} />}

        {!this.state.hideTerrainFeaturePanel && this.state.tileData.terrain_features.length > 0 &&
          <TerrainFeaturePanel tileData={this.state.tileData} />}

        {!this.state.hideGatherPanel &&
          <GatherPanel selectedKey={this.state.selectedKey} />}

        {!this.state.hideBuildPanel &&
          <BuildPanel structuresData={this.state.structuresData} />}

        {!this.state.hideStructureUpgradePanel &&
          <StructureUpgradePanel upgradeData={this.state.structureUpgradeData} />}

        {!this.state.hideStructureCraftPanel &&
          <StructureCraftPanel structureId={this.state.structureInventoryData.id}
            structureInventory={this.state.structureInventoryData}
            recipesData={this.state.recipesData}
            craftingItem={this.state.craftData.crafting_item} />}

        {!this.state.hideAssignPanel &&
          <AssignPanel structureData={this.state.structureData}
            assignData={this.state.assignData} />}

        {!this.state.hideWorkQueuePanel &&
          <WorkQueuePanel structureData={this.state.structureData}
            workQueue={this.state.workQueueData} />}

        {!this.state.hideWorkQueueEntryPanel &&
          <WorkQueueEntryPanel workQueueEntryData={this.state.workQueueEntryData} />}

        {!this.state.hideItemDividePanel &&
          <ItemDividePanel itemData={this.state.itemDivideData} />}

        {!this.state.hideWantedItemPanel &&
          <WantedItemPanel wantedItemData={this.state.wantedItemData} />}

        {!this.state.hideMerchantQuantityPanel &&
          <MerchantQuantityPanel itemData={this.state.itemMerchantQuantityData}
            action={this.state.merchantAction} />}

        {!this.state.hideResourcePanel &&
          <ResourcePanel resourceData={this.state.resourceData} />}

        {!this.state.hideMerchantHirePanel &&
          <MerchantHirePanel hireData={this.state.hireData} />}

        {!this.state.hideExperimentPanel &&
          <ExperimentPanel expData={this.state.expData} />}

        {!this.state.hideNoticePanel &&
          <NoticePanel noticemsg={this.state.noticemsg} noticeExpiry={this.state.noticeExpiry} />}

        {!this.state.hideErrorPanel &&
          <ErrorPanel errmsg={this.state.errmsg} />}

        {!this.state.hideConfirmPanel &&
          <ConfirmPanel msg={this.state.confirmMsg} />}

        <HeroDeathOverlay data={this.state.heroDeathData} />

        {!this.state.hideTrueDeathPanel &&
          <TrueDeathPanel
            heroName={this.state.trueDeathData.hero_name}
            heroRank={this.state.trueDeathData.hero_rank}
            totalXp={this.state.trueDeathData.total_xp}
            scoreTotal={this.state.trueDeathData.score_total}
            scoreBreakdown={this.state.trueDeathData.score_breakdown}
            daysSurvived={this.state.trueDeathData.days_survived}
            wavesSurvived={this.state.trueDeathData.waves_survived}
            highestPressureLevel={this.state.trueDeathData.highest_pressure_level}
            legendaryKills={this.state.trueDeathData.legendary_kills}
            hideoutsCleared={this.state.trueDeathData.hideouts_cleared}
            fate={this.state.trueDeathData.fate} />
        }
      </div>
    );
  }
}


/*
          <div
            onTransitionEnd={this.handleTransitionEnd} 
            style={{display: this.state.fierceButtonDisplay,
                    position: 'fixed',
                    bottom: '10px',
                    left: "50%",
                    marginLeft: '-50px',
                    backgroundColor: 'black',
                    opacity: 0.66,
                    width: '50px',
                    height: this.state.fierceButtonHeight + 'px',
                    transition: 'height 5s'}} />

          <img src={fierceattackbutton} 
              id="fierceattackbutton"
              className={styles.fierceattackbutton}
              onClick={this.handleFierceAttack}/>



*/
