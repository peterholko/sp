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
import { characterImageUrl } from "../../core/portraitCatalog";

const NEED_STATUS_SIZE = 30;

// Single Sanctuary effect name sent by the server.
const SANCTUARY_EFFECT = "Sanctuary";
const SANCTUARY_COLOR = "#3fb84f";

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

function renderNeedStatusIcon(kind: NeedKind, value: string, style: React.CSSProperties) {
  const icon = getNeedStatusIcon(kind, value);

  if (!icon) {
    return null;
  }

  if (!isCriticalNeed(kind, value)) {
    return <img src={icon} style={style}/>;
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
    <span style={containerStyle}>
      <img className="critical-need-warning-icon" src={icon} style={iconStyle}/>
    </span>
  );
}

export default class HeroFrame extends React.Component<HeroFrameProps, any> {
  constructor(props) {
    super(props);

    this.state = {
      hideHero : true,
      sanctuary : false,
    };
  }

  componentDidMount() {
    Global.gameEmitter.on(NetworkEvent.PERCEPTION, this.handlePerception, this);
    Global.gameEmitter.on(NetworkEvent.GAINED_EFFECT, this.handleGainedEffect, this);
    Global.gameEmitter.on(NetworkEvent.LOST_EFFECT, this.handleLostEffect, this);
    Global.gameEmitter.on(NetworkEvent.HERO_DEATH_STATE, this.handleSanctuaryCleared, this);
    Global.gameEmitter.on(NetworkEvent.INFO_TRUE_DEATH, this.handleSanctuaryCleared, this);
  }

  componentWillUnmount() {
    // avoid leaks / duplicate handlers
    Global.gameEmitter.off(NetworkEvent.PERCEPTION, this.handlePerception, this);
    Global.gameEmitter.off(NetworkEvent.GAINED_EFFECT, this.handleGainedEffect, this);
    Global.gameEmitter.off(NetworkEvent.LOST_EFFECT, this.handleLostEffect, this);
    Global.gameEmitter.off(NetworkEvent.HERO_DEATH_STATE, this.handleSanctuaryCleared, this);
    Global.gameEmitter.off(NetworkEvent.INFO_TRUE_DEATH, this.handleSanctuaryCleared, this);
  }

  handlePerception() {
    this.setState({hideHero: false});
  }

  // The server now has one Sanctuary tier: gained means green shield, lost means none.
  handleGainedEffect(message) {
    if (message.id != Global.heroId) return;
    if (message.effect == SANCTUARY_EFFECT) {
      this.setState({ sanctuary: true });
    }
  }

  handleLostEffect(message) {
    if (message.id != Global.heroId) return;
    if (message.effect == SANCTUARY_EFFECT) {
      this.setState({ sanctuary: false });
    }
  }

  handleSanctuaryCleared() {
    this.setState({ sanctuary: false });
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

    if(Global.heroId in Global.objectStates) {
      const heroState = Global.objectStates[Global.heroId];
      imagePath = characterImageUrl(heroState.portrait, heroState.image);
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
      position: 'fixed',
      width: '72px',
      height: '72px',
      borderRadius: '50%',
      objectFit: 'cover'
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
    const sanctuaryStyle = {
      transform: 'translate(238px, 21px)',
      zIndex: 4,
      position: 'fixed',
      pointerEvents: 'none',
      filter: 'drop-shadow(0 0 2px rgba(0, 0, 0, 0.85))'
    } as React.CSSProperties

    const sanctuary = this.state.sanctuary === true;
    const sanctuaryLabel = "Sanctuary";

    return (
      
      <div>
          <style>{CRITICAL_NEED_WARNING_STYLE}</style>
          <img src={heroring} style={heroringStyle}/>
          <img src={hpframe} style={hpframeStyle}/>

          <img src={statbg} style={hpbgStyle}/>
          <img src={hpbar} style={hpBarStyle}/>
          <img src={statbg} style={stabgStyle}/>
          <img src={stabar} style={staBarStyle}/>
          {showMana &&
            <>
              <img src={statbg} style={manabgStyle}/>
              <img src={manabar} style={manaBarStyle}/>
            </>
          }

          <span style={tStyle}>T</span>
          <span style={hStyle}>H</span>
          <span style={fStyle}>F</span>

          {renderNeedStatusIcon("thirst", this.props.thirstStatus, thirstStatusStyle)}
          {renderNeedStatusIcon("hunger", this.props.hungerStatus, hungerStatusStyle)}
          {renderNeedStatusIcon("tiredness", this.props.fatigueStatus, fatigueStatusStyle)}

          {!this.state.hideHero &&
            <img src={imagePath} style={heroStyle}/>
          }

          {sanctuary &&
            <svg width="26" height="30" viewBox="0 0 24 28" style={sanctuaryStyle} role="img" aria-label={sanctuaryLabel}>
              <title>{sanctuaryLabel}</title>
              <path
                d="M12 1 L22 4.5 V13 C22 20 17.5 25 12 27 C6.5 25 2 20 2 13 V4.5 Z"
                fill={SANCTUARY_COLOR}
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
