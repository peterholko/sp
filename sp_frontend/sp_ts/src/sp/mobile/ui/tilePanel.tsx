
import * as React from "react";
import { Global } from "../../core/global";
import MobilePanelScreen from "./mobilePanelScreen";
import explorebutton from "ui_comp/explorebutton.png";
import resourcesbutton from "ui_comp/resourcesbutton.png";
import { GameEvent } from "../../core/gameEvent";
import { NetworkEvent } from "../../core/networkEvent";
import {
  MobilePanelActions,
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";
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
    const landscape = isLandscapeMobile();
    const imageSize = tileId == 32 ? (landscape ? 86 : 120) : tileId == 19 ? (landscape ? 76 : 100) : (landscape ? 58 : 82);

        return (
      <MobilePanelScreen
        panelType={'tile'}
        title={'Tile'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={<MobileSummaryCard imageSrc={'/static/art/' + imageName} title={`${this.props.tileData.name} (${x}, ${y})`} imageSize={imageSize} />}
          right={
            <>
              <MobileStatsList rows={[
                { label: 'Passable', value: passable },
                { label: 'Movement', value: movementCost },
                { label: 'Defense', value: this.props.tileData.def },
                { label: 'Sanctuary', value: sanctuary },
                { label: 'Wildness', value: this.props.tileData.wildness },
                { label: 'Survey Status', value: this.props.tileData.survey_status || 'Unsurveyed' },
                { label: 'Prospected Resources', value: `${discoveredResources} / ${numResources}` },
                {
                  label: 'Prospecting',
                  value: <progress style={{ width: '120px' }}
                                   max={this.state.prospectMax}
                                   value={this.state.prospectProgress}></progress>,
                  hidden: !this.state.prospecting,
                },
              ]} />
              <MobilePanelActions actions={[
                { key: 'prospect', label: 'Prospect', icon: explorebutton, onClick: this.handleProspectButtonClick },
                { key: 'resources', label: 'Discovered Resources', icon: resourcesbutton, onClick: this.handleResourceButtonClick },
              ]} align="start" />
            </>
          } />
      </MobilePanelScreen>
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
