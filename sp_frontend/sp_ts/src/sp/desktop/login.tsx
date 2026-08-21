import React from "react";
import Game from "./game"
import UI from "./ui";
import { Global } from "../core/global";
import { Network } from "../core/network";
import { NetworkEvent } from "../core/networkEvent";
import "./login.css"
import logo from "art/perilous_logo.png";
import leftArrowButton from "ui/leftbutton.png";
import rightArrowButton from "ui/rightbutton.png";
import IntroPanel from "./ui/introPanel";
import ErrorPanel from "./ui/errorPanel";
import AccountSetupPanel from "./ui/accountSetupPanel";
import { GameEvent } from "../core/gameEvent";
import TrueDeathPanel from "./ui/trueDeathPanel";
import { isDesktop } from "../core/config";
import appStyles from "./app.module.css";
import HeroCreationPanel from "../core/heroCreationPanel";
import { DEFAULT_HERO_PORTRAIT } from "../core/portraitCatalog";
import {
  SAFE_LOGOUT_COMPLETION_MESSAGE,
  SafeLogoutResumeNoticeGuard,
  clearSafeLogoutReconnectSuppression,
  consumeSafeLogoutCompletion,
  hasSafeLogoutReconnectSuppression,
} from "../core/safeLogoutStatus";
import { shouldShowAccountSetupPrompt } from "../core/accountSetupPrompt";

