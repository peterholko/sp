import React from "react";
import Game from "./game"
import UI from "./ui";
import { Global } from "./global";
import { Network } from "./network";
import { NetworkEvent } from "./networkEvent";
import { getFingerprint } from "./fingerprint";
import "./login.css"
import logo from "art/perilous_logo.png";
import warrior from "art/novicewarrior_single.png";
import ranger from "art/noviceranger_single.png";
import mage from "art/novicemage_single.png";
import halfpanel from "ui/halfpanel.png";
import leftArrowButton from "ui/leftbutton.png";
import rightArrowButton from "ui/rightbutton.png";
import IntroPanel from "./ui/introPanel";
import ErrorPanel from "./ui/errorPanel";
import AccountSetupPanel from "./ui/accountSetupPanel";
import { GameEvent } from "./gameEvent";
import TrueDeathPanel from "./ui/trueDeathPanel";

export default class LoginControl extends React.Component<any, any> {
  private readonly leaderboardPageSize = 5;
  private readonly accountSetupDelay = 60000; // 1 minute
  private healthIntervalId?: number;
  private accountSetupTimerId?: number;

  constructor(props) {
    super(props);

    this.state = {
      hideLandingPage: false,
      hideSelectClass: true,
      hideIntro: true,
      hideGame: true,
      hideError: true,
      hideTrueDeathPanel: true,
      hideAccountSetupPanel: true,
      showEnterWorld: false,
      showLeaderboard: false,
      leaderboardPage: 0,
      leaderboardPreviousPressed: false,
      leaderboardNextPressed: false,
      leaderboardEntries: [],
      heroName: '',
      selectedClass: '',
      isHeroNameEmpty: false,
      errorMessage: 'Play',
      firstRender: true,
      inappropiateName: false,
      takenName: false,
      trueDeathData: {},
      serverHealthLoading: true,
      serverHealthy: null,
      accountSetupError: '',
      preConnectionSelect: false,
      showLoginPanel: false,
      loginError: '',
      loginAccountName: '',
      loginPassword: '',
      loginButtonPressed: false,
    };

    this.handleHeroNameChange = this.handleHeroNameChange.bind(this);

    this.handleLoggedIn = this.handleLoggedIn.bind(this);

    this.handleWarriorSelect = this.handleWarriorSelect.bind(this);
    this.handleRangerSelect = this.handleRangerSelect.bind(this);
    this.handleMageSelect = this.handleMageSelect.bind(this);

    this.handleEnterWorld = this.handleEnterWorld.bind(this);
    this.handleShowLogin = this.handleShowLogin.bind(this);
    this.handleLoginAccountNameChange = this.handleLoginAccountNameChange.bind(this);
    this.handleLoginPasswordChange = this.handleLoginPasswordChange.bind(this);
    this.handleLoginFormSubmit = this.handleLoginFormSubmit.bind(this);
    this.handleLoginCancel = this.handleLoginCancel.bind(this);

    this.handleLeaderboardOpen = this.handleLeaderboardOpen.bind(this);
    this.handleLeaderboardClose = this.handleLeaderboardClose.bind(this);
    this.handleLeaderboardNext = this.handleLeaderboardNext.bind(this);
    this.handleLeaderboardPrevious = this.handleLeaderboardPrevious.bind(this);

    this.handleServerOffline = this.handleServerOffline.bind(this);
    this.handleNetworkError = this.handleNetworkError.bind(this);

    Global.gameEmitter.on(GameEvent.INTRO_OK_CLICK, this.handleIntroOkClick, this);
    Global.gameEmitter.on(GameEvent.ERROR_OK_CLICK, this.handleErrorOkClick, this);

    Global.gameEmitter.on(NetworkEvent.SELECT_CLASS, this.handleSelectClass, this);
    Global.gameEmitter.on(NetworkEvent.FIRST_LOGIN, this.handleFirstLogin, this);
    Global.gameEmitter.on(NetworkEvent.LOGGED_IN, this.handleLoggedIn, this);
    Global.gameEmitter.on(NetworkEvent.ERROR, this.handleError, this);

    Global.gameEmitter.on(NetworkEvent.SERVER_OFFLINE, this.handleServerOffline, this);
    Global.gameEmitter.on(NetworkEvent.NETWORK_ERROR, this.handleNetworkError, this);

    Global.gameEmitter.on(NetworkEvent.INFO_TRUE_DEATH, this.handleInfoTrueDeath, this);

    Global.gameEmitter.on(GameEvent.ACCOUNT_SETUP_SUBMIT, this.handleAccountSetupSubmit, this);
    Global.gameEmitter.on(GameEvent.ACCOUNT_SETUP_SKIP, this.handleAccountSetupSkip, this);
    Global.gameEmitter.on(GameEvent.HERO_DEAD, this.handleHeroDead, this);
  }


