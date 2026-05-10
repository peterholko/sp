
import * as React from "react";
import errorpanel from "ui_comp/buttonsframe.png";
import okbutton from "ui_comp/okbutton.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";

interface NoticeProps {
  noticemsg,
  noticeExpiry,
}

export default class NoticePanel extends React.Component<NoticeProps, any> {
  private timer;

  constructor(props) {
    super(props);

    this.state = {

    };
   
  }

  componentWillUnmount() {
    clearInterval(this.timer);
  }

  componentDidMount() {
    this.timer = setInterval(() => {
      Global.gameEmitter.emit(GameEvent.NOTICE_EXPIRE, {});
    }, this.props.noticeExpiry);
  }

  render() {
    const noticeStyle = {
      top: '20px',
      left: '50%',
      width: '315px',
      height: '67px',
      marginLeft: '-148px',
      position: 'fixed',
      zIndex: Global.zIndexManager.getTop() + 1
    } as React.CSSProperties

    const noticePanelStyle = {
      position: 'fixed'
    } as React.CSSProperties

    const spanNameStyle = {
      transform: 'translate(7px, 10px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '300px'
    } as React.CSSProperties

    return (
      <div style={noticeStyle}>
        <img src={errorpanel} style={noticePanelStyle}/>
        <span style={spanNameStyle}>{this.props.noticemsg}</span>
      </div>
    );
  }
}

