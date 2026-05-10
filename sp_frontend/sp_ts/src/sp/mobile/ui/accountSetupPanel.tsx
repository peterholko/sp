import * as React from "react";
import widepanel from "ui_comp/widepanel.png";
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

    const panelStyle = {
      top: '50%',
      left: '50%',
      width: '667px',
      height: '375px',
      marginTop: '-187px',
      marginLeft: '-333px',
      position: 'fixed',
      zIndex: Global.zIndexManager.getTop() + 1,
    } as React.CSSProperties;

    const bgStyle = {
      position: 'fixed',
    } as React.CSSProperties;

    const titleStyle = {
      transform: 'translate(20px, 30px)',
      position: 'fixed',
      textAlign: 'center',
      color: '#ea4c4c',
      fontFamily: 'Cinzel',
      fontSize: '20px',
      width: '620px',
      letterSpacing: '0.12em',
      textTransform: 'uppercase',
    } as React.CSSProperties;

    const descStyle = {
      transform: 'translate(20px, 70px)',
      position: 'fixed',
      textAlign: 'center',
      color: '#FFFFF0',
      fontFamily: 'Cinzel',
      fontSize: '13px',
      width: '620px',
    } as React.CSSProperties;

    const labelStyle = {
      position: 'fixed',
      textAlign: 'right',
      color: '#b4bcc4',
      fontFamily: 'Cinzel',
      fontSize: '14px',
      width: '200px',
    } as React.CSSProperties;

    const inputStyle = {
      backgroundColor: '#363b41',
      borderRadius: '3px',
      color: '#b4bcc4',
      display: 'block',
      height: '35px',
      lineHeight: '35px',
      textAlign: 'center',
      width: '200px',
      border: 'none',
      fontSize: '14px',
      fontFamily: 'Open Sans, Arial, sans-serif',
    } as React.CSSProperties;

    const nameLabel = { ...labelStyle, transform: 'translate(100px, 115px)' } as React.CSSProperties;
    const nameInput = { ...inputStyle, transform: 'translate(320px, 108px)', position: 'fixed' } as React.CSSProperties;

    const passLabel = { ...labelStyle, transform: 'translate(100px, 165px)' } as React.CSSProperties;
    const passInput = { ...inputStyle, transform: 'translate(320px, 158px)', position: 'fixed' } as React.CSSProperties;

    const confirmLabel = { ...labelStyle, transform: 'translate(100px, 215px)' } as React.CSSProperties;
    const confirmInput = { ...inputStyle, transform: 'translate(320px, 208px)', position: 'fixed' } as React.CSSProperties;

    const errorStyle = {
      transform: 'translate(20px, 255px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'red',
      fontSize: '12px',
      fontWeight: 'bold',
      width: '620px',
    } as React.CSSProperties;

    const submitStyle = {
      transform: 'translate(308px, 285px)',
      position: 'fixed',
      cursor: 'pointer',
    } as React.CSSProperties;

    const skipStyle = {
      transform: 'translate(20px, 340px)',
      position: 'fixed',
      textAlign: 'center',
      color: '#606468',
      fontFamily: 'Cinzel',
      fontSize: '12px',
      width: '620px',
      cursor: 'pointer',
      textDecoration: 'underline',
    } as React.CSSProperties;

    return (
      <div style={panelStyle}>
        <img src={widepanel} style={bgStyle} />
        <span style={titleStyle}>Secure Your Account</span>
        <span style={descStyle}>Choose an account name and password to secure your progress.</span>

        <span style={nameLabel}>Account Name:</span>
        <input style={nameInput} type="text" value={this.state.accountName} onChange={this.handleAccountNameChange} />

        <span style={passLabel}>Password:</span>
        <input style={passInput} type="password" value={this.state.password} onChange={this.handlePasswordChange} />

        <span style={confirmLabel}>Confirm Password:</span>
        <input style={confirmInput} type="password" value={this.state.confirmPassword} onChange={this.handleConfirmPasswordChange} />

        {errorMessage && <span style={errorStyle}>{errorMessage}</span>}

        <img src={okbutton} style={submitStyle} onClick={this.handleSubmit} />
        <span style={skipStyle} onClick={this.handleSkip}>Skip for now</span>
      </div>
    );
  }
}
