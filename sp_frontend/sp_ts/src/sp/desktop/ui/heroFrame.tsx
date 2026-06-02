import * as React from "react";
import { Global } from "../../core/global";
import heroring from "ui_comp/heroring.png";
import hpframe from "ui_comp/hpframe.png";
import statbg from "ui_comp/statbg.png";
import hpbar from "ui_comp/hpbar.png";
import stabar from "ui_comp/stabar.png";
import manabar from "ui_comp/manabar.png";
import { NetworkEvent } from "../../core/networkEvent";
import { STAT_BAR_WIDTH, STAT_BAR_HEIGHT } from "../../core/config";
import { getNeedStatusIcon, isCriticalNeed, NeedKind } from "./needStatus";

const NEED_STATUS_SIZE = 30;

// Effect names as sent by the server (see sp_server effect.rs SANCTUARY / WEAK_SANCTUARY).
const SANCTUARY_EFFECT = "Sanctuary";
const WEAK_SANCTUARY_EFFECT = "Weak Sanctuary";

// Sanctuary strength shown beside the HP/Stamina panel: green when strong, yellow when weak.
type SanctuaryState = "strong" | "weak" | null;
const SANCTUARY_STRONG_COLOR = "#3fb84f";
const SANCTUARY_WEAK_COLOR = "#e2b007";

const CRITICAL_NEED_WARNING_STYLE = `
@keyframes criticalNeedIconPulse {
  0%, 100% {
    opacity: 1;
    transform: scale(1);
    filter: brightness(1) drop-shadow(0 0 1px rgba(255, 70, 48, 0.65));
  }
  50% {
    opacity: 0.72;
    transform: scale(1.16);
    filter: brightness(1.35) drop-shadow(0 0 6px rgba(255, 70, 48, 0.95));
  }
}

.critical-need-warning-icon {
  animation: criticalNeedIconPulse 0.85s ease-in-out infinite;
  transform-origin: 50% 50%;
}

@media (prefers-reduced-motion: reduce) {
  .critical-need-warning-icon {
    animation: none !important;
    filter: brightness(1.2) drop-shadow(0 0 4px rgba(255, 70, 48, 0.85));
  }
}
`;

interface HeroFrameProps {
  heroStats: any,
  hungerStatus: string,
  thirstStatus: string,
  fatigueStatus: string
}

function renderNeedStatusIcon(kind: NeedKind, value: string, style: React.CSSProperties, label: string) {
  const icon = getNeedStatusIcon(kind, value);

  if (!icon) {
    return null;
  }

  const tooltip = value ? `${label}: ${value}` : label;

  if (!isCriticalNeed(kind, value)) {
    return <img src={icon} style={style} title={tooltip} alt={tooltip} aria-label={tooltip}/>;
  }

  const containerStyle = {
    ...style,
    width: NEED_STATUS_SIZE + 'px',
    height: NEED_STATUS_SIZE + 'px',
    display: 'block',
    pointerEvents: 'none',
  } as React.CSSProperties;

  const iconStyle = {
    position: 'absolute',
    left: 0,
    top: 0,
    width: NEED_STATUS_SIZE + 'px',
    height: NEED_STATUS_SIZE + 'px',
  } as React.CSSProperties;

  return (
    <span style={containerStyle} title={tooltip} aria-label={tooltip}>
      <img className="critical-need-warning-icon" src={icon} style={iconStyle} alt={tooltip}/>
    </span>
  );
}

export default class HeroFrame extends React.Component<HeroFrameProps, any> {
  constructor(props) {
    super(props);

    this.state = {
      hideHero : true,
      sanctuary : null as SanctuaryState,
    };
  }

  componentDidMount() {
    Global.gameEmitter.on(NetworkEvent.PERCEPTION, this.handlePerception, this);
    Global.gameEmitter.on(NetworkEvent.GAINED_EFFECT, this.handleGainedEffect, this);
    Global.gameEmitter.on(NetworkEvent.LOST_EFFECT, this.handleLostEffect, this);
    Global.gameEmitter.on(NetworkEvent.INCREASED_EFFECT, this.handleIncreasedEffect, this);
    Global.gameEmitter.on(NetworkEvent.REDUCED_EFFECT, this.handleReducedEffect, this);
  }

