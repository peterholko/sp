import * as React from "react";
import HalfPanel from "./halfPanel";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import ResourceItem from "./resourceItem";

interface TileResourceDetailPanelProps {
  tileData,
}

export default class TileResourceDetailPanel extends React.Component<TileResourceDetailPanelProps, any> {
  constructor(props) {
    super(props);

    if (this.props.tileData.resources.length > 0) {
      this.state = {
        resource: this.props.tileData.resources[0],
        index: 0
      };
    } else {
      this.state = {
        resource: null,
        index: 0
      }
    }

    this.handleLeftClick = this.handleLeftClick.bind(this);
    this.handleRightClick = this.handleRightClick.bind(this);
  }

  handleLeftClick(event) {
    if (this.state.index != 0) {
      const newIndex = this.state.index - 1;
      this.setState({
        resource: this.props.tileData.resources[newIndex],
        index: newIndex
      })
    }
  }

  handleRightClick(event) {
    if (this.state.index != (this.props.tileData.resources.length - 1)) {
      const newIndex = this.state.index + 1;
      this.setState({
        resource: this.props.tileData.resources[newIndex],
        index: newIndex
      })
    }
  }

  render() {
    const resources = []
    const zeroResources = this.props.tileData.resources.length == 0;

    var imageName;
    var resourceTitle;

    for (var i = 0; i < this.props.tileData.resources.length; i++) {
      var resource = this.props.tileData.resources[i];
      var resourceImage = resource.image.toLowerCase().replace(/\s/g, '');

      resources.push(
        <ResourceItem key={i}
          resourceName={resource.name}
          resourceImage={resourceImage}
          yieldLabel={resource.yield_label}
          quantityLabel={resource.quantity_label}
          quantity={0}
          properties={resource.properties}
          index={i}
          showQuantity={false}
          spaceDistance={35} />
        //xPos={10 + (i * 25)}
        //yPos={150}/>
      )
    }

    if (!zeroResources) {
      imageName = this.state.resource.name.replace(/\s/g, '').toLowerCase();
      resourceTitle = this.state.resource.name;
    } else {
      resourceTitle = 'No resources found.'
    }

    var properties = [];

    if (!zeroResources) {
      if (this.state.resource.properties) {
        for (var i = 0; i < this.state.resource.properties.length; i++) {
          properties.push(<tr key={i}>
            <td>+{this.state.resource.properties[i].value} {this.state.resource.properties[i].name}</td>
          </tr>);
        }
      }
    }

    const tableStyle = {
      transform: 'translate(20px, -250px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const resDivStyle = {
      transform: 'translate(40px, -70px)',
      position: 'fixed',
    } as React.CSSProperties

    const titleStyle = {
      transform: 'translate(-323px, 30px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    const resourceNameStyle = {
      transform: 'translate(-323px, 90px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    const imageStyle = {
      transform: 'translate(-195px, 25px)',
      position: 'fixed'
    } as React.CSSProperties

    const tableStyle2 = {
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const leftStyle = {
      transform: 'translate(-305px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    const rightStyle = {
      transform: 'translate(-65px, 295px)',
      position: 'fixed'
    } as React.CSSProperties

    //<span style={titleStyle}>Resources ({x}, {y})</span>



    return (
      <HalfPanel left={true}
        panelType={'tile_resource_detail'}
        hideExitButton={false}>

        {!zeroResources &&
          <img src={'/static/art/items/' + imageName + '.png'} style={imageStyle} />}

        <span style={resourceNameStyle}>{resourceTitle}</span>

        {!zeroResources &&
          <table style={tableStyle}>
            <tbody>
              <tr>
                <td>Quantity: </td>
                <td>{this.state.resource.quantity_label}</td>
              </tr>

              <tr>
                <td>Yield: </td>
                <td>{this.state.resource.yield_label}</td>
              </tr>
              <tr><td></td></tr>
              <tr>
                <td colSpan={2}>
                  <table style={tableStyle2}>
                    <tbody>
                      {properties}
                    </tbody>
                  </table>
                </td>
              </tr>
            </tbody>
          </table>}

        {!zeroResources && <img src={leftbutton} style={leftStyle} onClick={this.handleLeftClick} />}
        {!zeroResources && <img src={rightbutton} style={rightStyle} onClick={this.handleRightClick} />}

      </HalfPanel>
    );
  }
}