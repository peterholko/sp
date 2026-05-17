import * as React from "react";
import halfpanel from "ui_comp/halfpanel.png";
import MobilePanelScreen from "./mobilePanelScreen";

interface HalfPanelProps {
  left : boolean,
  panelType : string,
  hideExitButton : boolean,
  children: React.ReactNode,
  middle?: boolean,
  zIndexBonus?: number
}

export default class HalfPanel extends React.Component<HalfPanelProps, any> {
  constructor(props) {
    super(props);
  }

  render() {
    const stageStyle: React.CSSProperties = {
      position: 'relative',
      width: '323px',
      height: '360px',
      margin: '0 auto',
      transform: 'translateZ(0)',
      flex: '0 0 auto',
    };

    const imageStyle: React.CSSProperties = {
      position: 'absolute',
      top: 0,
      left: 0,
      width: '323px',
      height: '360px',
      pointerEvents: 'none',
    };

    return (
      <MobilePanelScreen
        panelType={this.props.panelType}
        title={this.props.panelType}
        hideExitButton={this.props.hideExitButton}
        zIndexBonus={this.props.zIndexBonus}
        contentStyle={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'center' }}
      >
        <div style={stageStyle}>
          <img src={halfpanel} style={imageStyle} />
          {this.props.children}
        </div>
      </MobilePanelScreen>
    );
  }
}
