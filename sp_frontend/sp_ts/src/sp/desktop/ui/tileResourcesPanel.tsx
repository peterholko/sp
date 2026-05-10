
import * as React from "react";
import { Global } from "../../core/global";
import HalfPanel from "./halfPanel";
import itemframe from "ui_comp/itemframe.png";
import selectitemborder from "ui_comp/selectitemborder.png";
import ResourceItem from "./resourceItem";
import { GameEvent } from "../../core/gameEvent";

interface TRPProps {
  tileData
}

export default class TileResourcesPanel extends React.Component<TRPProps, any> {
  constructor(props) {
    super(props);

    Global.selectedItemId = -1;
    Global.selectedItemOwnerId = -1;  

    this.state = {
      selectItemStyle: false
    };
    
    Global.gameEmitter.on(GameEvent.RESOURCE_CLICK, this.handleResourceClick, this);
  }

  handleResourceClick(eventData) {
    console.log('handleSelect ' + eventData);
    var xPos = -293 + ((eventData.index % 5) * 53);
    var yPos = 83 + (Math.floor(eventData.index / 5) * 53);

    const selectItemStyle = {
      transform: 'translate(' + xPos + 'px, ' + yPos + 'px)',
      position: 'fixed'
    }

    this.setState({
      selectItemStyle: selectItemStyle
    });
  }

  render() {
    var resourceList = [];
    var itemFrameResources = [];

    for (var i = 0; i < 20; i++) {
      var xPos = -293 + ((i % 5) * 53);
      var yPos = 83 + (Math.floor(i / 5) * 53);

      var frameResource = {
        transform: 'translate(' + xPos + 'px, ' + yPos + 'px',
        position: 'fixed'
      } as React.CSSProperties

      itemFrameResources.push(
        <img src={itemframe} key={i} style={frameResource} />
      )
    }

    if (this.props.tileData.resources.length > 0) {
      for (var i = 0; i < this.props.tileData.resources.length; i++) {
        var resource = this.props.tileData.resources[i];

        var xPos = 31 + ((i % 5) * 53);
        var yPos = -275 + (Math.floor(i / 5) * 53);

        resourceList.push(
          <ResourceItem key={i}
            resourceName={resource.name}
            resourceImage={resource.image}
            yieldLabel={resource.yield_label}
            quantityLabel={resource.quantity_label}
            quantity={0}
            properties={resource.properties}
            index={i}
            showQuantity={false}
            xPos={xPos}
            yPos={yPos}
          />
        )
      }

    }

    const featureNameStyle = {
      transform: 'translate(-323px, 30px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    return (
      <div>
        <HalfPanel left={true}
          panelType={'tile_resources'}
          hideExitButton={false}>

          <span style={featureNameStyle}>Discovered Resources</span>

          {itemFrameResources}
          {resourceList}

          {this.state.selectItemStyle &&
            <img src={selectitemborder} style={this.state.selectItemStyle} />
          }

        </HalfPanel>
      </div>
    );
  }
}
