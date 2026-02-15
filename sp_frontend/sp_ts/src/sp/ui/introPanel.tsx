import * as React from "react";
import widepanel from "ui_comp/widepanel.png";
import okbutton from "ui_comp/okbutton.png";
import { Global } from "../global";
import { GameEvent } from "../gameEvent";

interface IntroProps {
}

interface IntroState {
  windowHeight: number;
}

export default class IntroPanel extends React.Component<IntroProps, IntroState> {
  constructor(props) {
    super(props);

    this.state = {
      windowHeight: window.innerHeight,
    };

    this.handleOkClick = this.handleOkClick.bind(this);
    this.updateWindowHeight = this.updateWindowHeight.bind(this);
  }

  componentDidMount() {
    window.addEventListener("resize", this.updateWindowHeight);
  }

  componentWillUnmount() {
    window.removeEventListener("resize", this.updateWindowHeight);
  }

  updateWindowHeight() {
    this.setState({ windowHeight: window.innerHeight });
  }

  handleOkClick() {
    Global.gameEmitter.emit(GameEvent.INTRO_OK_CLICK, {});
  }

  render() {
    const introText = `Tasked by the Empire to explore the mystical lands of the East, disaster struck as your ship neared the shores. Violent winds and rising seas wrecked your vessel, leaving you stranded with only salvaged supplies and one villager.

In this untamed land of peril and opportunity, survival is your first challenge. Rebuild, endure, and carve your place in history.

Welcome to Perilous.`;

    const introStyle = {
      top: "50%",
      left: "50%",
      width: "667px",
      height: "375px",
      marginTop: "-193px",
      marginLeft: "-333px",
      position: "fixed",
      zIndex: 20,
    } as React.CSSProperties;

    const introPanelStyle = {
      position: "fixed",
    } as React.CSSProperties;

    const spanNameStyle = {
      transform: "translate(20px, 180px)",
      position: "fixed",
      textAlign: "center",
      color: "white",
      fontFamily: "Cinzel",
      fontSize: "14px",
      width: "620px",
      whiteSpace: "pre-wrap",
    } as React.CSSProperties;

    const introShipwreckStyle = {
      transform: "translate(200px, 20px)",
      position: "fixed",
      width: "275px",
    } as React.CSSProperties;


    let okButtonStyle; 

    if (this.state.windowHeight < 400) {
      okButtonStyle = {
        transform: `translate(425px, 310px)`, 
        position: "fixed",
      } as React.CSSProperties;
    } else {
      okButtonStyle = {
      transform: `translate(307px, 350px)`, 
      position: "fixed",
    } as React.CSSProperties;
  }

    return (
      <div style={introStyle}>
        <img src={widepanel} style={introPanelStyle} />
        <img src={"/static/art/ui/intro_shipwreck.png"} style={introShipwreckStyle} />
        <span style={spanNameStyle}>{introText}</span>
        <img src={okbutton} style={okButtonStyle} onClick={this.handleOkClick} />
      </div>
    );
  }
}
