import * as React from "react";
import okbutton from "ui_comp/okbutton.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";

interface AccountSetupProps {
  errorMessage: string;
}

interface AccountSetupState {
  accountName: string;
  password: string;
  confirmPassword: string;
  validationError: string;
}

export default class AccountSetupPanel extends React.Component<AccountSetupProps, AccountSetupState> {
  constructor(props) {
    super(props);

    this.state = {
      accountName: '',
      password: '',
      confirmPassword: '',
      validationError: '',
    };

    this.handleAccountNameChange = this.handleAccountNameChange.bind(this);
    this.handlePasswordChange = this.handlePasswordChange.bind(this);
    this.handleConfirmPasswordChange = this.handleConfirmPasswordChange.bind(this);
    this.handleSubmit = this.handleSubmit.bind(this);
    this.handleSkip = this.handleSkip.bind(this);
  }

  handleAccountNameChange(event) {
    this.setState({ accountName: event.target.value, validationError: '' });
  }

  handlePasswordChange(event) {
    this.setState({ password: event.target.value, validationError: '' });
  }

  handleConfirmPasswordChange(event) {
    this.setState({ confirmPassword: event.target.value, validationError: '' });
  }

  handleSubmit() {
    const { accountName, password, confirmPassword } = this.state;

    if (accountName.length < 3 || accountName.length > 20) {
      this.setState({ validationError: 'Name must be 3-20 characters' });
      return;
    }

    if (!/^[a-zA-Z0-9_]+$/.test(accountName)) {
      this.setState({ validationError: 'Name: letters, numbers, underscore only' });
      return;
    }

    if (password.length < 6) {
      this.setState({ validationError: 'Password must be at least 6 characters' });
      return;
    }

    if (password !== confirmPassword) {
      this.setState({ validationError: 'Passwords do not match' });
      return;
    }

    Global.gameEmitter.emit(GameEvent.ACCOUNT_SETUP_SUBMIT, { accountName, password });
  }

  handleSkip() {
    Global.gameEmitter.emit(GameEvent.ACCOUNT_SETUP_SKIP, {});
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
      gap: '14px',
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

    const skipStyle: React.CSSProperties = {
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
          <h2 style={titleStyle}>Secure Your Account</h2>
          <p style={descStyle}>Choose an account name and password to secure your progress.</p>

          <div style={fieldStyle}>
            <label style={labelStyle}>Account Name</label>
            <input
              style={inputStyle}
              type="text"
              value={this.state.accountName}
              onChange={this.handleAccountNameChange}
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

          <div style={fieldStyle}>
            <label style={labelStyle}>Confirm Password</label>
            <input
              style={inputStyle}
              type="password"
              value={this.state.confirmPassword}
              onChange={this.handleConfirmPasswordChange}
            />
          </div>

          {errorMessage && <p style={errorStyle}>{errorMessage}</p>}

          <div style={submitContainer}>
            <img src={okbutton} style={submitStyle} onClick={this.handleSubmit} alt="Submit" />
          </div>

          <span style={skipStyle} onClick={this.handleSkip}>Skip for now</span>
        </div>
      </div>
    );
  }
}
