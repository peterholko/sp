
import * as React from "react";
import HalfPanel from "./halfPanel";
import { Global } from "../../core/global";
import ResourceItem from "./resourceItem";
import styles from "./../ui.module.css";
import SmallButtonClassName from "./smallButtonClassName";
import { GameEvent } from "../../core/gameEvent";
import { NetworkEvent } from "../../core/networkEvent";
import WorkQueueProgressBar from "../../core/workQueueProgressBar";
import { prospectingProgressForTile } from "../../core/prospectingProgress";
import { terrainFeatureButtonTitle } from "../../core/terrainFeaturePresentation";
interface TilePanelProps {
  tileData,
}

export default class TilePanel extends React.Component<TilePanelProps, any> {
  constructor(props) {
    super(props);

    this.handleResourceButtonClick = this.handleResourceButtonClick.bind(this);
    this.handleTerrainFeatureButtonClick = this.handleTerrainFeatureButtonClick.bind(this);
    this.handleProspectButtonClick = this.handleProspectButtonClick.bind(this);
  }

  handleResourceButtonClick(event: React.MouseEvent) {
    console.log('handleResourceButtonClick');
    Global.gameEmitter.emit(GameEvent.RESOURCE_BUTTON_CLICK, {});
  }

  handleTerrainFeatureButtonClick(event: React.MouseEvent) {
    Global.gameEmitter.emit(GameEvent.TERRAIN_FEATURE_BUTTON_CLICK, {});
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
    const prospectingProgress = prospectingProgressForTile(
      Global.objectStates[Global.heroId],
      x,
      y,
    );
    const terrainFeatureTitle = terrainFeatureButtonTitle(this.props.tileData);
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
    const isForestTile = imageName.indexOf('tileset/forest/') !== -1;

    var passable = (this.props.tileData.passable ? 'Yes' : 'No');
    var movementCost = String(this.props.tileData.mc * 100);
    movementCost = movementCost + '%';
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
    if(isForestTile) {
      tileStyle = {
        transform: 'translate(-205px, 10px)',
        width: '110px',
        height: '110px',
        objectFit: 'contain',
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
            <td>Wildness: </td>
            <td>{this.props.tileData.wildness}</td>
          </tr>
          <tr>
            <td>Scout Status: </td>
            <td>{this.props.tileData.survey_status || 'Unscouted'}</td>
          </tr>
          <tr>
            <td>Prospected Resources: </td>
            <td>{discoveredResources} / {numResources}</td>
          </tr>
          {prospectingProgress &&
          <tr>
            <td>Prospecting: </td>
            <td>
              <WorkQueueProgressBar
                {...prospectingProgress}
                style={{ width: '120px' }}
                label="Prospecting progress" />
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
              {terrainFeatureTitle &&
              <SmallButtonClassName handler={this.handleTerrainFeatureButtonClick}
                imageName="terrainfeaturebutton"
                className={styles.tilepanelfeaturebutton}
                title={terrainFeatureTitle} />}
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
