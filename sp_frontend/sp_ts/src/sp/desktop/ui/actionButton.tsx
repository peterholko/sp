
import * as React from "react";
import quickattackbutton from "ui_comp/quickattackbutton.png";
import preciseattackbutton from "ui_comp/preciseattackbutton.png";
import fierceattackbutton from "ui_comp/fierceattackbutton.png";
import cooldownbg from "ui_comp/cooldownbg.png";
import styles from "./../ui.module.css";
import { Global } from "../../core/global";
import { NetworkEvent } from "../../core/networkEvent";
import { QUICK, PRECISE, FIERCE } from "../../core/config";

interface ActionButtonProps {
  type,
  handler,
  title?: string,
}

export default class ActionButton extends React.Component<ActionButtonProps, any> {

  constructor(props) {
    super(props);

    this.state = {
      timerId: -1,
      cooldown: -1,
      cooldownEndsAt: 0,
    };

    this.startTimer = this.startTimer.bind(this);
    this.stopTimer = this.stopTimer.bind(this);
    this.handleClick = this.handleClick.bind(this);
    this.timer = this.timer.bind(this);
    
  }

  componentDidMount() {
    Global.gameEmitter.on(NetworkEvent.ATTACK, this.handleAttack, this);
  }

  componentWillUnmount() {
    Global.gameEmitter.off(NetworkEvent.ATTACK, this.handleAttack, this);
    this.stopTimer();
  }

  handleAttack(message) {
    this.stopTimer();
    const duration = Math.max(0, Number(message.cooldown) || 0);
    if (duration <= 0) {
      this.setState({ cooldown: -1, cooldownEndsAt: 0 });
      return;
    }
    this.setState({
      cooldown: Math.ceil(duration),
      cooldownEndsAt: Date.now() + duration * 1000,
    }, this.startTimer);
  }

   startTimer() {
    var timerId = setInterval(this.timer, 100);
    this.setState({timerId: timerId});
  }

  stopTimer() {
    clearInterval(this.state.timerId);
  }

  timer() {
    const remainingMs = this.state.cooldownEndsAt - Date.now();
    if (remainingMs <= 0) {
      this.setState({cooldown: -1, cooldownEndsAt: 0});
      this.stopTimer();
    } else {
      this.setState({cooldown: Math.ceil(remainingMs / 1000)});
    }
  }

  handleClick = () => {
    this.props.handler()
  }

  render() {
    var buttonType;
    var cssStyle;
    var defaultTitle;

    if(this.props.type == QUICK) {
      buttonType = quickattackbutton;
      cssStyle = styles.quickattackbutton;
      defaultTitle = "Quick Attack — fast, light hit (dodged by Dodge)";
    } else if(this.props.type == PRECISE) {
      buttonType = preciseattackbutton;
      cssStyle = styles.preciseattackbutton;
      defaultTitle = "Precise Attack — balanced hit (countered by Parry)";
    } else if(this.props.type == FIERCE) {
      buttonType = fierceattackbutton;
      cssStyle = styles.fierceattackbutton;
      defaultTitle = "Fierce Attack — slow, heavy hit (blocked by Brace)";
    }

    const title = this.props.title || defaultTitle;

    const spanStyle = {
      transform: 'translate(12px, 13px)',
      position: 'fixed',
      fontFamily: 'Verdana',
      fontSize: '36px',
      width: '50px',
      height: '50px',
      color: 'white',
      WebkitTextStroke: '1px black',
      userSelect: 'none',
      pointerEvents: 'none',
    } as React.CSSProperties

    const cooldownBgStyle = {
      position: 'fixed',
      opacity: 0.5,
    } as React.CSSProperties

    return (
      <div id={this.props.type + 'attackbutton'} className={cssStyle}>
        {this.state.cooldown != -1 && 
          <img src={cooldownbg} style={cooldownBgStyle}/> }

        {this.state.cooldown != -1 && 
          <span style={spanStyle}>{this.state.cooldown}</span> }

        <img src={buttonType}
             title={title}
             alt={title}
             aria-label={title}
             onClick={this.handleClick}/>
      </div>
    );
  }
}
