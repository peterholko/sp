
import * as React from "react";
import HalfPanel from "./halfPanel";
import { Global } from "../../core/global";
import ResourceItem from "./resourceItem";
import styles from "./../ui.module.css";
import SmallButtonClassName from "./smallButtonClassName";
import { GameEvent } from "../../core/gameEvent";
import { NetworkEvent } from "../../core/networkEvent";
interface TilePanelProps {
  tileData,
}

export default class TilePanel extends React.Component<TilePanelProps, any> {
  private timer: any = null;

  constructor(props) {
    super(props);

    this.state = {
      prospecting: false,
      prospectProgress: 0,
      prospectMax: 0,
    };

    this.handleResourceButtonClick = this.handleResourceButtonClick.bind(this);
    this.handleProspectButtonClick = this.handleProspectButtonClick.bind(this);
    this.handleProspect = this.handleProspect.bind(this);
    this.startProspectTimer = this.startProspectTimer.bind(this);
    this.stopProspectTimer = this.stopProspectTimer.bind(this);
  }

  componentDidMount() {
    Global.gameEmitter.on(NetworkEvent.PROSPECT, this.handleProspect, this);
  }

  componentWillUnmount() {
    Global.gameEmitter.removeListener(NetworkEvent.PROSPECT, this.handleProspect);
    this.stopProspectTimer();
  }

  componentDidUpdate(prevProps) {
    // Switched to a different tile: clear any in-progress prospecting bar.
    if (prevProps.tileData.x !== this.props.tileData.x ||
        prevProps.tileData.y !== this.props.tileData.y) {
      this.stopProspectTimer();
      if (this.state.prospecting) {
        this.setState({ prospecting: false, prospectProgress: 0, prospectMax: 0 });
      }
    }
  }

  handleProspect(message) {
    const hero = Global.objectStates[Global.heroId];
    // Only show the progress bar on the tile actually being prospected.
    if (!hero || hero.x !== this.props.tileData.x || hero.y !== this.props.tileData.y) {
      return;
    }

    // prospect_time / explore_time is in game ticks (10 ticks per second).
    const ticks = message.prospect_time ?? message.explore_time ?? 20;
    this.startProspectTimer(ticks);
  }

  startProspectTimer(ticks) {
    this.stopProspectTimer();
    this.setState({ prospecting: true, prospectProgress: 0, prospectMax: ticks });

    // Advance once per tick (~100ms) so the bar fills over the prospect duration.
    this.timer = setInterval(() => {
      if (this.state.prospectProgress >= this.state.prospectMax) {
        this.stopProspectTimer();
        this.setState({ prospecting: false, prospectProgress: 0, prospectMax: 0 });
      } else {
        this.setState({ prospectProgress: this.state.prospectProgress + 1 });
      }
    }, 100);
  }

  stopProspectTimer() {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  handleResourceButtonClick(event: React.MouseEvent) {
    console.log('handleResourceButtonClick');
    Global.gameEmitter.emit(GameEvent.RESOURCE_BUTTON_CLICK, {});
  }

  handleProspectButtonClick(event: React.MouseEvent) {
    const hero = Global.objectStates[Global.heroId];
    if (hero && (hero.x != this.props.tileData.x || hero.y != this.props.tileData.y)) {
      Global.gameEmitter.emit(NetworkEvent.NOTICE, {
        noticemsg: "Move onto this tile to prospect it."
      });
      return;
    }

    Global.network.sendProspect();
  }

  render() {
    const x = this.props.tileData.x;
    const y = this.props.tileData.y;
    const tileIndex = x + '_' + y; 
    const tileState = Global.tileStates[tileIndex];
    const tiles = [...tileState.tiles]; //Deep copy
    const resources = []

    let numResources = this.props.tileData.resources.length + 
                       this.props.tileData.unrevealed;

    let discoveredResources = this.props.tileData.resources.length;

    //The default Grass was "above" forest, solved it via sort
    var tileId = tiles.sort().reverse()[0];
    var imageName = Global.tileset[tileId].image;

    var passable = (this.props.tileData.passable ? 'Yes' : 'No');
    var movementCost = String(this.props.tileData.mc * 100);
    movementCost = movementCost + '%';
    var sanctuary = (this.props.tileData.sanctuary ? 'Yes' : 'No');
    var tileStyle;

    for(var i = 0; i < this.props.tileData.resources.length; i++) {
      var resource = this.props.tileData.resources[i];

      resources.push(
        <ResourceItem key={i}
                      resourceName={resource.name}
                      resourceImage={resource.image}
                      yieldLabel={resource.yield_label}
                      quantityLabel={resource.quantity_label}
                      quantity={0}
                      properties={resource.properties}
                      index={i}                      
                      showQuantity={false}/>
      )
    }

    //Manual size adjustments
    if(tileId == 19) {
      tileStyle = {
        transform: 'translate(-205px, 10px)',
        width: '110px',
        position: 'fixed'
      } as React.CSSProperties
    } else if (tileId == 32) {
      tileStyle = {
        transform: 'translate(-225px, -20px)',
        width: '150px',
        position: 'fixed'
      } as React.CSSProperties
    }
    else {
      tileStyle = {
        transform: 'translate(-185px, 25px)',
        position: 'fixed'
      } as React.CSSProperties
    }

    const tableStyle = {
      transform: 'translate(20px, -220px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const resDivStyle = {
      transform: 'translate(15px, -90px)',
      position: 'fixed',
    } as React.CSSProperties

    const spanNameStyle = {
      transform: 'translate(-323px, 110px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

        return (
      <HalfPanel left={true} 
                 panelType={'tile'} 
                 hideExitButton={false}>
        <img src={'/static/art/' + imageName} style={tileStyle} />
        <span style={spanNameStyle}>{this.props.tileData.name} ({x}, {y})</span>
        <table style={tableStyle}>
          <tbody>
          <tr>
            <td>Passable: </td>
            <td>{passable}</td>
          </tr>
          <tr>
            <td>Movement Cost: </td>
            <td>{movementCost}</td>
          </tr>
          <tr>
            <td>Defense Bonus: </td>
            <td>{this.props.tileData.def}</td>
          </tr>
          <tr>
            <td>Sanctuary: </td>
            <td>{sanctuary}</td>
          </tr>
          <tr>
            <td>Wildness: </td>
            <td>{this.props.tileData.wildness}</td>
          </tr>
          <tr>
            <td>Survey Status: </td>
            <td>{this.props.tileData.survey_status || 'Unsurveyed'}</td>
          </tr>
          <tr>
            <td>Prospected Resources: </td>
            <td>{discoveredResources} / {numResources}</td>
          </tr>
          {this.state.prospecting &&
          <tr>
            <td>Prospecting: </td>
            <td>
              <progress style={{ width: '120px' }}
                        max={this.state.prospectMax}
                        value={this.state.prospectProgress}></progress>
            </td>
          </tr>}
          <tr>
            <td>
              <SmallButtonClassName handler={this.handleProspectButtonClick}
                imageName="explorebutton"
                className={styles.tilepanelprospectbutton}
                title="Prospect" />
              <SmallButtonClassName handler={this.handleResourceButtonClick}
                imageName="resourcesbutton"
                className={styles.tilepanelresourcebutton}
                title="Discovered Resources" />
            </td>
          </tr>
          </tbody>
        </table>
      </HalfPanel>
    );
  }
}

/*
            <td>Resources Found: </td>
            <td>
              <table style={tableStyle2}>
                <tbody>
                  {resources}
                </tbody>
              </table>
            </td>
*/
