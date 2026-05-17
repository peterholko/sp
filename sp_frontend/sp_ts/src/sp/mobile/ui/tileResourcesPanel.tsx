
import * as React from "react";
import { Global } from "../../core/global";
import MobilePanelScreen from "./mobilePanelScreen";
import { GameEvent } from "../../core/gameEvent";
import {
  MobileCard,
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
  resourceImageForName,
} from "./mobilePanelLayout";

interface TRPProps {
  tileData
}

export default class TileResourcesPanel extends React.Component<TRPProps, any> {
  constructor(props) {
    super(props);

    Global.selectedItemId = -1;
    Global.selectedItemOwnerId = -1;  

    this.state = {
      selectedIndex: null
    };
    
    Global.gameEmitter.on(GameEvent.RESOURCE_CLICK, this.handleResourceClick, this);
  }

  handleResourceClick(eventData) {
    console.log('handleSelect ' + eventData);

    this.setState({
      selectedIndex: eventData.index
    });
  }

  render() {
    const landscape = isLandscapeMobile();
    const resources = this.props.tileData.resources || [];
    const tileSize = landscape ? 58 : 64;

    const gridStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: `repeat(auto-fill, ${tileSize}px)`,
      gridAutoRows: `${tileSize}px`,
      gap: landscape ? '6px' : '8px',
      justifyContent: 'start',
      alignItems: 'start',
    };

    const buttonStyle = (selected: boolean): React.CSSProperties => ({
      width: `${tileSize}px`,
      height: `${tileSize}px`,
      minHeight: `${tileSize}px`,
      border: selected ? '2px solid #c9aa71' : '1px solid rgba(201, 170, 113, 0.24)',
      borderRadius: '4px',
      background: selected ? 'rgba(201, 170, 113, 0.18)' : 'rgba(255,255,255,0.05)',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      padding: '3px',
      boxSizing: 'border-box',
    });

    const imageStyle: React.CSSProperties = {
      width: '48px',
      height: '48px',
      objectFit: 'contain',
      imageRendering: 'pixelated',
    };

    const emptyStyle: React.CSSProperties = {
      color: '#777d82',
      fontFamily: 'Verdana',
      fontSize: '11px',
      textAlign: 'center',
      padding: '12px 0',
    };

    return (
      <MobilePanelScreen
        panelType={'tile_resources'}
        title={'Resources'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={<MobileSummaryCard title="Discovered Resources" subtitle={`${resources.length} found`} />}
          right={
            <MobileCard compact={landscape}>
              {resources.length == 0 && <div style={emptyStyle}>No resources discovered</div>}
              {resources.length > 0 &&
                <div style={gridStyle}>
                  {resources.map((resource, index) => {
                    const selected = this.state.selectedIndex == index;
                    const handleClick = () => {
                      const eventData = {
                        name: resource.name,
                        image: resource.image,
                        yieldLabel: resource.yield_label,
                        quantityLabel: resource.quantity_label,
                        properties: resource.properties,
                        index,
                      };
                      Global.gameEmitter.emit(GameEvent.RESOURCE_CLICK, eventData);
                      this.handleResourceClick(eventData);
                    };

                    return (
                      <button key={index} type="button" style={buttonStyle(selected)} onClick={handleClick} title={resource.name}>
                        <img src={'/static/art/items/' + resourceImageForName(resource.image || resource.name) + '.png'} style={imageStyle} />
                      </button>
                    );
                  })}
                </div>}
            </MobileCard>
          } />
      </MobilePanelScreen>
    );
  }
}
