import * as React from "react";
import errorpanel from "ui_comp/buttonsframe.png";
import { MOBILE_NOTICE_Z } from "./mobileLayers";

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
      width: '100%',
      minHeight: '67px',
      marginBottom: '6px',
      position: 'relative',
      cursor: 'pointer',
      pointerEvents: 'auto',
    } as React.CSSProperties

    const noticePanelStyle = {
      inset: 0,
      width: '100%',
      height: '67px',
      position: 'absolute',
      objectFit: 'fill',
    } as React.CSSProperties

    const spanNameStyle = {
      top: '10px',
      left: '10px',
      right: '10px',
      position: 'absolute',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      lineHeight: 1.25,
      overflowWrap: 'anywhere',
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
      top: 'calc(132px + env(safe-area-inset-top, 0px))',
      left: '50%',
      width: 'min(315px, calc(100vw - 16px - env(safe-area-inset-left, 0px) - env(safe-area-inset-right, 0px)))',
      transform: 'translateX(-50%)',
      position: 'fixed',
      zIndex: MOBILE_NOTICE_Z,
      pointerEvents: 'none',
    } as React.CSSProperties

    const overflowStyle = {
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      lineHeight: '18px',
      textAlign: 'center',
      textShadow: '1px 1px 2px black',
      width: '100%',
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
