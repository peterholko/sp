import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import { Global } from "../../core/global";
import rightarrow from "ui_comp/rightarrow.png";
import '../ui.module.css';
import { FOUNDED, STALLED, NONE, CRAFT, UPGRADING, PLANNING_UPGRADE, RESOURCE, BUILDING } from "../../core/config";
import { NetworkEvent } from "../../core/networkEvent";
import { GameEvent } from "../../core/gameEvent";
import {
  MobileCard,
  MobilePanelActions,
  MobileRequirementGrid,
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

interface StructurePanelProps {
  structureData,
}

export default class StructurePanel extends React.Component<StructurePanelProps, any> {
  private timer;

  constructor(props) {
    super(props);

    // Get hero position and structure position
    const heroX = Global.objectStates[Global.heroId].x;
    const heroY = Global.objectStates[Global.heroId].y;
    const structureX = this.props.structureData.x;
    const structureY = this.props.structureData.y;

    // Get hero state 
    const heroState = Global.objectStates[Global.heroId].state;

    var refineButtonSelected = false;

    // If hero is on the same tile as the structure and state is refining, set refineButtonSelected to true
    if (heroX == structureX && heroY == structureY && heroState == "refining") {
      refineButtonSelected = true;
    }

    var workDone = 0;
    var workPerSecond = 0;
    var buildUpgradeCost = 0;

    if ('work_done' in this.props.structureData) {
      workDone = this.props.structureData.work_done;
    }

    if ('work_per_sec' in this.props.structureData) {
      workPerSecond = this.props.structureData.work_per_sec;
    }

    if (this.props.structureData.state == PLANNING_UPGRADE || this.props.structureData.state == UPGRADING) {
      buildUpgradeCost = this.props.structureData.upgrade_cost;
    } else {
      buildUpgradeCost = this.props.structureData.build_cost;
    }

    this.state = {
      buildUpgradeCost: buildUpgradeCost,
      workDone: workDone,
      workPerSecond: workPerSecond,
      structureData: this.props.structureData,
      refineButtonSelected: refineButtonSelected
    };

    this.handleCraftClick = this.handleCraftClick.bind(this);
    this.handleQueueClick = this.handleQueueClick.bind(this);
    this.handleOperateClick = this.handleOperateClick.bind(this);
    this.handleRefineClick = this.handleRefineClick.bind(this);
    this.handleBuildClick = this.handleBuildClick.bind(this);
    this.handleAssignClick = this.handleAssignClick.bind(this);
    this.handleDeleteClick = this.handleDeleteClick.bind(this);
    this.handleExperimentClick = this.handleExperimentClick.bind(this);
    this.handleStartUpgradeClick = this.handleStartUpgradeClick.bind(this);
    this.handlePlantClick = this.handlePlantClick.bind(this);
    this.handleTendClick = this.handleTendClick.bind(this);
    this.handleHarvestClick = this.handleHarvestClick.bind(this);
    this.handleSendDelete = this.handleSendDelete.bind(this);
    this.handleResponseUpgrade = this.handleResponseUpgrade.bind(this);
    this.handleCampfireClick = this.handleCampfireClick.bind(this);
    this.handleSleepClick = this.handleSleepClick.bind(this);

    this.startTimer = this.startTimer.bind(this)
    this.stopTimer = this.stopTimer.bind(this)

    Global.gameEmitter.on(NetworkEvent.WORK_UPDATE, this.handleNetworkWorkUpdate, this);
    Global.gameEmitter.on(NetworkEvent.UPGRADE, this.handleResponseUpgrade, this);
    Global.gameEmitter.on(GameEvent.CONFIRM_OK_CLICK, this.handleSendDelete, this);
  }

  componentWillUnmount() {
    this.stopTimer();
    Global.gameEmitter.removeListener(NetworkEvent.WORK_UPDATE, this.handleNetworkWorkUpdate);
    Global.gameEmitter.removeListener(NetworkEvent.UPGRADE, this.handleResponseUpgrade);
    Global.gameEmitter.removeListener(GameEvent.CONFIRM_OK_CLICK, this.handleSendDelete);
  }

  componentDidMount() {
    if (this.props.structureData.state == BUILDING ||
      this.props.structureData.state == UPGRADING) {
      this.startTimer();
    }
  }

  componentDidUpdate() {
    console.log("componentDidUpdate" + JSON.stringify(this.props.structureData));
  }

  handleStartUpgradeClick() {
    Global.network.sendInfoUpgrade(this.state.structureData.id);
  }

  handleCraftClick() {
    Global.network.sendInfoStructureCraft(this.state.structureData.id);
    //Global.gameEmitter.emit(GameEvent.STRUCTURE_CRAFT_CLICK, {structureData: this.state.structureData});
  }

  handleQueueClick() {
    Global.network.sendInfoStructureQueue(this.state.structureData.id);
  }

  handleOperateClick() {
    Global.network.sendOperate(this.state.structureData.id);
    Global.gameEmitter.emit(GameEvent.OPERATE_CLICK, {});
  }

  handleRefineClick() {
    // If refineButtonSelected in state is false, the button has been clicked to activate refine
    var refineButtonActivated = this.state.refineButtonSelected ? false : true;

    /*Global.network.sendRefine(this.state.structureData.id, refineButtonActivated);
    this.setState({refineButtonSelected: refineButtonActivated});
    Global.gameEmitter.emit(GameEvent.REFINE_CLICK, {structureData: this.state.structureData, refineButtonActivated: refineButtonActivated});*/
    Global.network.sendInfoStructureRefine(this.state.structureData.id);
  }

  handleExperimentClick() {
    Global.network.sendInfoExperiment(this.state.structureData.id);
  }

  handleBuildClick() {
    Global.network.sendBuild(Global.heroId, this.state.structureData.id);
  }

  handleUpgradeClick() {
    Global.network.sendUpgrade(Global.heroId, this.state.structureData.id);
  }

  handleAssignClick() {
    Global.network.sendInfoAssign(this.state.structureData.id);
  }

  handlePlantClick() {
    Global.network.sendPlant(this.state.structureData.id);
  }

  handleTendClick() {
    Global.network.sendTend(this.state.structureData.id);
  }

  handleHarvestClick() {
    Global.network.sendHarvest(this.state.structureData.id);
  }

  handleCampfireClick() {
    Global.network.sendActivate(this.state.structureData.id);
  }


  handleSleepClick() {
    Global.network.sendSleep(this.state.structureData.id);
  }

  handleSendDelete() {
    Global.network.sendDelete(this.state.structureData.id);

    //To hide the structure panel
    Global.gameEmitter.emit(GameEvent.DELETE_STRUCTURE_CLICK, {});
  }

  handleDeleteClick() {
    console.log('Delete Structure');

    const event = {
      msg: 'Remove the structure?',
      type: 'delete_structure'
    };

    Global.gameEmitter.emit(GameEvent.CONFIRMATION, event);
  }

  handleNetworkWorkUpdate(message) {
    console.log('Network work update');

    this.setState({ workDone: message.work_done, workPerSecond: message.work_per_sec });
    this.startTimer();
  }

  handleResponseUpgrade(message) {
    console.log('Response Upgrade');
    const upgradeTimeSeconds = Math.floor(message.upgrade_time);

    this.setState({
      progress: 0,
      maxProgress: upgradeTimeSeconds / 10
    });
    this.startTimer();
  }

  startTimer = () => {
    if (this.timer) return; // prevent duplicates

    this.timer = setInterval(() => {
      this.setState(prevState => {
        if (prevState.workDone >= prevState.buildUpgradeCost) {
          this.stopTimer();
          Global.network.sendInfoObj(prevState.structureData.id);
          return null;
        }

        return {
          workDone: prevState.workDone + prevState.workPerSecond
        };
      });
    }, 1000);
  };

  stopTimer() {
    clearInterval(this.timer);
    this.timer = null;
  }

  render() {
    //console.log('Rendering Structure Panel...');
    //console.log(this.props.structureData);

    const isFarm = this.props.structureData.subclass == 'farm';
    const isResource = this.props.structureData.subclass == 'resource';
    const isShelter = this.props.structureData.subclass == 'shelter';

    const isTent = this.props.structureData.template == 'Tent';
    const isCampfire = this.props.structureData.template == 'Campfire';

    const showQueueButton = (this.props.structureData.state == NONE &&
      (this.props.structureData.subclass == CRAFT ||
        this.props.structureData.subclass == RESOURCE));

    const showOperateButton = (this.props.structureData.state == NONE && isResource);

    const showCraftButton = (this.props.structureData.state == NONE &&
      this.props.structureData.subclass == CRAFT);

    const showRefineButton = (this.props.structureData.state == NONE &&
      this.props.structureData.subclass == CRAFT);

    const showExperimentButton = (this.props.structureData.state == NONE &&
      this.props.structureData.subclass == CRAFT &&
      this.props.structureData.level != -1);

    const showBuildButton = (this.props.structureData.state == FOUNDED ||
      this.props.structureData.state == BUILDING);

    const showUpgradeButton = (this.props.structureData.state == PLANNING_UPGRADE ||
      this.props.structureData.state == UPGRADING);

    const showProgress = (this.props.structureData.state == FOUNDED ||
      this.props.structureData.state == BUILDING ||
      this.props.structureData.state == STALLED ||
      this.props.structureData.state == UPGRADING)

    const showAssignButton = true;
    const showStartUpgradeButton = this.props.structureData.state == NONE && this.props.structureData.upgradeable;

    const showPlantButton = (this.props.structureData.state == NONE && isFarm);
    const showTendButton = (this.props.structureData.state == NONE && isFarm);
    const showHarvestButton = (this.props.structureData.state == NONE && isFarm);

    const showCampfireButton = (this.props.structureData.state == NONE && (isTent || isCampfire));
    const showSleepButton = (this.props.structureData.state == NONE && isTent);

    const isFinished = this.props.structureData.state == NONE;

    const isUpgrading = this.props.structureData.state == PLANNING_UPGRADE || this.props.structureData.state == UPGRADING;

    let progressLabel = "Build";

    let stateText = this.props.structureData.state;

    if (this.props.structureData.state == PLANNING_UPGRADE) {
      stateText = "Upgrading (need resources)";
    }

    if (this.props.structureData.state == UPGRADING) {
      progressLabel = "Upgrade";
    }

    var imageName = '';
    var upgradeToImageName = '';

    if (this.props.structureData.props == 'founded') {
      imageName = 'foundation.png'
    } else {
      imageName = this.props.structureData.image + '.png';
    }

    if (isUpgrading) {

      if (this.props.structureData.selected_upgrade) {
        upgradeToImageName = this.props.structureData.selected_upgrade.toLowerCase().replace(/\s/g, '') + '.png';
      } else {
        upgradeToImageName = Global.selectedUpgrade.toLowerCase().replace(/\s/g, '') + '.png';
      }
    }

    const reqs = [];
    const upgradeReqs = [];

    /*if (this.props.structureData.state == PLANNING_UPGRADE) {
      if (this.props.structureData.hasOwnProperty('upgrade_req')) {
        for (var i = 0; i < this.props.structureData.upgrade_req.length; i++) {
          var upgrade_req = this.props.structureData.upgrade_req[i];

          reqs.push(
            <ResourceItem key={i}
              resourceName={upgrade_req.type}
              quantity={upgrade_req.quantity}
              index={i}
              showQuantity={true} />
          )
        }
      }
    } else { */
    if (this.props.structureData.hasOwnProperty('req')) {
      for (var i = 0; i < this.props.structureData.req.length; i++) {
        var req = this.props.structureData.req[i];

        reqs.push(req)
      }
    }

    if (this.props.structureData.hasOwnProperty('upgrade_req')) {
      for (var i = 0; i < this.props.structureData.upgrade_req.length; i++) {
        var req = this.props.structureData.upgrade_req[i];

        upgradeReqs.push(req)
      }
    }

    console.log("buildUpgradeCost: " + this.state.buildUpgradeCost);
    console.log("workDone: " + this.state.workDone);
    console.log("workPerSecond: " + this.state.workPerSecond);

    const landscape = isLandscapeMobile();
    const panelTitle = isUpgrading ? `Upgrading to ${this.props.structureData.selected_upgrade || Global.selectedUpgrade}` : `${this.props.structureData.name} Level ${this.props.structureData.level}`;
    const activeRequirements = isUpgrading ? upgradeReqs : reqs;
    const progress = <progress max={this.state.buildUpgradeCost} value={this.state.workDone}>{this.state.workDone}</progress>;
    const iconPath = (name: string) => name.indexOf('.') != -1 ? `/static/art/ui/${name}` : `/static/art/ui/${name}.png`;
    const actions = [
      showQueueButton && { key: 'queue', label: 'Queue', icon: iconPath('queuebutton'), onClick: this.handleQueueClick },
      showOperateButton && { key: 'operate', label: 'Operate', icon: iconPath('gatherbutton'), onClick: this.handleOperateClick },
      showStartUpgradeButton && { key: 'start-upgrade', label: 'Start upgrade', icon: iconPath('upgradebutton'), onClick: this.handleStartUpgradeClick },
      showExperimentButton && { key: 'experiment', label: 'Experiment', icon: iconPath('experimentbutton'), onClick: this.handleExperimentClick },
      showCraftButton && { key: 'craft', label: 'Craft', icon: iconPath('craftbutton'), onClick: this.handleCraftClick },
      showRefineButton && { key: 'refine', label: 'Refine', icon: iconPath('refinebutton'), onClick: this.handleRefineClick, selected: this.state.refineButtonSelected },
      showBuildButton && { key: 'build', label: 'Build', icon: iconPath('buildbutton'), onClick: this.handleBuildClick },
      showUpgradeButton && { key: 'upgrade', label: 'Upgrade', icon: iconPath('upgradebutton'), onClick: this.handleStartUpgradeClick },
      showAssignButton && { key: 'assign', label: 'Assign', icon: iconPath('assignbutton'), onClick: this.handleAssignClick },
      showPlantButton && { key: 'plant', label: 'Plant', icon: iconPath('plant.png'), onClick: this.handlePlantClick },
      showTendButton && { key: 'tend', label: 'Tend', icon: iconPath('plant.png'), onClick: this.handleTendClick },
      showHarvestButton && { key: 'harvest', label: 'Harvest', icon: iconPath('plant.png'), onClick: this.handleHarvestClick },
      showCampfireButton && { key: 'campfire', label: 'Campfire', icon: iconPath('sleepbutton'), onClick: this.handleCampfireClick },
      showSleepButton && { key: 'sleep', label: 'Sleep', icon: iconPath('sleepbutton'), onClick: this.handleSleepClick },
      { key: 'delete', label: 'Delete', icon: iconPath('deletebutton'), onClick: this.handleDeleteClick },
    ].filter(Boolean);

    const upgradeVisualStyle: React.CSSProperties = {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      gap: '8px',
    };

    const smallImageStyle: React.CSSProperties = {
      width: landscape ? '48px' : '64px',
      height: landscape ? '48px' : '64px',
      objectFit: 'contain',
      imageRendering: 'pixelated',
    };

    const arrowStyle: React.CSSProperties = {
      width: '32px',
      height: '32px',
      objectFit: 'contain',
    };

    return (
      <MobilePanelScreen
        panelType={'structure'}
        title={'Structure'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={
            <>
              {isUpgrading
                ? <MobileCard compact={landscape}>
                  <div style={upgradeVisualStyle}>
                    <img src={'/static/art/' + imageName} style={smallImageStyle} />
                    <img src={rightarrow} style={arrowStyle} />
                    <img src={'/static/art/' + upgradeToImageName} style={smallImageStyle} />
                  </div>
                </MobileCard>
                : <MobileSummaryCard imageSrc={'/static/art/' + imageName} title={panelTitle} imageSize={landscape ? 58 : 82} />}
              <MobileStatsList rows={[
                { label: 'State', value: stateText },
                { label: 'Class', value: this.props.structureData.subclass },
                { label: 'HP', value: `${this.props.structureData.hp} / ${this.props.structureData.base_hp}`, hidden: !isFinished },
                { label: 'Defense', value: this.props.structureData.base_def, hidden: !isFinished },
                { label: 'Residents', value: `${this.props.structureData.residents} / ${this.props.structureData.max_residents}`, hidden: !(isFinished && isShelter) },
                { label: `${progressLabel} Cost`, value: this.state.buildUpgradeCost, hidden: isFinished },
                { label: `${progressLabel} Progress`, value: progress, hidden: !showProgress },
                { label: 'Crop Type', value: this.props.structureData.crop_type, hidden: !(isFinished && isFarm) },
                { label: 'Crop Qty', value: this.props.structureData.crop_quantity, hidden: !(isFinished && isFarm) },
                { label: 'Crop Stage', value: this.props.structureData.crop_stage, hidden: !(isFinished && isFarm) },
              ]} />
            </>
          }
          right={
            <>
              {activeRequirements.length > 0 &&
                <MobileRequirementGrid title="Requirements" requirements={activeRequirements} showCurrent={true} />}
              <MobilePanelActions actions={actions as any} align="start" />
            </>
          } />
      </MobilePanelScreen>
    );
  }
}

