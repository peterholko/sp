import * as React from "react";
import HalfPanel from "./halfPanel";
import { Global } from "../../core/global";

interface ObjPanelProps {
  objData,
}

export default class ObjPanel extends React.Component<ObjPanelProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };

  }

  render() {
    let imagePath = '/static/art/' + this.props.objData.image + '.png';

    let hideSoulshards = true;

    if (this.props.objData.subclass == 'monolith') {
      hideSoulshards = false;
    }

    const imageStyle = {
      transform: 'translate(-197px, 25px)',
      position: 'fixed'
    } as React.CSSProperties

    const tableStyle = {
      transform: 'translate(20px, -250px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    return (
      <HalfPanel left={true}
        panelType={'obj'}
        hideExitButton={false}>

        <img src={imagePath} style={imageStyle} />
        <table style={tableStyle}>
          <tbody>
            <tr>
              <td>Name: </td>
              <td>{this.props.objData.name}</td>
            </tr>
            <tr>
              <td>State: </td>
              <td>{this.props.objData.state}</td>
            </tr>
            {!hideSoulshards &&
              <tr>
                <td>Soulshards: </td>
                <td>{this.props.objData.soulshards}</td>
              </tr>
            }
          </tbody>
        </table>
      </HalfPanel>
    );
  }
}