  async handleServerOffline() {

    console.log('Handle server offline');
    const healthUrl = `${window.location.origin}/health`;

    try {
      const response = await fetch(healthUrl, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      });

      if (!response.ok) {
        Global.serverOffline = true;
        this.setState({
          errorMessage: "Server is offline, please try again later.",
          hideError: false,
          hideTrueDeathPanel: true,
          hideGame: true,
          hideLandingPage: true,
          hideSelectClass: true,
          serverHealthy: false,
          serverHealthLoading: false,
        });
      }
    } catch (error) {
      Global.serverOffline = true;
      this.setState({
        errorMessage: "Server is offline, please try again later.",
        hideError: false,
        hideTrueDeathPanel: true,
        hideGame: true,
        hideLandingPage: true,
        hideSelectClass: true,
        serverHealthy: false,
        serverHealthLoading: false,
      });
    }
  }

  handleNetworkError() {
    Global.networkError = true;
    this.setState({ errorMessage: "Network error, click to reconnect.", hideError: false });
  }

  async fetchServerHealth() {
    const healthUrl = `${window.location.origin}/health`;

    try {
      const response = await fetch(healthUrl, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      });

      if (!response.ok) {
        throw new Error(`Health check failed with status ${response.status}`);
      }

      const data = await response.json();

      this.setState({
        serverHealthy: Boolean(data && data.healthy),
        serverHealthLoading: false,
      });
    } catch (error) {
      console.error('Error fetching server health:', error);
      this.setState({
        serverHealthy: false,
        serverHealthLoading: false,
      });
    }
  }

  async componentDidMount() {
    Global.connected = false;

    this.fetchServerHealth();
    this.healthIntervalId = window.setInterval(() => this.fetchServerHealth(), 10000);

    try {
      const url = `${window.location.origin}/session`;

      const response = await fetch(url, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      });

      if (!response.ok) {
        console.log('Session not found, showing enter world button');
        this.setState({ showEnterWorld: true });
      } else {
        const result = await response.json();
        console.log('Session found', result);

        if (result.device_token) {
          localStorage.setItem('deviceToken', result.device_token);
        }

        if (result.account_name) {
          Global.accountName = result.account_name;
          Global.accountSetupCompleted = true;
        }

        Global.network = new Network();
        Global.network.connect();
        Global.connected = true;
      }
    } catch (error) {
      console.error('Error checking session:', error);
      this.setState({ showEnterWorld: true });
    }

    this.loadLeaderboardEntries();
  }

  componentWillUnmount() {
    if (this.healthIntervalId) {
      window.clearInterval(this.healthIntervalId);
    }
    if (this.accountSetupTimerId) {
      window.clearTimeout(this.accountSetupTimerId);
    }
  }

  async fingerprintAuth() {
    try {
      const fingerprint = await getFingerprint();
      const deviceToken = localStorage.getItem('deviceToken');
      const url = `${window.location.origin}/fingerprint-auth`;

      const body: Record<string, string> = { fingerprint };
      if (deviceToken) {
        body.device_token = deviceToken;
      }

      const response = await fetch(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(body),
      });

      if (!response.ok) {
        try {
          const errorResult = await response.json();
          if (errorResult.error === 'password_required') {
            // Account is password-protected — show login form pre-filled with account name
            this.setState({
              hideLandingPage: true,
              showLoginPanel: true,
              loginError: '',
              loginAccountName: errorResult.account_name || '',
              loginPassword: '',
              loginButtonPressed: false,
            });
            return;
          }
        } catch (e) {
          // Could not parse error response, fall through to generic error
        }
        this.setState({ errorMessage: "Failed to connect. Please try again.", hideError: false });
      } else {
        const result = await response.json();
        console.log('Fingerprint authentication successful', result);

        if (result.device_token) {
          localStorage.setItem('deviceToken', result.device_token);
        }

        Global.playerId = result.playerId;
        if (result.hasAccount) {
          Global.accountSetupCompleted = true;
        }

        if (result.account_name) {
          Global.accountName = result.account_name;
        }

        if (result.newPlayer) {
          // New player: show hero selection before connecting to game server
          this.setState({
            hideLandingPage: true,
            hideSelectClass: false,
            preConnectionSelect: true,
          });
        } else {
          // Returning player: connect to game server directly
          Global.network = new Network();
          Global.network.connect();
          Global.connected = true;
        }
      }
    } catch (error) {
      console.error('Error during fingerprint authentication:', error);
      this.setState({ errorMessage: "Failed to connect. Please try again.", hideError: false });
    }
  }

  handleEnterWorld() {
    this.setState({ showEnterWorld: false, hideLandingPage: true });
    this.fingerprintAuth();
  }

  handleShowLogin() {
    this.setState({
      hideLandingPage: true,
      hideSelectClass: true,
      showLoginPanel: true,
      loginError: '',
      loginAccountName: '',
      loginPassword: '',
      loginButtonPressed: false,
    });
  }

  async passwordAuth(accountName: string, password: string) {
    try {
      const url = `${window.location.origin}/auth`;
      const response = await fetch(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ account_name: accountName, password }),
      });

      if (!response.ok) {
        const result = await response.json().catch(() => ({}));
        this.setState({ loginError: result.error || 'Login failed. Please try again.', loginButtonPressed: false });
        return;
      }

      const result = await response.json();
      console.log('Password authentication successful', result);

      if (result.device_token) {
        localStorage.setItem('deviceToken', result.device_token);
      }

      Global.playerId = result.playerId;
      Global.accountSetupCompleted = true;
      Global.accountName = accountName;

      this.setState({ showLoginPanel: false, loginError: '', loginButtonPressed: false, loginAccountName: '', loginPassword: '' });

      if (result.newPlayer) {
        this.setState({
          hideLandingPage: true,
          hideSelectClass: false,
          preConnectionSelect: true,
        });
      } else {
        Global.network = new Network();
        Global.network.connect();
        Global.connected = true;
      }
    } catch (error) {
      console.error('Error during password authentication:', error);
      this.setState({ loginError: 'Network error. Please try again.', loginButtonPressed: false });
    }
  }

  handleLoginAccountNameChange(event) {
    this.setState({ loginAccountName: event.target.value, loginError: '' });
  }

  handleLoginPasswordChange(event) {
    this.setState({ loginPassword: event.target.value, loginError: '' });
  }

  handleLoginFormSubmit(event) {
    event.preventDefault();
    const { loginAccountName, loginPassword } = this.state;

    if (loginAccountName.length === 0) {
      this.setState({ loginError: 'Please enter your account name' });
      return;
    }

    if (loginPassword.length === 0) {
      this.setState({ loginError: 'Please enter your password' });
      return;
    }

    this.setState({ loginButtonPressed: true });
    this.passwordAuth(loginAccountName, loginPassword);
  }

  handleLoginCancel(event?) {
    if (event) event.preventDefault();
    this.setState({
      showLoginPanel: false,
      loginError: '',
      loginAccountName: '',
      loginPassword: '',
      loginButtonPressed: false,
      hideLandingPage: false,
      showEnterWorld: true,
    });
  }


  startAccountSetupTimer() {
    if (Global.accountSetupCompleted) {
      return;
    }
    if (this.accountSetupTimerId) {
      window.clearTimeout(this.accountSetupTimerId);
    }
    this.accountSetupTimerId = window.setTimeout(() => {
      if (!Global.accountSetupCompleted && !Global.heroDead) {
        this.setState({ hideAccountSetupPanel: false, accountSetupError: '' });
      }
    }, this.accountSetupDelay);
  }

  async handleAccountSetupSubmit(data) {
    const { accountName, password } = data;
    try {
      const url = `${window.location.origin}/register`;
      const response = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ account_name: accountName, password }),
      });

      if (!response.ok) {
        const result = await response.json();
        this.setState({ accountSetupError: result.error || 'Failed to save account. Please try again.' });
        return;
      }

      Global.accountSetupCompleted = true;
      this.setState({ hideAccountSetupPanel: true, accountSetupError: '' });
    } catch (error) {
      console.error('Error during account setup:', error);
      this.setState({ accountSetupError: 'Network error. Please try again.' });
    }
  }

  handleAccountSetupSkip() {
    this.setState({ hideAccountSetupPanel: true, accountSetupError: '' });
  }

  handleHeroDead() {
    if (this.accountSetupTimerId) {
      window.clearTimeout(this.accountSetupTimerId);
      this.accountSetupTimerId = undefined;
    }
    this.setState({ hideAccountSetupPanel: true });
  }

  async loadLeaderboardEntries() {
    try {
      const url = `${window.location.origin}/scores`;

      const response = await fetch(url, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      });

      if (!response.ok) {
        console.error('Failed to fetch leaderboard entries');
        return;
      }

      const data = await response.json();
      if (!Array.isArray(data)) {
        console.error('Leaderboard response is not an array');
        return;
      }

      const leaderboardEntries = data
        .slice()
        .sort((a, b) => {
          const totalXpA = typeof a.total_xp === 'number' ? a.total_xp : Number(a.total_xp) || 0;
          const totalXpB = typeof b.total_xp === 'number' ? b.total_xp : Number(b.total_xp) || 0;

          if (totalXpB !== totalXpA) {
            return totalXpB - totalXpA;
          }

          const heroNameA = typeof a.hero_name === 'string' ? a.hero_name : '';
          const heroNameB = typeof b.hero_name === 'string' ? b.hero_name : '';
          return heroNameA.localeCompare(heroNameB);
        })
        .map(entry => ({
          id: entry.id,
          heroName: entry.hero_name,
          heroRank: entry.hero_rank,
          totalXp: typeof entry.total_xp === 'number' ? entry.total_xp.toLocaleString() : entry.total_xp,
          fate: entry.fate,
        }));

      this.setState({ leaderboardEntries });
    } catch (error) {
      console.error('Error fetching leaderboard entries:', error);
    }
  }

  handleError(data) {
    // Do not show error if the user is connected
    if (!Global.connected) {
      this.setState({ errorMessage: data.errmsg, hideError: false });
    }

    if (data.errmsg == 'Hero name is inappropriate') {
      this.setState({ inappropiateName: true });
    }

    if (data.errmsg == 'Hero name is already taken') {
      this.setState({ takenName: true });
    }
  }

  async handleErrorOkClick() {
    if (Global.serverOffline) {
      console.log('ErrorOkClick: server offline');
      try {
        const url = `${window.location.origin}/logout`;

        const response = await fetch(url, {
          method: 'POST',
          headers: {
            'Content-Type': 'application/json',
          },
        });

        if (!response.ok) {
          console.error('Failed to logout');
        }
      } catch (error) {
        console.error('Error logging out:', error);
      }

      Global.serverOffline = false;

      this.setState({
        hideLandingPage: false,
        hideSelectClass: true,
        hideTrueDeathPanel: true,
        hideGame: true,
        hideError: true,
        showEnterWorld: true,
      });
    } else if (Global.networkError) {
      console.log('ErrorOkClick: network error');
      Global.networkError = false;
      Global.network.connect();

      this.setState({ hideError: true });
    } else {
      this.setState({ hideError: true });
    }
  }

  handleIntroOkClick() {
    this.setState({ hideIntro: true });
    Global.network.sendSelectedClass(this.state.selectedClass, this.state.heroName);
  }

  handleHeroNameChange(event) {
    if (this.state.inappropiateName) {
      this.setState({ inappropiateName: false });
    }

    if (this.state.takenName) {
      this.setState({ takenName: false });
    }

    this.setState({ heroName: event.target.value });
  }

  handleSelectClass() {
    this.setState({
      hideLandingPage: true,
      hideSelectClass: false,
      hideTrueDeathPanel: true,
      hideGame: true
    });
  }

  handleFirstLogin() {
    this.setState({
      hideLandingPage: true,
      hideSelectClass: true,
      hideTrueDeathPanel: true,
      hideGame: false
    });

    fetch(`${window.location.origin}/set-display-name`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ hero_name: this.state.heroName }),
    }).then(async (res) => {
      if (res.ok) {
        const data = await res.json();
        if (data.account_name) {
          Global.accountName = data.account_name;
        }
      }
    }).catch(() => {});

    this.startAccountSetupTimer();
  }

  handleLoggedIn(data?) {
    this.setState({
      hideLandingPage: true,
      hideSelectClass: true,
      hideTrueDeathPanel: true,
      hideGame: false
    });
    if (data && data.has_account) {
      Global.accountSetupCompleted = true;
    }
    this.startAccountSetupTimer();
  }

  handleInfoTrueDeath(message) {
    this.setState({
      hideLandingPage: true,
      hideSelectClass: true,
      hideGame: true,
      hideTrueDeathPanel: false,
      trueDeathData: message
    });
  }

  handleClassSelect(className: string) {
    if (this.state.heroName == '') {
      this.setState({ isHeroNameEmpty: true });
    } else if (this.state.preConnectionSelect) {
      // New player: store selection and connect to game server
      Global.pendingClassSelection = { className, heroName: this.state.heroName };
      Global.network = new Network();
      Global.network.connect();
      Global.connected = true;
      this.setState({ hideSelectClass: true, preConnectionSelect: false });
    } else {
      Global.network.sendSelectedClass(className, this.state.heroName);
    }
  }

  handleWarriorSelect() {
    this.handleClassSelect("Warrior");
  }

  handleRangerSelect() {
    this.handleClassSelect("Ranger");
  }

  handleMageSelect() {
    this.handleClassSelect("Mage");
  }

  handleLeaderboardOpen(event) {
    event.preventDefault();
    this.loadLeaderboardEntries();
    this.setState({
      showLeaderboard: true,
      leaderboardPage: 0,
      leaderboardPreviousPressed: false,
      leaderboardNextPressed: false
    });
  }

  handleLeaderboardClose(event) {
    event.preventDefault();
    this.setState({
      showLeaderboard: false,
      leaderboardPreviousPressed: false,
      leaderboardNextPressed: false
    });
  }

  handleLeaderboardNext() {
    this.setState(prevState => {
      const totalPages = Math.ceil(prevState.leaderboardEntries.length / this.leaderboardPageSize);
      const nextPage = Math.min(prevState.leaderboardPage + 1, Math.max(totalPages - 1, 0));

      return {
        leaderboardPage: nextPage,
        leaderboardNextPressed: true,
        leaderboardPreviousPressed: false
      };
    });
  }

  handleLeaderboardPrevious() {
    this.setState(prevState => {
      const previousPage = Math.max(prevState.leaderboardPage - 1, 0);

      return {
        leaderboardPage: previousPage,
        leaderboardPreviousPressed: true,
        leaderboardNextPressed: false
      };
    });
  }

  render() {
    const logoStyle = {
    }

    const totalPages = Math.ceil(this.state.leaderboardEntries.length / this.leaderboardPageSize);
    const currentPage = Math.min(this.state.leaderboardPage, Math.max(totalPages - 1, 0));
    const paginatedEntries = this.state.leaderboardEntries.slice(
      currentPage * this.leaderboardPageSize,
      (currentPage + 1) * this.leaderboardPageSize
    );

    const warriorStyle = {
      transform: 'translate(40px, 155px)',
      position: 'fixed'
    } as React.CSSProperties

    const rangerStyle = {
      transform: 'translate(140px, 155px)',
      position: 'fixed'
    } as React.CSSProperties

    const mageStyle = {
      transform: 'translate(240px, 155px)',
      position: 'fixed'
    } as React.CSSProperties

    const selectClassStyle = {
      top: '50%',
      left: '50%',
      width: '360px',
      height: '323px',
      marginTop: '-161px',
      marginLeft: '-180px',
      position: 'fixed'
    } as React.CSSProperties

    const selectClassBGStyle = {
      position: 'fixed',
      WebkitTransform: 'rotate(90deg)',
      transform: 'rotate(90deg) translate(-19px, -18px)'
    } as React.CSSProperties

    const selectHeroNameText = {
      transform: 'translate(50px, 50px)',
      position: 'fixed',
      textAlign: 'left',
      color: '#FFFFF0',
      fontFamily: 'Cinzel',
      fontSize: '16px',
      width: '360px'
    } as React.CSSProperties

    const selectHeroClassText = {
      transform: 'translate(50px, 125px)',
      position: 'fixed',
      textAlign: 'left',
      color: '#FFFFF0',
      fontFamily: 'Cinzel',
      fontSize: '16px',
      width: '360px'
    } as React.CSSProperties

    const warriorText = {
      transform: 'translate(48px, 230px)',
      position: 'fixed',
      textAlign: 'center',
      color: '#FFFFF0',
      fontFamily: 'Cinzel',
      fontSize: '14px',
    } as React.CSSProperties

    const rangerText = {
      transform: 'translate(152px, 230px)',
      position: 'fixed',
      textAlign: 'center',
      color: '#FFFFF0',
      fontFamily: 'Cinzel',
      fontSize: '14px',
    } as React.CSSProperties

    const mageText = {
      transform: 'translate(256px, 230px)',
      position: 'fixed',
      textAlign: 'center',
      color: '#FFFFF0',
      fontFamily: 'Cinzel',
      fontSize: '14px',
    } as React.CSSProperties

    const { serverHealthLoading, serverHealthy } = this.state;

    let serverStatusClass = 'server-status--checking';
    let serverStatusLabel = 'Checking server status';

    if (!serverHealthLoading) {
      if (serverHealthy) {
        serverStatusClass = 'server-status--online';
        serverStatusLabel = 'Server Online';
      } else {
        serverStatusClass = 'server-status--offline';
        serverStatusLabel = 'Server Offline';
      }
    }

    const selectHeroInput = {
      position: 'fixed',
      transform: 'translate(175px, 41px)',
      backgroundColor: '#363b41',
      borderRadius: '3px 3px 3px 3px',
      color: '#b4bcc4',
      display: 'block',
      float: 'left',
      height: '35px',
      lineHeight: '50px',
      textAlign: 'center',
      width: '150px',
      zIndex: '5',
      border: this.state.isHeroNameEmpty ? '1px solid red' : 'none',
      boxShadow: this.state.isHeroNameEmpty ? '0 0 10px #719ECE' : 'none'
    } as React.CSSProperties

    const nameErrorText = {
      transform: 'translate(192px, 80px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'red',
      fontSize: '12px',
      fontWeight: 'bold'
    } as React.CSSProperties

    return (
      <div>
        {!this.state.hideLandingPage && (
          <div className="container">
            <img src={logo} style={logoStyle} />
            <div id="login">
              <div className={`server-status ${serverStatusClass}`} role="status" aria-live="polite">
                <span className="server-status__indicator" aria-hidden="true"></span>
                <div className="server-status__details">
                  <span className="server-status__label">{serverStatusLabel}</span>
                </div>
              </div>

              {this.state.showEnterWorld ? (
                <p style={{ textAlign: 'center', marginTop: '1.5em' }}>
                  <button type="button" className="enter-world-button" onClick={this.handleEnterWorld}>Enter World</button>
                </p>
              ) : (
                <p style={{ textAlign: 'center', color: '#b4bcc4' }}>Connecting...</p>
              )}

              <p className="leaderboard-link">
                <button type="button" className="leaderboard-button" onClick={this.handleLeaderboardOpen}>View Leaderboard</button>
              </p>
            </div>
          </div>
        )
        }

        {!this.state.hideSelectClass && (
          <>
            <div style={selectClassStyle}>
              <img src={halfpanel} style={selectClassBGStyle} />
              <span style={selectHeroNameText}>Hero's Name: </span>
              <input style={selectHeroInput} type="text" autoFocus onChange={this.handleHeroNameChange} />
              <span style={selectHeroClassText}>Hero's Class:</span>
              <img src={warrior} style={warriorStyle} onClick={this.handleWarriorSelect} />
              <span style={warriorText}>Warrior</span>
              <img src={ranger} style={rangerStyle} onClick={this.handleRangerSelect} />
              <span style={rangerText}>Ranger</span>
              <img src={mage} style={mageStyle} onClick={this.handleMageSelect} />
              <span style={mageText}>Mage</span>
              {this.state.inappropiateName &&
                <span style={nameErrorText}>Inappropiate Name</span>
              }

              {this.state.takenName &&
                <span style={nameErrorText}>Name Already Taken</span>
              }
            </div>

            <p className="existing-account-link--select-class">
              <span className="existing-account-text-link" onClick={this.handleShowLogin}>Already have an account? Log in</span>
            </p>
          </>
        )}

        {!this.state.hideIntro && (
          <IntroPanel />
        )}

        {!this.state.hideError && (
          <ErrorPanel errmsg={this.state.errorMessage} yOffset={70} />
        )}

        {this.state.showLoginPanel && (
          <div className="container">
            <img src={logo} style={logoStyle} />
            <div id="login">
              <div className={`server-status ${serverStatusClass}`} role="status" aria-live="polite">
                <span className="server-status__indicator" aria-hidden="true"></span>
                <div className="server-status__details">
                  <span className="server-status__label">{serverStatusLabel}</span>
                </div>
              </div>
              <form onSubmit={this.handleLoginFormSubmit}>
                <p><span className="fontawesome-user"></span>
                  <input type="text"
                    value={this.state.loginAccountName}
                    onChange={this.handleLoginAccountNameChange}
                    placeholder="Account Name"
                    autoFocus />
                </p>
                <p><span className="fontawesome-lock"></span>
                  <input type="password"
                    value={this.state.loginPassword}
                    onChange={this.handleLoginPasswordChange}
                    placeholder="Password" />
                </p>
                {this.state.loginError && (
                  <p style={{ color: '#ea4c4c', fontSize: '12px', textAlign: 'center' }}>{this.state.loginError}</p>
                )}
                <p><input
                  type="submit"
                  value={this.state.loginButtonPressed ? 'Logging in...' : 'Log In'}
                  className={`form-button${this.state.loginButtonPressed ? ' form-button--pressed' : ''}`}
                  disabled={this.state.loginButtonPressed}
                  aria-busy={this.state.loginButtonPressed}
                /></p>
              </form>
              <p><a href="#" onClick={this.handleLoginCancel}>Back</a></p>
            </div>
          </div>
        )}

        {!this.state.hideGame && (
          <div id="gameContainer" className="gameContainer">
            <UI />
            <Game />
          </div>
        )
        }

        {!this.state.hideAccountSetupPanel && !this.state.hideGame && (
          <AccountSetupPanel errorMessage={this.state.accountSetupError} />
        )}

        {!this.state.hideTrueDeathPanel &&
          <TrueDeathPanel
            heroName={this.state.trueDeathData.hero_name}
            heroRank={this.state.trueDeathData.hero_rank}
            totalXp={this.state.trueDeathData.total_xp}
            fate={this.state.trueDeathData.fate} />
        }

        {this.state.showLeaderboard && (
          <div className="leaderboard-overlay">
            <div className="leaderboard-container">
              <section className="leaderboard-card">
                <button type="button" className="leaderboard-close" onClick={this.handleLeaderboardClose} aria-label="Close leaderboard">×</button>
                <h1>Hall of Heroes</h1>
                <table>
                  <thead>
                    <tr>
                      <th>Hero Name</th>
                      <th>Hero Rank</th>
                      <th>Total XP</th>
                      <th>Fate</th>
                    </tr>
                  </thead>
                  <tbody>
                    {paginatedEntries.map(entry => (
                      <tr key={entry.id}>
                        <td>{entry.heroName}</td>
                        <td className="rank">{entry.heroRank}</td>
                        <td>{entry.totalXp}</td>
                        <td className="fate">{entry.fate}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <div className="leaderboard-pagination">

                  <button
                    type="button"
                    className="leaderboard-arrow-button"
                    onClick={this.handleLeaderboardPrevious}
                    aria-label="Show previous leaderboard page"
                  >
                    <img src={leftArrowButton} alt="Previous page" />
                  </button>

                  <span>Page {totalPages === 0 ? 0 : currentPage + 1} of {Math.max(totalPages, 1)}</span>
                  <button
                    type="button"
                    className="leaderboard-arrow-button"
                    onClick={this.handleLeaderboardNext}
                    aria-label="Show next leaderboard page"
                  >
                    <img src={rightArrowButton} alt="Next page" />
                  </button>

                </div>
              </section>
            </div>
          </div>
        )}

      </div>
    );
  }
}
