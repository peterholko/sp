
import * as React from "react";
import confirmpanel from "ui_comp/errorframe.png";
import exitbutton from "ui_comp/exitbutton.png";
import okbutton from "ui_comp/okbutton.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { MOBILE_DIALOG_Z } from "./mobileLayers";

interface ConfirmProps {
  msg,
  onConfirm?: () => void,
  onCancel?: () => void,
}

export default class ConfirmPanel extends React.Component<ConfirmProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };
   
    this.handleOkClick = this.handleOkClick.bind(this);
    this.handleExitClick = this.handleExitClick.bind(this)
  }

  handleOkClick() {
    console.log('Handle Ok Click');
    if (this.props.onConfirm) {
      this.props.onConfirm();
      return;
    }
    Global.gameEmitter.emit(GameEvent.CONFIRM_OK_CLICK, {});
  }

  handleExitClick(event : React.MouseEvent) {
    if (this.props.onCancel) {
      this.props.onCancel();
      return;
    }
    const eventData = {panelType: "confirm"};
    Global.gameEmitter.emit(GameEvent.EXIT_HALFPANEL_CLICK, eventData);
  }

  render() {
    const confirmStyle = {
      top: '50%',
      left: '50%',
      width: 'min(333px, calc(100vw - 20px - env(safe-area-inset-left, 0px) - env(safe-area-inset-right, 0px)))',
      minHeight: '132px',
      transform: 'translate(-50%, -50%)',
      position: 'fixed',
      zIndex: MOBILE_DIALOG_Z
    } as React.CSSProperties

    const confirmPanelStyle = {
      position: 'absolute',
      inset: 0,
      width: '100%',
      height: '100%',
      objectFit: 'fill',
    } as React.CSSProperties

    const spanNameStyle = {
      top: '24px',
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

    const exitButtonStyle = {
      top: 0,
      right: 0,
      position: 'absolute',
    } as React.CSSProperties

    return (
      <div style={confirmStyle}>
        <img src={confirmpanel} style={confirmPanelStyle}/>
        <span style={spanNameStyle}>{this.props.msg}</span>
        <img src={okbutton} style={okButtonStyle} onClick={this.handleOkClick}/>
        <img src={exitbutton} 
                   onClick={this.handleExitClick} 
                   style={exitButtonStyle}/>        
      </div>
    );
  }
}
