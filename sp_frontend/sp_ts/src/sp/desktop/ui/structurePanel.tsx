import * as React from "react";
import HalfPanel from "./halfPanel";
import { Global } from "../../core/global";
import rightarrow from "ui_comp/rightarrow.png";
import '../ui.module.css';
import { FOUNDED, STALLED, NONE, CRAFT, UPGRADING, PLANNING_UPGRADE, RESOURCE, BUILDING } from "../../core/config";
import { NetworkEvent } from "../../core/networkEvent";
import { GameEvent } from "../../core/gameEvent";
import ResourceItem from "./resourceItem";
import SmallButton from "./smallButton";
import ToggleLinkedButton from './toggleLinkedButton';

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

        var resourceImage = req.type.toLowerCase().replace(/\s/g, '');

        reqs.push(
          <ResourceItem key={i}
            index={i}
            resourceName={req.type}
            resourceImage={resourceImage}
            quantity={req.quantity}
            currentQuantity={req.cquantity}
            showQuantity={true} />
        )
      }
    }

    if (this.props.structureData.hasOwnProperty('upgrade_req')) {
      for (var i = 0; i < this.props.structureData.upgrade_req.length; i++) {
        var req = this.props.structureData.upgrade_req[i];

        var resourceImage = req.type.toLowerCase().replace(/\s/g, '');

        upgradeReqs.push(
          <ResourceItem key={i}
            index={i}
            resourceName={req.type}
            resourceImage={resourceImage}
            quantity={req.quantity}
            currentQuantity={req.cquantity}
            showQuantity={true} />
        )
      }
    }


    const imageStyle = {
      transform: 'translate(-195px, 25px)',
      position: 'fixed'
    } as React.CSSProperties

    const imageUpgradingStyle = {
      transform: 'translate(-272px, 25px)',
      position: 'fixed'
    } as React.CSSProperties

    const rightStyle = {
      transform: 'translate(-195px, 35px)',
      position: 'fixed'
    } as React.CSSProperties

    const upgradeToStyle = {
      transform: 'translate(-138px, 25px)',
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
      transform: 'translate(20px, -230px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const divReqsStyle = {
      transform: 'translate(-100px, 15px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const startUpgradeStyle = {
      transform: 'translate(-250px, 38px)',
      position: 'fixed'
    } as React.CSSProperties

    const queueStyle = {
      transform: 'translate(-312px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const experimentStyle = {
      transform: 'translate(-262px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const plantStyle = {
      transform: 'translate(-262px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const craftStyle = {
      transform: 'translate(-212px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const tendStyle = {
      transform: 'translate(-212px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const refineStyle = {
      transform: 'translate(-162px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const operateStyle = {
      transform: 'translate(-162px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const harvestStyle = {
      transform: 'translate(-162px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const buildStyle = {
      transform: 'translate(-162px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const upgradeStyle = {
      transform: 'translate(-162px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const assignStyle = {
      transform: 'translate(-112px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const deleteStyle = {
      transform: 'translate(-62px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const campfireStyle = {
      transform: 'translate(-162px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const sleepStyle = {
      transform: 'translate(-212px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    console.log("buildUpgradeCost: " + this.state.buildUpgradeCost);
    console.log("workDone: " + this.state.workDone);
    console.log("workPerSecond: " + this.state.workPerSecond);

    return (
      <HalfPanel left={true}
        panelType={'structure'}
        hideExitButton={false}>

        {isUpgrading &&
          <span>
            <img src={'/static/art/' + imageName} style={imageUpgradingStyle} />
            <img src={rightarrow} style={rightStyle} />
            <img src={'/static/art/' + upgradeToImageName} style={upgradeToStyle} />
          </span>
        }

        {isUpgrading &&
          <span style={spanNameStyle}>
            Upgrading to {this.props.structureData.selected_upgrade}
          </span>
        }

        {!isUpgrading &&
          <img src={'/static/art/' + imageName} style={imageStyle} />
        }

        {!isUpgrading &&
          <span style={spanNameStyle}>
            {this.props.structureData.name} Level {this.props.structureData.level}
          </span>
        }

        <table style={tableStyle}>
          <tbody>

            <tr>
              <td>State:</td>
              <td>{stateText}</td>
            </tr>
            <tr>
              <td>Class:</td>
              <td>{this.props.structureData.subclass}</td>
            </tr>

            {isFinished &&
              <tr>
                <td>HP:</td>
                <td>{this.props.structureData.hp} / {this.props.structureData.base_hp}</td>
              </tr>
            }

            {isFinished &&
              <tr>
                <td>Defense:</td>
                <td>{this.props.structureData.base_def}</td>
              </tr>
            }

            {isFinished && isShelter &&
              <tr>
                <td>Residents:</td>
                <td>{this.props.structureData.residents} / {this.props.structureData.max_residents}</td>
              </tr>
            }

            {!isFinished &&
              <tr>
                <td>{progressLabel} Cost:</td>
                <td>{this.state.buildUpgradeCost}</td>
              </tr>
            }

            {showProgress &&
              <tr>
                <td>{progressLabel} Progress: </td>
                <td><progress max={this.state.buildUpgradeCost}
                  value={this.state.workDone}>{this.state.workDone}
                </progress></td>
              </tr>
            }

            {!isFinished && !isUpgrading &&
              <tr>
                <td>Requirements:</td>
                <td>
                  <div style={divReqsStyle}>
                    {reqs}
                  </div>
                </td>
              </tr>
            }

            {isUpgrading &&
              <tr>
                <td>Requirements:</td>
                <td>
                  <div style={divReqsStyle}>
                    {upgradeReqs}
                  </div>
                </td>
              </tr>
            }

            {(isFinished && isFarm) &&
              <tr>
                <td>Crop Type:</td>
                <td>{this.props.structureData.crop_type}</td>
              </tr>
            }

            {(isFinished && isFarm) &&
              <tr>
                <td>Crop Quantity:</td>
                <td>{this.props.structureData.crop_quantity}</td>
              </tr>
            }

            {(isFinished && isFarm) &&
              <tr>
                <td>Crop Stage:</td>
                <td>{this.props.structureData.crop_stage}</td>
              </tr>
            }
          </tbody>
        </table>

        {showQueueButton &&
          <SmallButton handler={this.handleQueueClick}
            imageName="queuebutton"
            style={queueStyle} />}

        {showOperateButton &&
          <SmallButton handler={this.handleOperateClick}
            imageName="gatherbutton"
            style={operateStyle} />}

        {showStartUpgradeButton &&
          <SmallButton handler={this.handleStartUpgradeClick}
            imageName="upgradebutton"
            style={startUpgradeStyle} />}

        {showExperimentButton &&
          <SmallButton handler={this.handleExperimentClick}
            imageName="experimentbutton"
            style={experimentStyle} />}


        {showCraftButton &&
          <SmallButton handler={this.handleCraftClick}
            imageName="craftbutton"
            style={craftStyle} />}

        {showRefineButton &&
          <ToggleLinkedButton handler={this.handleRefineClick}
            imageName="refinebutton"
            style={refineStyle}
            displayInline={true}
            toggleIconBorder={this.state.refineButtonSelected} />}

        {showBuildButton &&
          <SmallButton handler={this.handleBuildClick}
            imageName="buildbutton"
            style={buildStyle} />}

        {showUpgradeButton &&
          <SmallButton handler={this.handleStartUpgradeClick}
          imageName="upgradebutton"
          style={upgradeStyle} />}        

        {showAssignButton &&
          <SmallButton handler={this.handleAssignClick}
            imageName="assignbutton"
            style={assignStyle} />}

        {showPlantButton &&
          <SmallButton handler={this.handlePlantClick}
            imageName="plantbutton"
            style={plantStyle} />}

        {showTendButton &&
          <SmallButton handler={this.handleTendClick}
            imageName="tendbutton"
            style={tendStyle} />}

        {showHarvestButton &&
          <SmallButton handler={this.handleHarvestClick}
            imageName="harvestbutton"
            style={harvestStyle} />}

        {showCampfireButton &&
          <SmallButton handler={this.handleCampfireClick}
            imageName="campfirebutton"
            style={campfireStyle} />}

        {showSleepButton &&
          <SmallButton handler={this.handleSleepClick}
            imageName="sleepbutton"
            style={sleepStyle} />}

        <SmallButton handler={this.handleDeleteClick}
          imageName="deletebutton"
          style={deleteStyle} />
      </HalfPanel>
    );
  }
}



