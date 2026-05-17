import * as React from "react";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { MOBILE_PANEL_Z_BASE } from "./mobileLayers";

interface MobilePanelScreenProps {
  panelType: string,
  title?: string,
  hideExitButton?: boolean,
  zIndexBonus?: number,
  children: React.ReactNode,
  footer?: React.ReactNode,
  contentStyle?: React.CSSProperties,
}

export default class MobilePanelScreen extends React.Component<MobilePanelScreenProps, any> {
  private id: string;

  constructor(props) {
    super(props);
    this.id = `${props.panelType}:${Math.random().toString(36).slice(2)}`;
    this.state = { z: Global.zIndexManager.register(this.id) };
    this.handleClose = this.handleClose.bind(this);
    this.handleActivate = this.handleActivate.bind(this);
  }

  componentWillUnmount() {
    Global.zIndexManager.unregister(this.id);
  }

  handleActivate() {
    const z = Global.zIndexManager.bringToFront(this.id);
    if (z !== this.state.z) this.setState({ z });
  }

  handleClose() {
    Global.gameEmitter.emit(GameEvent.EXIT_HALFPANEL_CLICK, { panelType: this.props.panelType });
  }

  render() {
    const rootStyle: React.CSSProperties = {
      position: 'fixed',
      top: 0,
      right: 0,
      bottom: 0,
      left: 0,
      zIndex: MOBILE_PANEL_Z_BASE + this.state.z + (this.props.zIndexBonus || 0),
      background: 'rgba(8, 10, 12, 0.96)',
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      pointerEvents: 'auto',
      boxSizing: 'border-box',
      display: 'flex',
      flexDirection: 'column',
      padding: 'calc(12px + env(safe-area-inset-top, 0px)) calc(10px + env(safe-area-inset-right, 0px)) calc(12px + env(safe-area-inset-bottom, 0px)) calc(10px + env(safe-area-inset-left, 0px))',
    };

    const headerStyle: React.CSSProperties = {
      flex: '0 0 auto',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'space-between',
      gap: '10px',
      minHeight: '44px',
      borderBottom: '1px solid rgba(201, 170, 113, 0.35)',
      paddingBottom: '8px',
      boxSizing: 'border-box',
    };

    const titleStyle: React.CSSProperties = {
      color: '#c9aa71',
      fontFamily: 'Cinzel, Verdana, serif',
      fontSize: '16px',
      fontWeight: 'bold',
      letterSpacing: 0,
      margin: 0,
      overflow: 'hidden',
      textOverflow: 'ellipsis',
      whiteSpace: 'nowrap',
    };

    const closeStyle: React.CSSProperties = {
      flex: '0 0 auto',
      minHeight: '36px',
      minWidth: '64px',
      border: '1px solid rgba(201, 170, 113, 0.55)',
      borderRadius: '4px',
      background: '#25282b',
      color: '#f2e7cf',
      fontFamily: 'Verdana',
      fontSize: '12px',
    };

    const contentStyle: React.CSSProperties = {
      flex: '1 1 auto',
      minHeight: 0,
      overflowY: 'auto',
      WebkitOverflowScrolling: 'touch',
      padding: '12px 0',
      boxSizing: 'border-box',
      ...this.props.contentStyle,
    };

    const footerStyle: React.CSSProperties = {
      flex: '0 0 auto',
      borderTop: '1px solid rgba(201, 170, 113, 0.35)',
      paddingTop: '8px',
    };

    const title = this.props.title || this.props.panelType;

    return (
      <div style={rootStyle} onMouseDown={this.handleActivate}>
        <div style={headerStyle}>
          <h2 style={titleStyle}>{title}</h2>
          {!this.props.hideExitButton &&
            <button type="button" style={closeStyle} onClick={this.handleClose}>Close</button>}
        </div>
        <div style={contentStyle}>
          {this.props.children}
        </div>
        {this.props.footer &&
          <div style={footerStyle}>{this.props.footer}</div>}
      </div>
    );
  }
}
