
import * as React from "react";
import errorpanel from "ui_comp/errorframe.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { MOBILE_DIALOG_Z } from "./mobileLayers";

interface LoadingProps {
  errmsg: string,
  yOffset?: number
}

export default class LoadingPanel extends React.Component<LoadingProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };
   
  }

  render() {
    const marginTop = this.props.yOffset ? -59 + this.props.yOffset : -59;

    const loadingStyle = {
      top: '50%',
      left: '50%',
      width: '333px',
      height: '119px',
      marginTop: `${marginTop}px`,
      marginLeft: '-166px',
      position: 'fixed',
      zIndex: MOBILE_DIALOG_Z
    } as React.CSSProperties

    const loadingPanelStyle = {
      position: 'fixed'
    } as React.CSSProperties

    const spanNameStyle = {
      transform: 'translate(15px, 20px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '14px',
      width: '300px'
    } as React.CSSProperties

    return (
      <div style={loadingStyle}>
        <img src={errorpanel} style={loadingPanelStyle}/>
        <span style={spanNameStyle}>{this.props.errmsg}</span>
      </div>
    );
  }
}
