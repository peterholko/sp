import * as React from "react";
import exitbutton from "ui_comp/exitbutton.png";
import halfpanel from "ui_comp/halfpanel.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { getHalfPanelMarginTop } from "../../core/uiLayout";

interface HalfPanelProps {
  left : boolean,
  panelType : string,
  hideExitButton : boolean,
  children: React.ReactNode,
  middle?: boolean,
  zIndexBonus?: number
}

export default class HalfPanel extends React.Component<HalfPanelProps, any> {
  private id: string;
  constructor(props) {
    super(props);

    this.id = `${props.panelType}:${props.instanceId ?? Math.random().toString(36).slice(2)}`;
    this.state = { z: Global.zIndexManager.register(this.id) };

    this.handleExitClick = this.handleExitClick.bind(this);
    this.handleActivate = this.handleActivate.bind(this);
  }

  handleExitClick(event : React.MouseEvent) {
    console.log('Exit click')
    const eventData = {panelType: this.props.panelType};

    Global.gameEmitter.emit(GameEvent.EXIT_HALFPANEL_CLICK, eventData);
  }

  handleActivate() {
    const z = Global.zIndexManager.bringToFront(this.id);
    if (z !== this.state.z) this.setState({ z });
  }

  componentWillUnmount() {
    Global.zIndexManager.unregister(this.id);
  }

  render() {
    const marginTop = `${getHalfPanelMarginTop()}px`;

    const baseStyle = {
      top: '50%',
      left: '50%',
      width: '323px',
      height: '360px',
      marginTop: marginTop,
      position: 'fixed',
      zIndex: this.state.z + (this.props.zIndexBonus || 0),
    } as React.CSSProperties;

    let halfPanelStyle: React.CSSProperties;
    let exitStyle: React.CSSProperties;

    if (this.props.middle) {
      halfPanelStyle = { ...baseStyle, marginLeft: '-161px' };
      exitStyle = {
        top: '50%',
        left: '50%',
        marginTop: baseStyle.marginTop,
        marginLeft: '111px',
        position: 'fixed',
        zIndex: (baseStyle.zIndex as number) + 1,
      };
    } else if (this.props.left) {
      halfPanelStyle = { ...baseStyle, marginLeft: '-323px' };
      exitStyle = {
        top: '50%',
        left: '50%',
        marginTop: baseStyle.marginTop,
        marginLeft: '-50px',
        position: 'fixed',
        zIndex: (baseStyle.zIndex as number) + 1,
      };
    } else {
      halfPanelStyle = { ...baseStyle, marginLeft: '0px' };
      exitStyle = {
        top: '50%',
        left: '50%',
        marginTop: baseStyle.marginTop,
        marginLeft: '273px',
        position: 'fixed',
        zIndex: (baseStyle.zIndex as number) + 1,
      };
    }

    return (
      <div style={halfPanelStyle} onMouseDown={this.handleActivate}>
        <img src={halfpanel} />
        {!this.props.hideExitButton && (
          <img src={exitbutton} onClick={this.handleExitClick} style={exitStyle} />
        )}
        {this.props.children}
      </div>
    );
  }
}
