import * as React from "react";
import HalfPanel from "./halfPanel";
import { Global } from "../../core/global";
import { isSafeLogoutProtectedObject } from "../../core/protectedSettlements";
import DroppedBagExpiry from "../../core/droppedBagExpiry";

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

    const safeLogoutProtected = isSafeLogoutProtectedObject(
      Global.objectStates[this.props.objData.id],
      Global.protectedSettlements,
    );

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

    const protectedStyle = {
      color: '#e5fbff',
      background: 'rgba(16, 40, 58, 0.94)',
      border: '1px solid #e4c66f',
      borderRadius: '3px',
      padding: '5px 7px',
      display: 'inline-block',
      marginBottom: '7px',
      boxShadow: '0 0 10px rgba(114, 214, 232, 0.28)'
    } as React.CSSProperties

    return (
      <HalfPanel left={true}
        panelType={'obj'}
        hideExitButton={false}>

        <img src={imagePath} style={imageStyle} />
        <table style={tableStyle}>
          <tbody>
            {safeLogoutProtected &&
              <tr>
                <td colSpan={2}>
                  <span
                    style={protectedStyle}
                    title="Frozen and protected until this settlement's owner returns."
                  >
                    ◇ Safe Logout protected
                  </span>
                </td>
              </tr>
            }
            <tr>
              <td>Name: </td>
              <td>{this.props.objData.name}</td>
            </tr>
            <tr>
              <td>State: </td>
              <td>{this.props.objData.state}</td>
            </tr>
            {this.props.objData.expires_in != null &&
              <tr>
                <td colSpan={2}>
                  <DroppedBagExpiry
                    key={this.props.objData.id}
                    expiresIn={this.props.objData.expires_in}
                  />
                </td>
              </tr>
            }
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