  componentWillUnmount() {
    // avoid leaks / duplicate handlers
    Global.gameEmitter.off(NetworkEvent.PERCEPTION, this.handlePerception, this);
    Global.gameEmitter.off(NetworkEvent.GAINED_EFFECT, this.handleGainedEffect, this);
    Global.gameEmitter.off(NetworkEvent.LOST_EFFECT, this.handleLostEffect, this);
    Global.gameEmitter.off(NetworkEvent.INCREASED_EFFECT, this.handleIncreasedEffect, this);
    Global.gameEmitter.off(NetworkEvent.REDUCED_EFFECT, this.handleReducedEffect, this);
  }

  handlePerception() {
    this.setState({hideHero: false});
  }

  // Track the hero's Sanctuary strength from effect-change packets. The server only
  // sends these for the hero (villagers are skipped) as it crosses monolith ranges:
  // gained -> entered, increased -> weak became strong, reduced -> strong became weak,
  // lost -> left entirely.
  handleGainedEffect(message) {
    if (message.id != Global.heroId) return;
    if (message.effect == SANCTUARY_EFFECT) {
      this.setState({ sanctuary: "strong" });
    } else if (message.effect == WEAK_SANCTUARY_EFFECT) {
      this.setState({ sanctuary: "weak" });
    }
  }

  handleLostEffect(message) {
    if (message.id != Global.heroId) return;
    if (message.effect == SANCTUARY_EFFECT || message.effect == WEAK_SANCTUARY_EFFECT) {
      this.setState({ sanctuary: null });
    }
  }

  handleIncreasedEffect(message) {
    if (message.id != Global.heroId) return;
    if (message.effect == SANCTUARY_EFFECT) {
      this.setState({ sanctuary: "strong" });
    }
  }

  handleReducedEffect(message) {
    if (message.id != Global.heroId) return;
    if (message.effect == SANCTUARY_EFFECT) {
      this.setState({ sanctuary: "weak" });
    }
  }

