import * as React from "react";
import okbutton from "ui_comp/okbutton.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import {
  INTRO_SLIDES,
  INTRO_SLIDE_FADE_MS,
  INTRO_SLIDE_HOLD_MS,
} from "../../core/introSlides";
import { MOBILE_DIALOG_Z } from "./mobileLayers";

interface IntroProps {
}

interface IntroState {
  currentPanel: number;
  slideVisible: boolean;
}

export default class IntroPanel extends React.Component<IntroProps, IntroState> {
  private displayTimer?: number;
  private fadeTimer?: number;
  private preloadedImages: HTMLImageElement[] = [];

  constructor(props) {
    super(props);

    this.state = {
      currentPanel: 0,
      slideVisible: false,
    };

    this.handleOkClick = this.handleOkClick.bind(this);
  }

  componentDidMount() {
    this.preloadSlides();

    this.fadeTimer = window.setTimeout(() => {
      this.fadeTimer = undefined;
      this.setState({ slideVisible: true }, this.scheduleAutomaticAdvance);
    }, 50);
  }

  componentWillUnmount() {
    this.clearSlideshowTimers();
    this.preloadedImages = [];
  }

  preloadSlides = () => {
    this.preloadedImages = INTRO_SLIDES.slice(1).map((slide) => {
      const image = new Image();
      image.src = slide.image;
      return image;
    });
  };

  clearSlideshowTimers = () => {
    if (this.displayTimer !== undefined) {
      window.clearTimeout(this.displayTimer);
      this.displayTimer = undefined;
    }

    if (this.fadeTimer !== undefined) {
      window.clearTimeout(this.fadeTimer);
      this.fadeTimer = undefined;
    }
  };

  scheduleAutomaticAdvance = () => {
    if (this.state.currentPanel >= INTRO_SLIDES.length - 1) {
      return;
    }

    this.displayTimer = window.setTimeout(
      this.fadeToNextSlide,
      INTRO_SLIDE_FADE_MS + INTRO_SLIDE_HOLD_MS,
    );
  };

  fadeToNextSlide = () => {
    if (!this.state.slideVisible || this.state.currentPanel >= INTRO_SLIDES.length - 1) {
      return;
    }

    this.clearSlideshowTimers();
    this.setState({ slideVisible: false });
    this.fadeTimer = window.setTimeout(() => {
      this.fadeTimer = undefined;
      this.setState((state) => ({
        currentPanel: Math.min(state.currentPanel + 1, INTRO_SLIDES.length - 1),
        slideVisible: true,
      }), this.scheduleAutomaticAdvance);
    }, INTRO_SLIDE_FADE_MS);
  };

  handleOkClick() {
    if (!this.state.slideVisible) {
      return;
    }

    if (this.state.currentPanel < INTRO_SLIDES.length - 1) {
      this.fadeToNextSlide();
    } else {
      this.clearSlideshowTimers();
      Global.gameEmitter.emit(GameEvent.INTRO_OK_CLICK, {});
    }
  }

  render() {
    const introSlide = INTRO_SLIDES[this.state.currentPanel];

    const overlayStyle: React.CSSProperties = {
      position: 'fixed',
      top: 0, left: 0, right: 0, bottom: 0,
      background: 'rgba(0, 0, 0, 0.85)',
      zIndex: MOBILE_DIALOG_Z,
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
      alignItems: 'center',
      gap: '12px',
    };

    const introImageStyle: React.CSSProperties = {
      width: '100%',
      maxWidth: '360px',
      height: 'auto',
      transform: 'translateY(8px)',
      opacity: this.state.slideVisible ? 1 : 0,
      transition: `opacity ${INTRO_SLIDE_FADE_MS}ms ease-in-out`,
      willChange: 'opacity',
    };

    const indicatorsStyle: React.CSSProperties = {
      color: '#c8a56b',
      fontSize: '11px',
      letterSpacing: '5px',
      lineHeight: 1,
    };

    const textStyle: React.CSSProperties = {
      textAlign: 'center',
      transform: 'translateY(8px)',
      color: '#efe4cf',
      fontFamily: "'IM Fell English', Alegreya, Georgia, serif",
      fontSize: '17px',
      lineHeight: 1.4,
      letterSpacing: '0.15px',
      whiteSpace: 'pre-wrap',
      margin: 0,
      opacity: this.state.slideVisible ? 1 : 0,
      transition: `opacity ${INTRO_SLIDE_FADE_MS}ms ease-in-out`,
      willChange: 'opacity',
    };

    const submitStyle: React.CSSProperties = {
      cursor: 'pointer',
    };

    return (
      <div style={overlayStyle}>
        <div style={cardStyle}>
          <img src={introSlide.image} style={introImageStyle} alt={introSlide.alt} />
          <div style={indicatorsStyle} aria-label={`Slide ${this.state.currentPanel + 1} of ${INTRO_SLIDES.length}`}>
            {INTRO_SLIDES.map((_slide, index) => index === this.state.currentPanel ? "●" : "○")}
          </div>
          <p style={textStyle}>{introSlide.text}</p>
          <img
            src={okbutton}
            style={submitStyle}
            onClick={this.handleOkClick}
            title={this.state.currentPanel === INTRO_SLIDES.length - 1 ? "Begin" : "Continue"}
            alt={this.state.currentPanel === INTRO_SLIDES.length - 1 ? "Begin" : "Continue"} />
        </div>
      </div>
    );
  }
}
