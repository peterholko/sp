import * as React from "react";
import widepanel from "ui_comp/widepanel.png";
import okbutton from "ui_comp/okbutton.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import {
  INTRO_SLIDES,
  INTRO_SLIDE_FADE_MS,
  INTRO_SLIDE_HOLD_MS,
} from "../../core/introSlides";

interface IntroProps {
}

interface IntroState {
  windowHeight: number;
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
      windowHeight: window.innerHeight,
      currentPanel: 0,
      slideVisible: false,
    };

    this.handleOkClick = this.handleOkClick.bind(this);
    this.updateWindowHeight = this.updateWindowHeight.bind(this);
  }

  componentDidMount() {
    window.addEventListener("resize", this.updateWindowHeight);
    this.preloadSlides();

    this.fadeTimer = window.setTimeout(() => {
      this.fadeTimer = undefined;
      this.setState({ slideVisible: true }, this.scheduleAutomaticAdvance);
    }, 50);
  }

  componentWillUnmount() {
    window.removeEventListener("resize", this.updateWindowHeight);
    this.clearSlideshowTimers();
    this.preloadedImages = [];
  }

  updateWindowHeight() {
    this.setState({ windowHeight: window.innerHeight });
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

    const introStyle = {
      top: "50%",
      left: "50%",
      width: "667px",
      height: "375px",
      marginTop: "-193px",
      marginLeft: "-333px",
      position: "fixed",
      zIndex: 20,
    } as React.CSSProperties;

    const introPanelStyle = {
      position: "absolute",
      inset: 0,
    } as React.CSSProperties;

    const spanNameStyle = {
      position: "absolute",
      top: "249px",
      left: "20px",
      textAlign: "center",
      color: "#efe4cf",
      fontFamily: "'IM Fell English', Alegreya, Georgia, serif",
      fontSize: "16px",
      lineHeight: 1.3,
      letterSpacing: "0.15px",
      width: "620px",
      whiteSpace: "pre-wrap",
      opacity: this.state.slideVisible ? 1 : 0,
      transition: `opacity ${INTRO_SLIDE_FADE_MS}ms ease-in-out`,
      willChange: "opacity",
    } as React.CSSProperties;

    const introImageStyle = {
      position: "absolute",
      top: "21px",
      left: "153px",
      width: "360px",
      height: "203px",
      objectFit: "cover",
      opacity: this.state.slideVisible ? 1 : 0,
      transition: `opacity ${INTRO_SLIDE_FADE_MS}ms ease-in-out`,
      willChange: "opacity",
    } as React.CSSProperties;

    const indicatorsStyle = {
      position: "absolute",
      top: "227px",
      left: 0,
      width: "667px",
      color: "#c8a56b",
      fontSize: "11px",
      letterSpacing: "5px",
      textAlign: "center",
    } as React.CSSProperties;

    let okButtonStyle;

    if (this.state.windowHeight < 400) {
      okButtonStyle = {
        top: "310px",
        left: "425px",
        position: "absolute",
        cursor: "pointer",
      } as React.CSSProperties;
    } else {
      okButtonStyle = {
        top: "350px",
        left: "307px",
        position: "absolute",
        cursor: "pointer",
      } as React.CSSProperties;
    }

    return (
      <div style={introStyle}>
        <img src={widepanel} style={introPanelStyle} />
        <img src={introSlide.image} style={introImageStyle} alt={introSlide.alt} />
        <div style={indicatorsStyle} aria-label={`Slide ${this.state.currentPanel + 1} of ${INTRO_SLIDES.length}`}>
          {INTRO_SLIDES.map((_slide, index) => index === this.state.currentPanel ? "●" : "○")}
        </div>
        <span style={spanNameStyle}>{introSlide.text}</span>
        <img
          src={okbutton}
          style={okButtonStyle}
          onClick={this.handleOkClick}
          title={this.state.currentPanel === INTRO_SLIDES.length - 1 ? "Begin" : "Continue"}
          alt={this.state.currentPanel === INTRO_SLIDES.length - 1 ? "Begin" : "Continue"} />
      </div>
    );
  }
}
