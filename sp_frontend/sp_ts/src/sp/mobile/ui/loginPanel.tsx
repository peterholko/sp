import * as React from "react";
import okbutton from "ui_comp/okbutton.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";

interface LoginPanelProps {
  errorMessage: string;
}

interface LoginPanelState {
  accountName: string;
  password: string;
  validationError: string;
}

export default class LoginPanel extends React.Component<LoginPanelProps, LoginPanelState> {
  constructor(props) {
    super(props);

    this.state = {
      accountName: '',
      password: '',
      validationError: '',
    };

    this.handleAccountNameChange = this.handleAccountNameChange.bind(this);
    this.handlePasswordChange = this.handlePasswordChange.bind(this);
    this.handleSubmit = this.handleSubmit.bind(this);
    this.handleCancel = this.handleCancel.bind(this);
  }

  handleAccountNameChange(event) {
    this.setState({ accountName: event.target.value, validationError: '' });
  }

  handlePasswordChange(event) {
    this.setState({ password: event.target.value, validationError: '' });
  }

  handleSubmit() {
    const { accountName, password } = this.state;

    if (accountName.length === 0) {
      this.setState({ validationError: 'Please enter your account name' });
      return;
    }

    if (password.length === 0) {
      this.setState({ validationError: 'Please enter your password' });
      return;
    }

    Global.gameEmitter.emit(GameEvent.LOGIN_SUBMIT, { accountName, password });
  }

  handleCancel() {
    Global.gameEmitter.emit(GameEvent.LOGIN_CANCEL, {});
  }

  render() {
    const errorMessage = this.props.errorMessage || this.state.validationError;

    const overlayStyle: React.CSSProperties = {
      position: 'fixed',
      top: 0, left: 0, right: 0, bottom: 0,
      background: 'rgba(0, 0, 0, 0.7)',
      zIndex: Global.zIndexManager.getTop() + 1,
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
      gap: '16px',
    };

    const titleStyle: React.CSSProperties = {
      textAlign: 'center',
      color: '#ea4c4c',
      fontFamily: 'Cinzel',
      fontSize: '20px',
      letterSpacing: '0.12em',
      textTransform: 'uppercase',
      margin: 0,
    };

    const descStyle: React.CSSProperties = {
      textAlign: 'center',
      color: '#FFFFF0',
      fontFamily: 'Cinzel',
      fontSize: '13px',
      lineHeight: 1.4,
      margin: 0,
    };

    const fieldStyle: React.CSSProperties = {
      display: 'flex',
      flexDirection: 'column',
      gap: '6px',
    };

    const labelStyle: React.CSSProperties = {
      color: '#b4bcc4',
      fontFamily: 'Cinzel',
      fontSize: '14px',
    };

    const inputStyle: React.CSSProperties = {
      backgroundColor: '#363b41',
      borderRadius: '4px',
      color: '#ffffff',
      height: '48px',
      padding: '0 12px',
      width: '100%',
      border: 'none',
      fontSize: '16px',
      fontFamily: 'Open Sans, Arial, sans-serif',
      boxSizing: 'border-box',
    };

    const errorStyle: React.CSSProperties = {
      textAlign: 'center',
      color: '#ea4c4c',
      fontSize: '13px',
      fontWeight: 'bold',
      margin: 0,
    };

    const submitContainer: React.CSSProperties = {
      display: 'flex',
      justifyContent: 'center',
      paddingTop: '4px',
    };

    const submitStyle: React.CSSProperties = {
      cursor: 'pointer',
    };

    const backStyle: React.CSSProperties = {
      textAlign: 'center',
      color: '#b4bcc4',
      fontFamily: 'Cinzel',
      fontSize: '14px',
      cursor: 'pointer',
      textDecoration: 'underline',
      padding: '12px',
      margin: 0,
      minHeight: '44px',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
    };

    return (
      <div style={overlayStyle}>
        <div style={cardStyle}>
          <h2 style={titleStyle}>Account Login</h2>
          <p style={descStyle}>This account is registered. Please log in with your account name and password.</p>

          <div style={fieldStyle}>
            <label style={labelStyle}>Account Name</label>
            <input
              style={inputStyle}
              type="text"
              value={this.state.accountName}
              onChange={this.handleAccountNameChange}
              autoFocus
              autoCapitalize="none"
              autoCorrect="off"
            />
          </div>

          <div style={fieldStyle}>
            <label style={labelStyle}>Password</label>
            <input
              style={inputStyle}
              type="password"
              value={this.state.password}
              onChange={this.handlePasswordChange}
            />
          </div>

          {errorMessage && <p style={errorStyle}>{errorMessage}</p>}

          <div style={submitContainer}>
            <img src={okbutton} style={submitStyle} onClick={this.handleSubmit} alt="Submit" />
          </div>

          <span style={backStyle} onClick={this.handleCancel}>Back</span>
        </div>
      </div>
    );
  }
}
