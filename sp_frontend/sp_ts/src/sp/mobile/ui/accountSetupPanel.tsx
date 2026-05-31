import * as React from "react";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { MOBILE_DIALOG_Z } from "./mobileLayers";

interface AccountSetupProps {
  errorMessage: string;
}

interface AccountSetupState {
  accountName: string;
  email: string;
  password: string;
  confirmPassword: string;
  validationError: string;
}

export default class AccountSetupPanel extends React.Component<AccountSetupProps, AccountSetupState> {
  constructor(props) {
    super(props);

    this.state = {
      accountName: '',
      email: '',
      password: '',
      confirmPassword: '',
      validationError: '',
    };

    this.handleAccountNameChange = this.handleAccountNameChange.bind(this);
    this.handleEmailChange = this.handleEmailChange.bind(this);
    this.handlePasswordChange = this.handlePasswordChange.bind(this);
    this.handleConfirmPasswordChange = this.handleConfirmPasswordChange.bind(this);
    this.handleFormSubmit = this.handleFormSubmit.bind(this);
    this.handleSubmit = this.handleSubmit.bind(this);
    this.handleSkip = this.handleSkip.bind(this);
  }

  handleAccountNameChange(event) {
    this.setState({ accountName: event.target.value, validationError: '' });
  }

  handleEmailChange(event) {
    this.setState({ email: event.target.value, validationError: '' });
  }

  handlePasswordChange(event) {
    this.setState({ password: event.target.value, validationError: '' });
  }

  handleConfirmPasswordChange(event) {
    this.setState({ confirmPassword: event.target.value, validationError: '' });
  }

  handleFormSubmit(event) {
    if (event) event.preventDefault();
    this.handleSubmit();
  }

  handleSubmit() {
    const { accountName, email, password, confirmPassword } = this.state;

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

    const trimmedEmail = email.trim();
    if (trimmedEmail.length > 0 && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(trimmedEmail)) {
      this.setState({ validationError: 'Enter a valid email, or leave it blank' });
      return;
    }

    Global.gameEmitter.emit(GameEvent.ACCOUNT_SETUP_SUBMIT, { accountName, password, email: trimmedEmail });
  }

  handleSkip(event?) {
    if (event) event.preventDefault();
    Global.gameEmitter.emit(GameEvent.ACCOUNT_SETUP_SKIP, {});
  }

  render() {
    const errorMessage = this.props.errorMessage || this.state.validationError;

    // Dimmed full-screen backdrop, matching the mobile leaderboard overlay.
    const overlayStyle: React.CSSProperties = {
      position: 'fixed',
      top: 0, left: 0, right: 0, bottom: 0,
      background: 'rgba(12, 14, 17, 0.75)',
      zIndex: MOBILE_DIALOG_Z,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      padding: 'calc(16px + env(safe-area-inset-top, 0px)) 16px calc(16px + env(safe-area-inset-bottom, 0px))',
      boxSizing: 'border-box',
      overflowY: 'auto',
    };

    // Dark card using the same palette as the login screen / leaderboard card.
    const panelStyle: React.CSSProperties = {
      background: '#2c3338',
      border: '1px solid #3b4148',
      borderRadius: '8px',
      boxShadow: '0 18px 36px rgba(0, 0, 0, 0.45)',
      padding: '24px 20px',
      boxSizing: 'border-box',
    };

    const titleStyle: React.CSSProperties = {
      color: '#ea4c4c',
      fontFamily: "'Open Sans', sans-serif",
      fontSize: '1.1rem',
      fontWeight: 700,
      letterSpacing: '0.12em',
      textTransform: 'uppercase',
      textAlign: 'center',
      margin: '0 0 0.4em',
    };

    const descStyle: React.CSSProperties = {
      color: '#b4bcc4',
      fontFamily: "'Open Sans', sans-serif",
      fontSize: '0.85rem',
      textAlign: 'center',
      margin: '0 0 1.25em',
    };

    const errorStyle: React.CSSProperties = {
      color: '#ea4c4c',
      fontFamily: "'Open Sans', sans-serif",
      fontSize: '0.8rem',
      fontWeight: 600,
      textAlign: 'center',
      margin: '0 0 0.5em',
    };

    // Matches the #login text/password inputs (type="email" isn't covered by the
    // shared login.css selector, so apply the same look to every field here).
    const inputStyle: React.CSSProperties = {
      backgroundColor: '#3b4148',
      borderRadius: '0 3px 3px 0',
      color: '#b4bcc4',
      height: '50px',
      padding: '0 16px',
      width: '230px',
      border: 'none',
      fontFamily: "'Open Sans', Arial, sans-serif",
      fontSize: '14px',
    };

    return (
      <div style={overlayStyle}>
        <div style={panelStyle}>
          <p style={titleStyle}>Secure Your Account</p>
          <p style={descStyle}>Choose an account name and password to secure your progress.</p>

          <div id="login">
            <form onSubmit={this.handleFormSubmit}>
              <p><span className="fontawesome-user"></span>
                <input
                  style={inputStyle}
                  type="text"
                  value={this.state.accountName}
                  onChange={this.handleAccountNameChange}
                  placeholder="Account Name"
                  autoCapitalize="none"
                  autoCorrect="off"
                />
              </p>
              <p><span className="fontawesome-envelope"></span>
                <input
                  style={inputStyle}
                  type="email"
                  value={this.state.email}
                  onChange={this.handleEmailChange}
                  placeholder="Email (optional)"
                  autoCapitalize="none"
                  autoCorrect="off"
                />
              </p>
              <p><span className="fontawesome-lock"></span>
                <input
                  style={inputStyle}
                  type="password"
                  value={this.state.password}
                  onChange={this.handlePasswordChange}
                  placeholder="Password"
                />
              </p>
              <p><span className="fontawesome-lock"></span>
                <input
                  style={inputStyle}
                  type="password"
                  value={this.state.confirmPassword}
                  onChange={this.handleConfirmPasswordChange}
                  placeholder="Confirm Password"
                />
              </p>

              {errorMessage && <p style={errorStyle}>{errorMessage}</p>}

              <p><input type="submit" className="form-button" value="Secure Account" /></p>
            </form>
            <p><a href="#" onClick={this.handleSkip}>Skip for now</a></p>
          </div>
        </div>
      </div>
    );
  }
}
