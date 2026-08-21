
import * as React from "react";
import cooldownbg from "ui_comp/cooldownbg.png";
import { Global } from "../../core/global";
import { GATHERING } from "../../core/config";
import { GameEvent } from "../../core/gameEvent";
import { NetworkEvent } from "../../core/networkEvent";
import gatherbuttonimage from "ui_comp/gatherbutton.png";
import smalliconborder from "ui_comp/selectbordersmall.png";

interface GatherButtonProps {
  className,
  activeClassName?: string,
  handler,
  title?: string,
}

export default class GatherButton extends React.Component<GatherButtonProps, any> {

  constructor(props) {
    super(props);

    this.state = {
      timerId: -1,
      cooldown: -1,
      showClicked: false,
      active: false,
    };

    this.startTimer = this.startTimer.bind(this);
    this.stopTimer = this.stopTimer.bind(this);
    this.handleClick = this.handleClick.bind(this);
    this.timer = this.timer.bind(this);
    this.hideImage = this.hideImage.bind(this);
    this.handleObjUpdate = this.handleObjUpdate.bind(this);
    this.syncActiveState = this.syncActiveState.bind(this);
    this.handleDisconnected = this.handleDisconnected.bind(this);
  }

  componentDidMount() {
    Global.gameEmitter.on(NetworkEvent.GATHER, this.handleGather, this);
    Global.gameEmitter.on(GameEvent.OBJ_UPDATE, this.handleObjUpdate, this);
    Global.gameEmitter.on(GameEvent.OBJ_MOVED, this.handleObjUpdate, this);
    Global.gameEmitter.on(NetworkEvent.HERO_INIT, this.syncActiveState, this);
    Global.gameEmitter.on(NetworkEvent.SERVER_OFFLINE, this.handleDisconnected, this);
    Global.gameEmitter.on(NetworkEvent.SAFE_LOGOUT_COMPLETE, this.handleDisconnected, this);
    this.syncActiveState();
  }

  componentWillUnmount() {
    Global.gameEmitter.off(NetworkEvent.GATHER, this.handleGather, this);
    Global.gameEmitter.off(GameEvent.OBJ_UPDATE, this.handleObjUpdate, this);
    Global.gameEmitter.off(GameEvent.OBJ_MOVED, this.handleObjUpdate, this);
    Global.gameEmitter.off(NetworkEvent.HERO_INIT, this.syncActiveState, this);
    Global.gameEmitter.off(NetworkEvent.SERVER_OFFLINE, this.handleDisconnected, this);
    Global.gameEmitter.off(NetworkEvent.SAFE_LOGOUT_COMPLETE, this.handleDisconnected, this);
    this.stopTimer();
  }

  handleObjUpdate(objId) {
    if (Number(objId) === Number(Global.heroId)) {
      this.syncActiveState();
    }
  }

  syncActiveState() {
    const hero = Global.objectStates[Global.heroId];
    const active = Boolean(hero && hero.state === GATHERING);
    if (active !== this.state.active) {
      this.setState({ active });
    }
  }

  handleDisconnected() {
    if (this.state.active) {
      this.setState({ active: false });
    }
  }

  handleGather(message) {
    this.setState({
      cooldown: message.gather_time,
      active: true,
    });
    this.startTimer();
  }

  startTimer() {
    this.stopTimer();
    var timerId = setInterval(this.timer, 1000);
    this.setState({ timerId: timerId });
  }

  stopTimer() {
    clearInterval(this.state.timerId);
  }

  timer() {
    if (this.state.cooldown > 1) {
      this.setState({ cooldown: this.state.cooldown - 1 });
    } else {
      this.setState({ cooldown: -1 });
      this.stopTimer();
    }
  }

  handleClick = () => {
    this.props.handler()
    this.setState({ showClicked: true });
    setTimeout(this.hideImage, 100);
  }

  hideImage() {
    this.setState({ showClicked: false });
  }

  render() {
    const title = this.state.active
      ? "Gathering — move to stop"
      : this.props.title;
    const spanStyle = {
      transform: 'translate(0px, 13px)',
      position: 'fixed',
      fontFamily: 'Verdana',
      fontSize: '30px',
      width: '50px',
      height: '50px',
      color: 'white',
      textAlign: 'center',
      WebkitTextStroke: '1px black'
    } as React.CSSProperties

    const cooldownBgStyle = {
      position: 'fixed',
      opacity: 0.5,
    } as React.CSSProperties

    return (
      <div id='gatherbutton' className={this.props.className}>
        {this.state.cooldown != -1 &&
          <img src={cooldownbg} style={cooldownBgStyle} />}

        {this.state.cooldown != -1 &&
          <span style={spanStyle}>{this.state.cooldown}</span>}

        {!this.state.showClicked && <img src={gatherbuttonimage}
          title={title}
          alt={title}
          aria-label={title}
          onClick={this.handleClick} />}

        {this.state.showClicked && <img src={'/static/art/ui/gatherbutton_click.png'}
          title={title}
          alt={title}
          aria-label={title} />}

        {this.state.active && this.props.activeClassName &&
          <img
            src={smalliconborder}
            className={this.props.activeClassName}
            alt=""
            aria-hidden="true" />}

      </div>
    );
  }
}
