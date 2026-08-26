
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
      width: 'min(333px, calc(100vw - 20px - env(safe-area-inset-left, 0px) - env(safe-area-inset-right, 0px)))',
      height: '119px',
      transform: `translate(-50%, calc(-50% + ${marginTop + 59}px))`,
      position: 'fixed',
      zIndex: MOBILE_DIALOG_Z
    } as React.CSSProperties

    const loadingPanelStyle = {
      position: 'absolute',
      inset: 0,
      width: '100%',
      height: '100%',
      objectFit: 'fill',
    } as React.CSSProperties

    const spanNameStyle = {
      top: '22px',
      left: '14px',
      right: '14px',
      position: 'absolute',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '14px',
      lineHeight: 1.25,
      overflowWrap: 'anywhere',
    } as React.CSSProperties

    return (
      <div style={loadingStyle}>
        <img src={errorpanel} style={loadingPanelStyle}/>
        <span style={spanNameStyle}>{this.props.errmsg}</span>
      </div>
    );
  }
}
