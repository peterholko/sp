import * as React from "react";
import okbutton from "ui_comp/okbutton.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";

interface IntroProps {
}

interface IntroState {
  currentPanel: number;
}

export default class IntroPanel extends React.Component<IntroProps, IntroState> {
  constructor(props) {
    super(props);

    this.state = {
      currentPanel: 0,
    };

    this.handleOkClick = this.handleOkClick.bind(this);
  }

  handleOkClick() {
    if (this.state.currentPanel < 1) {
      this.setState({ currentPanel: this.state.currentPanel + 1 });
    } else {
      Global.gameEmitter.emit(GameEvent.INTRO_OK_CLICK, {});
    }
  }

  render() {
    const panels = [
      `Your ship broke apart on the rocks. You crawled ashore with what you could carry. Two of your crew were not so fortunate.

In this untamed land of peril and opportunity, survival is your first challenge.

Welcome to Perilous.`,
      `Search the shipwreck for salvaged supplies. Build a campfire before nightfall.

The wilderness here is unforgiving.`,
    ];

    const introText = panels[this.state.currentPanel];

    const overlayStyle: React.CSSProperties = {
      position: 'fixed',
      top: 0, left: 0, right: 0, bottom: 0,
      background: 'rgba(0, 0, 0, 0.85)',
      zIndex: 20,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      padding: 'calc(16px + env(safe-area-inset-top, 0px)) 16px calc(16px + env(safe-area-inset-bottom, 0px))',
      boxSizing: 'border-box',
      overflowY: 'auto',
    };

    const cardStyle: React.CSSProperties = {
      width: '100%',
      maxWidth: '400px',
      background: '#1c1814',
      border: '1px solid #5a4a38',
      borderRadius: '8px',
      padding: '24px 20px',
      boxSizing: 'border-box',
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      gap: '20px',
    };

    const shipwreckStyle: React.CSSProperties = {
      width: '100%',
      maxWidth: '275px',
      height: 'auto',
    };

    const textStyle: React.CSSProperties = {
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Cinzel',
      fontSize: '14px',
      lineHeight: 1.5,
      whiteSpace: 'pre-wrap',
      margin: 0,
    };

    const submitStyle: React.CSSProperties = {
      cursor: 'pointer',
    };

    return (
      <div style={overlayStyle}>
        <div style={cardStyle}>
          <img src={"/static/art/ui/intro_shipwreck.png"} style={shipwreckStyle} alt="Shipwreck" />
          <p style={textStyle}>{introText}</p>
          <img src={okbutton} style={submitStyle} onClick={this.handleOkClick} alt="Continue" />
        </div>
      </div>
    );
  }
}