  render() {
    let imagePath = '';

    const hpRatio = Global.heroMaxHp > 0 ? this.props.heroStats.hp / Global.heroMaxHp : 0;
    const hpBarWidth = hpRatio * STAT_BAR_WIDTH;

    const staRatio = Global.heroMaxStamina > 0 ? this.props.heroStats.stamina / Global.heroMaxStamina : 0;
    const staBarWidth = staRatio * STAT_BAR_WIDTH;

    const baseMana = this.props.heroStats.base_mana || Global.heroMaxMana || 0;
    const mana = this.props.heroStats.mana !== undefined ? this.props.heroStats.mana : Global.heroMana;
    const showMana = baseMana > 0;
    const manaRatio = showMana ? mana / baseMana : 0;
    const manaBarWidth = manaRatio * STAT_BAR_WIDTH;

    const hpTitle = (this.props.heroStats.hp != null && Global.heroMaxHp > 0)
      ? `Health: ${Math.round(this.props.heroStats.hp)} / ${Global.heroMaxHp}`
      : 'Health';
    const staTitle = (this.props.heroStats.stamina != null && Global.heroMaxStamina > 0)
      ? `Stamina: ${Math.round(this.props.heroStats.stamina)} / ${Global.heroMaxStamina}`
      : 'Stamina';
    const manaTitle = (mana != null && baseMana > 0)
      ? `Mana: ${Math.round(mana)} / ${baseMana}`
      : 'Mana';

    let heroName = '';
    if(Global.heroId in Global.objectStates) {
      let imageName = Global.objectStates[Global.heroId].image.toLowerCase().replace(/\s/g, '');
      imagePath = '/static/art/' + imageName  + '_single.png';
      heroName = Global.objectStates[Global.heroId].name || '';
    }

    const heroringStyle = {
      transform: 'translate(8px, 21px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const hpframeStyle = {
      transform: 'translate(41px, 10px)',
      zIndex: 2,
      position: 'fixed'
    } as React.CSSProperties

    const hpbgStyle = {
      transform: 'translate(95px, 17px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const stabgStyle = {
      transform: 'translate(95px, 35px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const manabgStyle = {
      transform: 'translate(95px, 53px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const hpBarStyle  = {
      transform: 'translate(97px, 19px)',
      width: hpBarWidth + 'px',
      height: STAT_BAR_HEIGHT + 'px',
      zIndex: 4,
      position: 'fixed' 
    } as React.CSSProperties
 
    const staBarStyle  = {
      transform: 'translate(97px, 37px)',
      width: staBarWidth + 'px',
      height: STAT_BAR_HEIGHT + 'px',
      zIndex: 4,
      position: 'fixed' 
    } as React.CSSProperties
  
    const manaBarStyle  = {
      transform: 'translate(97px, 55px)',
      width: manaBarWidth + 'px',
      height: STAT_BAR_HEIGHT + 'px',
      zIndex: 4,
      position: 'fixed' 
    } as React.CSSProperties
 
    const heroStyle = {
      transform: 'translate(13px, 24px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const thirstStatusStyle = {
      transform: 'translate(100px, 75px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties  

    const hungerStatusStyle = {
      transform: 'translate(150px, 75px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const fatigueStatusStyle = {
      transform: 'translate(200px, 75px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const tStyle = {
      transform: 'translate(90px, 79px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const hStyle = {
      transform: 'translate(140px, 79px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const fStyle = {
      transform: 'translate(190px, 79px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    // Sanctuary indicator sits just right of the HP/Stamina panel (hpframe ends ~x229).
    // pointerEvents must stay 'auto' so the hover tooltip below can appear.
    const sanctuaryStyle = {
      transform: 'translate(238px, 21px)',
      zIndex: 4,
      position: 'fixed',
      pointerEvents: 'auto',
      cursor: 'help',
      filter: 'drop-shadow(0 0 2px rgba(0, 0, 0, 0.85))'
    } as React.CSSProperties

    const sanctuary: SanctuaryState = this.state.sanctuary;
    const sanctuaryColor = sanctuary === "strong" ? SANCTUARY_STRONG_COLOR : SANCTUARY_WEAK_COLOR;
    const sanctuaryLabel = sanctuary === "strong"
      ? "Sanctuary (Strong) — the Monolith's protection greatly reduces the damage you take. Stay near the Monolith to keep it."
      : "Sanctuary (Weak) — the Monolith's protection slightly reduces the damage you take. Move closer to the Monolith to strengthen it.";

    return (
      
      <div>
          <style>{CRITICAL_NEED_WARNING_STYLE}</style>
          <img src={heroring} style={heroringStyle}/>
          <img src={hpframe} style={hpframeStyle}/>

          <img src={statbg} style={hpbgStyle} title={hpTitle}/>
          <img src={hpbar} style={hpBarStyle} title={hpTitle} alt={hpTitle} aria-label={hpTitle}/>
          <img src={statbg} style={stabgStyle} title={staTitle}/>
          <img src={stabar} style={staBarStyle} title={staTitle} alt={staTitle} aria-label={staTitle}/>
          {showMana &&
            <>
              <img src={statbg} style={manabgStyle} title={manaTitle}/>
              <img src={manabar} style={manaBarStyle} title={manaTitle} alt={manaTitle} aria-label={manaTitle}/>
            </>
          }

          <span style={tStyle} title="Thirst">T</span>
          <span style={hStyle} title="Hunger">H</span>
          <span style={fStyle} title="Fatigue">F</span>

          {renderNeedStatusIcon("thirst", this.props.thirstStatus, thirstStatusStyle, "Thirst")}
          {renderNeedStatusIcon("hunger", this.props.hungerStatus, hungerStatusStyle, "Hunger")}
          {renderNeedStatusIcon("tiredness", this.props.fatigueStatus, fatigueStatusStyle, "Fatigue")}

          {!this.state.hideHero &&
            <img src={imagePath} style={heroStyle} title={heroName} alt={heroName} aria-label={heroName}/>
          }

          {sanctuary &&
            <svg width="26" height="30" viewBox="0 0 24 28" style={sanctuaryStyle} role="img" aria-label={sanctuaryLabel}>
              <title>{sanctuaryLabel}</title>
              <path
                d="M12 1 L22 4.5 V13 C22 20 17.5 25 12 27 C6.5 25 2 20 2 13 V4.5 Z"
                fill={sanctuaryColor}
                stroke="#0c0e10"
                strokeWidth="1.6"
                strokeLinejoin="round"
              />
            </svg>
          }
      </div>
    );
  }
}
