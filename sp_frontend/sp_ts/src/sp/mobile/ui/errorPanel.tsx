
import * as React from "react";
import errorpanel from "ui_comp/errorframe.png";
import okbutton from "ui_comp/okbutton.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { MOBILE_DIALOG_Z } from "./mobileLayers";

interface ErrorProps {
  errmsg: string,
  yOffset?: number
}

export default class ErrorPanel extends React.Component<ErrorProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };
   
    this.handleOkClick = this.handleOkClick.bind(this);
  }

  handleOkClick() {
    Global.gameEmitter.emit(GameEvent.ERROR_OK_CLICK, {});
  }

  render() {
    //const marginTop = this.props.yOffset ? -59 + this.props.yOffset : -59;
    const marginTop = this.props.yOffset ? -9 + this.props.yOffset : 50;

    const errorStyle = {
      top: '50%',
      left: '50%',
      width: 'min(333px, calc(100vw - 20px - env(safe-area-inset-left, 0px) - env(safe-area-inset-right, 0px)))',
      minHeight: '132px',
      transform: `translate(-50%, calc(-50% + ${marginTop}px))`,
      position: 'fixed',
      zIndex: MOBILE_DIALOG_Z
    } as React.CSSProperties

    const errorPanelStyle = {
      position: 'absolute',
      inset: 0,
      width: '100%',
      height: '100%',
      objectFit: 'fill',
    } as React.CSSProperties

    const spanNameStyle = {
      top: '25px',
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

    const okButtonStyle = {
      left: '50%',
      bottom: '3px',
      transform: 'translateX(-50%)',
      position: 'absolute',
    } as React.CSSProperties

    return (
      <div style={errorStyle}>
        <img src={errorpanel} style={errorPanelStyle}/>
        <span style={spanNameStyle}>{this.props.errmsg}</span>
        <img src={okbutton} style={okButtonStyle} onClick={this.handleOkClick}/>
      </div>
    );
  }
}