export default class LoginControl extends React.Component<any, any> {
  private readonly leaderboardPageSize = 5;
  private healthIntervalId?: number;
  private accountSetupPrompted = false;
  private readonly safeLogoutResumeNotice = new SafeLogoutResumeNoticeGuard();

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
      selectedPortrait: DEFAULT_HERO_PORTRAIT,
      isHeroNameEmpty: false,
      isClassMissing: false,
      errorMessage: 'Play',
      firstRender: true,
      inappropiateName: false,
      takenName: false,
      trueDeathData: {},
      serverHealthLoading: true,
      serverHealthy: null,
      accountSetupError: '',
      accountSetupSubmitting: false,
      preConnectionSelect: false,
      showLoginPanel: false,
      loginError: '',
      loginAccountName: '',
      loginPassword: '',
      loginButtonPressed: false,
      showResetPanel: false,
      resetToken: '',
      resetPassword: '',
      resetConfirmPassword: '',
      resetButtonPressed: false,
      resetInfo: '',
      safeLogoutCompletionMessage: '',
    };

    this.handleHeroNameChange = this.handleHeroNameChange.bind(this);

    this.handleLoggedIn = this.handleLoggedIn.bind(this);

    this.handleWarriorSelect = this.handleWarriorSelect.bind(this);
    this.handleRangerSelect = this.handleRangerSelect.bind(this);
    this.handleMageSelect = this.handleMageSelect.bind(this);
    this.handlePortraitSelect = this.handlePortraitSelect.bind(this);
    this.handleCreateHero = this.handleCreateHero.bind(this);

    this.handleEnterWorld = this.handleEnterWorld.bind(this);
    this.handleShowLogin = this.handleShowLogin.bind(this);
    this.handleLoginAccountNameChange = this.handleLoginAccountNameChange.bind(this);
    this.handleLoginPasswordChange = this.handleLoginPasswordChange.bind(this);
    this.handleLoginFormSubmit = this.handleLoginFormSubmit.bind(this);
    this.handleLoginCancel = this.handleLoginCancel.bind(this);
    this.handleForgotPassword = this.handleForgotPassword.bind(this);
    this.handleResetPasswordChange = this.handleResetPasswordChange.bind(this);
    this.handleResetConfirmChange = this.handleResetConfirmChange.bind(this);
    this.handleResetSubmit = this.handleResetSubmit.bind(this);

    this.handleLeaderboardOpen = this.handleLeaderboardOpen.bind(this);
    this.handleLeaderboardClose = this.handleLeaderboardClose.bind(this);
    this.handleLeaderboardNext = this.handleLeaderboardNext.bind(this);
    this.handleLeaderboardPrevious = this.handleLeaderboardPrevious.bind(this);

    this.handleServerOffline = this.handleServerOffline.bind(this);
    this.handleNetworkError = this.handleNetworkError.bind(this);
    this.handleSafeLogoutComplete = this.handleSafeLogoutComplete.bind(this);
    this.handleSafeLogoutResumed = this.handleSafeLogoutResumed.bind(this);
    this.handleThreatState = this.handleThreatState.bind(this);

    Global.gameEmitter.on(GameEvent.INTRO_OK_CLICK, this.handleIntroOkClick, this);
    Global.gameEmitter.on(GameEvent.ERROR_OK_CLICK, this.handleErrorOkClick, this);

    Global.gameEmitter.on(NetworkEvent.SELECT_CLASS, this.handleSelectClass, this);
    Global.gameEmitter.on(NetworkEvent.FIRST_LOGIN, this.handleFirstLogin, this);
    Global.gameEmitter.on(NetworkEvent.LOGGED_IN, this.handleLoggedIn, this);
    Global.gameEmitter.on(NetworkEvent.ERROR, this.handleError, this);

    Global.gameEmitter.on(NetworkEvent.SERVER_OFFLINE, this.handleServerOffline, this);
    Global.gameEmitter.on(NetworkEvent.NETWORK_ERROR, this.handleNetworkError, this);
    Global.gameEmitter.on(NetworkEvent.SAFE_LOGOUT_COMPLETE, this.handleSafeLogoutComplete, this);
    Global.gameEmitter.on(NetworkEvent.SAFE_LOGOUT_RESUMED, this.handleSafeLogoutResumed, this);
    Global.gameEmitter.on(NetworkEvent.THREAT_STATE, this.handleThreatState, this);

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
      } else {
        Global.connected = false;
        Global.serverOffline = false;
        Global.networkError = true;
        this.setState({
          errorMessage: "Connection closed, click to reconnect.",
          hideError: false,
          serverHealthy: true,
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

  handleSafeLogoutComplete(data?) {
    Global.connected = false;
    Global.networkError = false;
    Global.serverOffline = false;
    this.safeLogoutResumeNotice.reset();
    this.setState({
      hideLandingPage: false,
      hideSelectClass: true,
      hideIntro: true,
      hideGame: true,
      hideError: true,
      hideTrueDeathPanel: true,
      hideAccountSetupPanel: true,
      showEnterWorld: true,
      safeLogoutCompletionMessage:
        data && typeof data.message === 'string'
          ? data.message
          : SAFE_LOGOUT_COMPLETION_MESSAGE,
    });
  }

  handleSafeLogoutResumed() {
    this.safeLogoutResumeNotice.receive();
    this.flushSafeLogoutResumeMessage();
  }

  flushSafeLogoutResumeMessage() {
    const message = this.safeLogoutResumeNotice.takeWhenReady(!this.state.hideGame);
    if (!message) {
      return;
    }

    Global.gameEmitter.emit(NetworkEvent.NOTICE, {
      noticemsg: message,
      expiry: Global.noticeExpiry,
    });
  }

  resetSafeLogoutResumeMessage() {
    this.safeLogoutResumeNotice.reset();
  }

  clearSafeLogoutSuppression() {
    try {
      clearSafeLogoutReconnectSuppression(window.sessionStorage);
    } catch (error) {
      console.warn('Unable to clear Safe Logout reconnect suppression', error);
    }
    this.resetSafeLogoutResumeMessage();
    this.setState({ safeLogoutCompletionMessage: '' });
  }

  resetNetworkForAuthentication() {
    if (Global.network && typeof Global.network.resetForAuthentication === 'function') {
      Global.network.resetForAuthentication();
    }
    this.clearSafeLogoutSuppression();
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

    const resetToken = new URLSearchParams(window.location.search).get('reset');
    if (resetToken) {
      // Arrived from a password-reset email link: show the reset form and skip
      // the normal session/auto-connect path.
      this.setState({ showResetPanel: true, resetToken, hideLandingPage: true });
      this.loadLeaderboardEntries();
      return;
    }

    try {
      const safeLogoutCompletionMessage = consumeSafeLogoutCompletion(window.sessionStorage);
      if (hasSafeLogoutReconnectSuppression(window.sessionStorage)) {
        this.setState({
          hideLandingPage: false,
          hideSelectClass: true,
          hideGame: true,
          hideError: true,
          showEnterWorld: true,
          safeLogoutCompletionMessage: safeLogoutCompletionMessage || '',
        });
        this.loadLeaderboardEntries();
        return;
      }
    } catch (error) {
      console.warn('Unable to restore Safe Logout completion state', error);
    }

    // A remounted login surface may still share the prior account's Network
    // singleton. Invalidate it before the asynchronous session check so none
    // of its cached state or delayed callbacks can cross the auth boundary.
    this.resetNetworkForAuthentication();

    try {
      const url = `${window.location.origin}/session`;

      const response = await fetch(url, {
        method: 'GET',
        headers: {
          'Content-Type': 'application/json',
        },
      });

      if (!response.ok) {
          // The trusted-device credential is HttpOnly, so the client cannot know
          // whether it exists. Always attempt a non-creating restore before
          // presenting the landing choices.
          await this.deviceAuth(false);
        } else {
          const result = await response.json();

          Global.playerId = result.playerId;
          Global.accountSetupCompleted = result.account_status === 'secured';
          if (result.account_name) {
            Global.accountName = result.account_name;
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
    Global.gameEmitter.off(NetworkEvent.SAFE_LOGOUT_COMPLETE, this.handleSafeLogoutComplete, this);
    Global.gameEmitter.off(NetworkEvent.SAFE_LOGOUT_RESUMED, this.handleSafeLogoutResumed, this);
    Global.gameEmitter.off(NetworkEvent.THREAT_STATE, this.handleThreatState, this);
    if (this.healthIntervalId) {
      window.clearInterval(this.healthIntervalId);
    }
  }

  async deviceAuth(createGuest = false) {
    this.resetNetworkForAuthentication();
    try {
      const url = `${window.location.origin}/device-auth`;

      const response = await fetch(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({ create_guest: createGuest }),
      });

      if (!response.ok) {
        try {
          const errorResult = await response.json();
          if (errorResult.error === 'authentication_required') {
            this.setState({
              hideLandingPage: false,
              showEnterWorld: true,
              hideError: true,
            });
            return;
          }
          this.setState({
            errorMessage: errorResult.error || "Failed to connect. Please try again.",
            hideError: false,
            showEnterWorld: true,
          });
          return;
        } catch (e) {
          // Could not parse error response, fall through to generic error
        }
        this.setState({ errorMessage: "Failed to connect. Please try again.", hideError: false });
      } else {
        const result = await response.json();

        Global.playerId = result.playerId;
        Global.accountSetupCompleted = result.accountStatus === 'secured';

        if (result.account_name) {
          Global.accountName = result.account_name;
        }

        if (result.needsHero || result.newPlayer) {
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
      console.error('Error during trusted-device authentication:', error);
      this.setState({ errorMessage: "Failed to connect. Please try again.", hideError: false });
    }
  }

  handleEnterWorld() {
    this.clearSafeLogoutSuppression();
    this.setState({ showEnterWorld: false, hideLandingPage: true });
    this.deviceAuth(true);
  }

  handleShowLogin() {
    this.resetNetworkForAuthentication();
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
    this.resetNetworkForAuthentication();
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

      Global.playerId = result.playerId;
      Global.accountSetupCompleted = true;
      Global.accountName = result.account_name || accountName;

      this.setState({ showLoginPanel: false, loginError: '', loginButtonPressed: false, loginAccountName: '', loginPassword: '' });

      if (result.needs_hero || result.newPlayer) {
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

  async handleForgotPassword(event?) {
    if (event) event.preventDefault();
    const identifier = (this.state.loginAccountName || '').trim();
    if (!identifier) {
      this.setState({ loginError: 'Enter your account name or email first', resetInfo: '' });
      return;
    }
    try {
      await fetch(`${window.location.origin}/request-password-reset`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ identifier }),
      });
    } catch (error) {
      console.error('Error requesting password reset:', error);
    }
    // Always show the same neutral confirmation (never reveal which accounts exist).
    this.setState({
      loginError: '',
      resetInfo: 'If an account with a recovery email exists, a reset link has been sent.',
    });
  }

  handleResetPasswordChange(event) {
    this.setState({ resetPassword: event.target.value, resetInfo: '' });
  }

  handleResetConfirmChange(event) {
    this.setState({ resetConfirmPassword: event.target.value, resetInfo: '' });
  }

  clearResetParam() {
    try {
      window.history.replaceState({}, document.title, window.location.pathname);
    } catch (e) {
      // ignore history errors
    }
  }

  async handleResetSubmit(event?) {
    if (event) event.preventDefault();
    const { resetToken, resetPassword, resetConfirmPassword } = this.state;

    if (resetPassword.length < 8) {
      this.setState({ resetInfo: 'Password must be at least 8 characters' });
      return;
    }
    if (resetPassword !== resetConfirmPassword) {
      this.setState({ resetInfo: 'Passwords do not match' });
      return;
    }

    this.setState({ resetButtonPressed: true });
    try {
      const response = await fetch(`${window.location.origin}/reset-password`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ token: resetToken, password: resetPassword }),
      });

      if (!response.ok) {
        const result = await response.json().catch(() => ({}));
        this.setState({
          resetInfo: result.msg || result.error || 'This reset link is invalid or has expired',
          resetButtonPressed: false,
        });
        return;
      }

      this.clearResetParam();
      this.setState({
        showResetPanel: false,
        showLoginPanel: true,
        hideLandingPage: true,
        resetToken: '',
        resetPassword: '',
        resetConfirmPassword: '',
        resetButtonPressed: false,
        loginError: '',
        resetInfo: 'Your password has been updated. Please log in.',
      });
    } catch (error) {
      console.error('Error resetting password:', error);
      this.setState({ resetInfo: 'Network error. Please try again.', resetButtonPressed: false });
    }
  }

  handleThreatState(message) {
    if (!shouldShowAccountSetupPrompt(
      message?.day,
      Global.accountSetupCompleted,
      Global.heroDead,
      this.accountSetupPrompted,
    )) {
      return;
    }

    this.accountSetupPrompted = true;
    this.setState({ hideAccountSetupPanel: false, accountSetupError: '' });
  }

  async handleAccountSetupSubmit(data) {
    const { accountName, password, email } = data;
    if (this.state.accountSetupSubmitting) {
      return;
    }
    this.setState({ accountSetupSubmitting: true, accountSetupError: '' });
    try {
      const url = `${window.location.origin}/register`;
      const response = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ account_name: accountName, password, email }),
      });

      if (!response.ok) {
        const result = await response.json().catch(() => ({}));
        this.setState({
          accountSetupError: result.error || 'Failed to save account. Please try again.',
          accountSetupSubmitting: false,
        });
        return;
      }

      const result = await response.json();
      if (result.account_name) {
        Global.accountName = result.account_name;
      }
      Global.accountSetupCompleted = true;
      this.setState({
        hideAccountSetupPanel: true,
        accountSetupError: '',
        accountSetupSubmitting: false,
      });
    } catch (error) {
      console.error('Error during account setup:', error);
      this.setState({
        accountSetupError: 'Network error. Please try again.',
        accountSetupSubmitting: false,
      });
    }
  }

  handleAccountSetupSkip() {
    this.setState({ hideAccountSetupPanel: true, accountSetupError: '' });
  }

  handleHeroDead() {
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
          const totalScoreA = typeof a.total_score === 'number' ? a.total_score : Number(a.total_score) || 0;
          const totalScoreB = typeof b.total_score === 'number' ? b.total_score : Number(b.total_score) || 0;

          if (totalScoreB !== totalScoreA) {
            return totalScoreB - totalScoreA;
          }

          const heroNameA = typeof a.hero_name === 'string' ? a.hero_name : '';
          const heroNameB = typeof b.hero_name === 'string' ? b.hero_name : '';
          return heroNameA.localeCompare(heroNameB);
        })
        .map(entry => ({
          id: entry.id,
          heroName: entry.hero_name,
          heroRank: entry.hero_rank,
          totalScore: typeof entry.total_score === 'number' ? entry.total_score.toLocaleString() : entry.total_score || entry.total_xp,
          totalXp: typeof entry.total_xp === 'number' ? entry.total_xp.toLocaleString() : entry.total_xp,
          daysSurvived: entry.days_survived || 0,
          legendaryKills: entry.legendary_kills || 0,
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
      this.setState({
        inappropiateName: true,
        hideSelectClass: false,
        hideIntro: true,
      });
    }

    if (data.errmsg == 'Hero name is already taken') {
      this.setState({
        takenName: true,
        hideSelectClass: false,
        hideIntro: true,
      });
    }
  }

  async handleErrorOkClick() {
    if (Global.serverOffline) {
      console.log('ErrorOkClick: server offline');
      this.clearSafeLogoutSuppression();
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

    if (this.state.preConnectionSelect) {
      Global.pendingClassSelection = {
        className: this.state.selectedClass,
        heroName: this.state.heroName,
        portrait: this.state.selectedPortrait,
      };
      Global.network = new Network();
      Global.network.connect();
      Global.connected = true;
      this.setState({ hideSelectClass: true, preConnectionSelect: false });
      return;
    }

    Global.network.sendSelectedClass(
      this.state.selectedClass,
      this.state.heroName,
      this.state.selectedPortrait,
    );
  }

  handleHeroNameChange(event) {
    if (this.state.isHeroNameEmpty) {
      this.setState({ isHeroNameEmpty: false });
    }

    if (this.state.inappropiateName) {
      this.setState({ inappropiateName: false });
    }

    if (this.state.takenName) {
      this.setState({ takenName: false });
    }

    this.setState({ heroName: event.target.value });
  }

  handleSelectClass() {
    this.clearSafeLogoutSuppression();
    this.setState({
      hideLandingPage: true,
      hideSelectClass: false,
      hideTrueDeathPanel: true,
      hideGame: true,
      selectedClass: '',
      selectedPortrait: DEFAULT_HERO_PORTRAIT,
      isClassMissing: false,
      safeLogoutCompletionMessage: '',
    });
  }

  handleFirstLogin() {
    this.accountSetupPrompted = false;
    this.clearSafeLogoutSuppression();
    this.setState({
      hideLandingPage: true,
      hideSelectClass: true,
      hideIntro: true,
      hideTrueDeathPanel: true,
      hideGame: false,
      safeLogoutCompletionMessage: '',
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

  }

  handleLoggedIn(data?) {
    this.accountSetupPrompted = false;
    this.setState({
      hideLandingPage: true,
      hideSelectClass: true,
      hideIntro: true,
      hideTrueDeathPanel: true,
      hideGame: false,
      safeLogoutCompletionMessage: '',
    }, () => this.flushSafeLogoutResumeMessage());
    if (data && data.has_account) {
      Global.accountSetupCompleted = true;
    }
  }

  handleInfoTrueDeath(message) {
    this.clearSafeLogoutSuppression();
    this.setState({
      hideLandingPage: true,
      hideSelectClass: true,
      hideGame: true,
      hideTrueDeathPanel: false,
      trueDeathData: message,
      safeLogoutCompletionMessage: '',
    });
  }

  handleClassSelect(className: string) {
    this.setState({ selectedClass: className, isClassMissing: false });
  }

  handlePortraitSelect(portrait: string) {
    this.setState({ selectedPortrait: portrait });
  }

  handleCreateHero() {
    if (this.state.heroName == '') {
      this.setState({ isHeroNameEmpty: true });
    } else if (this.state.selectedClass == '') {
      this.setState({ isClassMissing: true });
    } else {
      this.setState({ hideSelectClass: true, hideIntro: false });
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

              {this.state.safeLogoutCompletionMessage && (
                <p
                  role="status"
                  aria-live="polite"
                  style={{
                    color: '#9fcf95',
                    fontSize: '13px',
                    lineHeight: 1.4,
                    margin: '1em auto 0',
                    maxWidth: '300px',
                    textAlign: 'center',
                  }}
                >
                  {this.state.safeLogoutCompletionMessage}
                </p>
              )}

              <p className="leaderboard-link">
                <button type="button" className="leaderboard-button" onClick={this.handleLeaderboardOpen}>View Leaderboard</button>
              </p>

              <p className="existing-account-link">
                <button type="button" className="leaderboard-button" onClick={this.handleShowLogin}>
                  Log In to Existing Account
                </button>
              </p>
            </div>
          </div>
        )
        }

        {!this.state.hideSelectClass && (
          <HeroCreationPanel
            heroName={this.state.heroName}
            selectedClass={this.state.selectedClass}
            selectedPortrait={this.state.selectedPortrait}
            classImageSize={128}
            isHeroNameEmpty={this.state.isHeroNameEmpty}
            isClassMissing={this.state.isClassMissing}
            inappropriateName={this.state.inappropiateName}
            takenName={this.state.takenName}
            onHeroNameChange={this.handleHeroNameChange}
            onClassSelect={this.handleClassSelect.bind(this)}
            onPortraitSelect={this.handlePortraitSelect}
            onCreate={this.handleCreateHero}
            onShowLogin={this.handleShowLogin}
          />
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
                    autoFocus
                    autoComplete="username" />
                </p>
                <p><span className="fontawesome-lock"></span>
                  <input type="password"
                    value={this.state.loginPassword}
                    onChange={this.handleLoginPasswordChange}
                    placeholder="Password"
                    autoComplete="current-password" />
                </p>
                {this.state.loginError && (
                  <p style={{ color: '#ea4c4c', fontSize: '12px', textAlign: 'center' }}>{this.state.loginError}</p>
                )}
                {this.state.resetInfo && (
                  <p style={{ color: '#c9aa71', fontSize: '12px', textAlign: 'center' }}>{this.state.resetInfo}</p>
                )}
                <p><input
                  type="submit"
                  value={this.state.loginButtonPressed ? 'Logging in...' : 'Log In'}
                  className={`form-button${this.state.loginButtonPressed ? ' form-button--pressed' : ''}`}
                  disabled={this.state.loginButtonPressed}
                  aria-busy={this.state.loginButtonPressed}
                /></p>
              </form>
              <p><a href="#" onClick={this.handleForgotPassword}>Forgot password?</a></p>
              <p><a href="#" onClick={this.handleLoginCancel}>Back</a></p>
            </div>
          </div>
        )}

        {this.state.showResetPanel && (
          <div className="container">
            <img src={logo} style={logoStyle} />
            <div id="login">
              <form onSubmit={this.handleResetSubmit}>
                <p style={{ textAlign: 'center' }}>Choose a new password</p>
                <p><span className="fontawesome-lock"></span>
                  <input type="password"
                    value={this.state.resetPassword}
                    onChange={this.handleResetPasswordChange}
                    placeholder="New Password"
                    autoFocus
                    autoComplete="new-password" />
                </p>
                <p><span className="fontawesome-lock"></span>
                  <input type="password"
                    value={this.state.resetConfirmPassword}
                    onChange={this.handleResetConfirmChange}
                    placeholder="Confirm Password"
                    autoComplete="new-password" />
                </p>
                {this.state.resetInfo && (
                  <p style={{ color: '#ea4c4c', fontSize: '12px', textAlign: 'center' }}>{this.state.resetInfo}</p>
                )}
                <p><input
                  type="submit"
                  value={this.state.resetButtonPressed ? 'Saving...' : 'Set New Password'}
                  className={`form-button${this.state.resetButtonPressed ? ' form-button--pressed' : ''}`}
                  disabled={this.state.resetButtonPressed}
                  aria-busy={this.state.resetButtonPressed}
                /></p>
              </form>
            </div>
          </div>
        )}

        {!this.state.hideGame && (
          <div id="gameContainer" className="gameContainer">
            <div className={isDesktop() ? appStyles.uiContainerDesktop : ''}>
              <UI />
            </div>
            <Game />
            {isDesktop() && typeof window !== 'undefined' && window.innerHeight > 1150 && (
              <img
                src={logo}
                style={{
                  position: 'fixed',
                  top: '8px',
                  left: '50%',
                  transform: 'translateX(-50%)',
                  height: 'calc((100vh - 1000px) / 2 - 16px)',
                  width: 'auto',
                  maxWidth: '1200px',
                  pointerEvents: 'none',
                  zIndex: 2,
                }}
              />
            )}
          </div>
        )
        }

        {!this.state.hideAccountSetupPanel && !this.state.hideGame && (
          <AccountSetupPanel
            errorMessage={this.state.accountSetupError}
            submitting={this.state.accountSetupSubmitting} />
        )}

        {!this.state.hideTrueDeathPanel &&
          <TrueDeathPanel
            heroName={this.state.trueDeathData.hero_name}
            heroRank={this.state.trueDeathData.hero_rank}
            totalXp={this.state.trueDeathData.total_xp}
            scoreTotal={this.state.trueDeathData.score_total}
            scoreBreakdown={this.state.trueDeathData.score_breakdown}
            daysSurvived={this.state.trueDeathData.days_survived}
            wavesSurvived={this.state.trueDeathData.waves_survived}
            highestPressureLevel={this.state.trueDeathData.highest_pressure_level}
            legendaryKills={this.state.trueDeathData.legendary_kills}
            hideoutsCleared={this.state.trueDeathData.hideouts_cleared}
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
                      <th>Score</th>
                      <th>Days</th>
                      <th>Legends</th>
                      <th>Fate</th>
                    </tr>
                  </thead>
                  <tbody>
                    {paginatedEntries.map(entry => (
                      <tr key={entry.id}>
                        <td>{entry.heroName}</td>
                        <td className="rank">{entry.heroRank}</td>
                        <td>{entry.totalScore}</td>
                        <td>{entry.daysSurvived}</td>
                        <td>{entry.legendaryKills}</td>
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
