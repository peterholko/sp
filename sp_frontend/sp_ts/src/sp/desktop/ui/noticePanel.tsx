import * as React from "react";
import errorpanel from "ui_comp/buttonsframe.png";
import { Global } from "../../core/global";

const MAX_VISIBLE_NOTICES = 3;

interface NoticeData {
  id: number,
  message: string,
  expiryMs: number,
  createdAt: number,
}

interface NoticeStackProps {
  notifications: NoticeData[],
  onDismiss: (notificationId: number) => void,
}

interface NoticeToastProps {
  notice: NoticeData,
  onDismiss: (notificationId: number) => void,
}

class NoticeToast extends React.Component<NoticeToastProps, any> {
  private timer;

  constructor(props) {
    super(props);

    this.handleDismiss = this.handleDismiss.bind(this);
  }

  componentDidMount() {
    this.timer = setTimeout(() => {
      this.props.onDismiss(this.props.notice.id);
    }, this.props.notice.expiryMs);
  }

  componentWillUnmount() {
    clearTimeout(this.timer);
  }

  handleDismiss() {
    clearTimeout(this.timer);
    this.props.onDismiss(this.props.notice.id);
  }

  render() {
    const noticeStyle = {
      width: '315px',
      height: '67px',
      marginBottom: '6px',
      position: 'relative',
      cursor: 'pointer',
      pointerEvents: 'auto',
    } as React.CSSProperties

    const noticePanelStyle = {
      top: '0px',
      left: '0px',
      position: 'absolute'
    } as React.CSSProperties

    const spanNameStyle = {
      top: '10px',
      left: '7px',
      position: 'absolute',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '300px'
    } as React.CSSProperties

    return (
      <div style={noticeStyle} onClick={this.handleDismiss} role="button" aria-label="Dismiss notification">
        <img src={errorpanel} style={noticePanelStyle}/>
        <span style={spanNameStyle}>{this.props.notice.message}</span>
      </div>
    );
  }
}

export default class NoticeStack extends React.Component<NoticeStackProps, any> {
  render() {
    if (this.props.notifications.length == 0) {
      return null;
    }

    const orderedNotifications = this.props.notifications.slice().reverse();
    const visibleNotifications = orderedNotifications.slice(0, MAX_VISIBLE_NOTICES);
    const overflowCount = Math.max(0, orderedNotifications.length - MAX_VISIBLE_NOTICES);

    const stackStyle = {
      top: '20px',
      left: '50%',
      width: '315px',
      marginLeft: '-158px',
      position: 'fixed',
      zIndex: Global.zIndexManager.getTop() + 1,
      pointerEvents: 'none',
    } as React.CSSProperties

    const overflowStyle = {
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      lineHeight: '18px',
      textAlign: 'center',
      textShadow: '1px 1px 2px black',
      width: '315px',
    } as React.CSSProperties

    return (
      <div style={stackStyle}>
        {visibleNotifications.map((notification) =>
          <NoticeToast
            key={notification.id}
            notice={notification}
            onDismiss={this.props.onDismiss} />
        )}
        {overflowCount > 0 &&
          <div style={overflowStyle}>+{overflowCount}</div>}
      </div>
    );
  }
}
