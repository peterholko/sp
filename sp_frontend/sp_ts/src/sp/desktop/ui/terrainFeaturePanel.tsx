import * as React from "react";
import HalfPanel from "./halfPanel";

interface TerrainFeaturePanelProps {
  tileData,
}

export default class TerrainFeaturePanel extends React.Component<TerrainFeaturePanelProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };

  }


  render() {
    const tableStyle = {
      transform: 'translate(100px, -70px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const featureNameStyle = {
      transform: 'translate(-323px, 30px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    const imageStyle = {
      transform: 'translate(-260px, 75px)',
      position: 'fixed'
    } as React.CSSProperties

    const tableStyle2 = {
      //transform: 'translate(-5px, -5px)',
      //position: 'fixed',
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


    return (
      <HalfPanel left={false}
        panelType={'terrain_features'}
        hideExitButton={false}>
        
        <span style={featureNameStyle}>{this.props.tileData.terrain_features[0].name}</span>
        
        <table style={tableStyle}>
          <tbody>
            <tr>
                <td>{this.props.tileData.terrain_features[0].bonus}</td>
            </tr>
          </tbody>
        </table>

        <img src={'/static/art/features/' + this.props.tileData.terrain_features[0].image + '.png'} style={imageStyle} />

      </HalfPanel>
    );
  }
}