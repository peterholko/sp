import * as React from "react";
import widepanel from "ui_comp/widepanel.png";
import okbutton from "ui_comp/okbutton.png";
import { Global } from "../global";
import { GameEvent } from "../gameEvent";

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

    const nameLabel = { ...labelStyle, transform: 'translate(100px, 140px)' } as React.CSSProperties;
    const nameInput = { ...inputStyle, transform: 'translate(320px, 133px)', position: 'fixed' } as React.CSSProperties;

    const passLabel = { ...labelStyle, transform: 'translate(100px, 195px)' } as React.CSSProperties;
    const passInput = { ...inputStyle, transform: 'translate(320px, 188px)', position: 'fixed' } as React.CSSProperties;

    const errorStyle = {
      transform: 'translate(20px, 240px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'red',
      fontSize: '12px',
      fontWeight: 'bold',
      width: '620px',
    } as React.CSSProperties;

    const submitStyle = {
      transform: 'translate(200px, 270px)',
      position: 'fixed',
      cursor: 'pointer',
    } as React.CSSProperties;

    const cancelStyle = {
      transform: 'translate(20px, 325px)',
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
        <span style={titleStyle}>Account Login</span>
        <span style={descStyle}>This account is registered. Please log in with your account name and password.</span>

        <span style={nameLabel}>Account Name:</span>
        <input style={nameInput} type="text" value={this.state.accountName} onChange={this.handleAccountNameChange} autoFocus />

        <span style={passLabel}>Password:</span>
        <input style={passInput} type="password" value={this.state.password} onChange={this.handlePasswordChange} />

        {errorMessage && <span style={errorStyle}>{errorMessage}</span>}

        <img src={okbutton} style={submitStyle} onClick={this.handleSubmit} />
        <span style={cancelStyle} onClick={this.handleCancel}>Back</span>
      </div>
    );
  }
}
