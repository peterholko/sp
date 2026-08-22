import * as React from "react";
import { Global } from "../../core/global";
import { NetworkEvent } from "../../core/networkEvent";
import { getNeedStatusIcon, isCriticalNeed, NeedKind } from "./needStatus";
import { characterImageUrl } from "../../core/portraitCatalog";
import styles from "./../ui.module.css";

const SANCTUARY_EFFECT = "Sanctuary";
const SANCTUARY_COLOR = "#3fb84f";

const CRITICAL_NEED_WARNING_STYLE = `
@keyframes criticalNeedIconPulse {
  0%, 100% { opacity: 1; transform: scale(1); filter: brightness(1); }
  50% { opacity: .72; transform: scale(1.14); filter: brightness(1.35) drop-shadow(0 0 5px rgba(255,70,48,.95)); }
}
.critical-need-warning-icon { animation: criticalNeedIconPulse .85s ease-in-out infinite; transform-origin: 50% 50%; }
@media (prefers-reduced-motion: reduce) { .critical-need-warning-icon { animation: none !important; } }
`;

interface HeroFrameProps {
  heroStats: any,
  hungerStatus: string,
  thirstStatus: string,
  fatigueStatus: string,
  worldData: any,
}

function ratio(value: unknown, maximum: unknown): number {
  const current = Number(value);
  const max = Number(maximum);
  if (!Number.isFinite(current) || !Number.isFinite(max) || max <= 0) return 0;
  return Math.max(0, Math.min(1, current / max));
}

function renderNeedStatusIcon(kind: NeedKind, value: string) {
  const icon = getNeedStatusIcon(kind, value);
  if (!icon) return null;

  return (
    <img
      className={`${styles.heroNeedIcon} ${isCriticalNeed(kind, value) ? 'critical-need-warning-icon' : ''}`}
      src={icon}
      title={`${kind}: ${value || 'unknown'}`}
      alt={`${kind}: ${value || 'unknown'}`}
    />
  );
}

export default class HeroFrame extends React.Component<HeroFrameProps, any> {
  constructor(props) {
    super(props);
    this.state = { hideHero: true, sanctuary: false };
  }

  componentDidMount() {
    Global.gameEmitter.on(NetworkEvent.PERCEPTION, this.handlePerception, this);
    Global.gameEmitter.on(NetworkEvent.GAINED_EFFECT, this.handleGainedEffect, this);
    Global.gameEmitter.on(NetworkEvent.LOST_EFFECT, this.handleLostEffect, this);
    Global.gameEmitter.on(NetworkEvent.HERO_DEATH_STATE, this.handleSanctuaryCleared, this);
    Global.gameEmitter.on(NetworkEvent.INFO_TRUE_DEATH, this.handleSanctuaryCleared, this);
  }

  componentWillUnmount() {
    Global.gameEmitter.off(NetworkEvent.PERCEPTION, this.handlePerception, this);
    Global.gameEmitter.off(NetworkEvent.GAINED_EFFECT, this.handleGainedEffect, this);
    Global.gameEmitter.off(NetworkEvent.LOST_EFFECT, this.handleLostEffect, this);
    Global.gameEmitter.off(NetworkEvent.HERO_DEATH_STATE, this.handleSanctuaryCleared, this);
    Global.gameEmitter.off(NetworkEvent.INFO_TRUE_DEATH, this.handleSanctuaryCleared, this);
  }

  handlePerception() {
    this.setState({ hideHero: false });
  }

  handleGainedEffect(message) {
    if (message.id == Global.heroId && message.effect == SANCTUARY_EFFECT) {
      this.setState({ sanctuary: true });
    }
  }

  handleLostEffect(message) {
    if (message.id == Global.heroId && message.effect == SANCTUARY_EFFECT) {
      this.setState({ sanctuary: false });
    }
  }

  handleSanctuaryCleared() {
    this.setState({ sanctuary: false });
  }

  renderBar(label: string, value: unknown, maximum: unknown, className: string) {
    const pct = ratio(value, maximum) * 100;
    return (
      <div className={styles.heroStatRow} title={`${label}: ${Number(value) || 0} / ${Number(maximum) || 0}`}>
        <span>{label}</span>
        <div className={styles.heroStatRail}>
          <div className={`${styles.heroStatFill} ${className}`} style={{ width: `${pct}%` }} />
        </div>
      </div>
    );
  }

  render() {
    const stats = this.props.heroStats || {};
    const heroState = Global.objectStates[Global.heroId];
    const imagePath = heroState ? characterImageUrl(heroState.portrait, heroState.image) : '';
    const maxMana = stats.base_mana || Global.heroMaxMana || 0;
    const mana = stats.mana !== undefined ? stats.mana : Global.heroMana;
    const showMana = Number(maxMana) > 0;
    const world = this.props.worldData || {};

    return (
      <header className={styles.heroHud} aria-label="Hero status">
        <style>{CRITICAL_NEED_WARNING_STYLE}</style>
        <div className={styles.heroPortraitWrap}>
          {!this.state.hideHero && imagePath &&
            <img src={imagePath} className={styles.heroHudPortrait} alt="Hero portrait" />}
        </div>

        <div className={styles.heroStatusCenter}>
          {this.renderBar('HP', stats.hp, Global.heroMaxHp, styles.heroHpFill)}
          {this.renderBar('STA', stats.stamina, Global.heroMaxStamina, styles.heroStaminaFill)}
          {showMana && this.renderBar('MP', mana, maxMana, styles.heroManaFill)}
          <div className={styles.heroNeeds} aria-label="Hero needs">
            {renderNeedStatusIcon('thirst', this.props.thirstStatus)}
            {renderNeedStatusIcon('hunger', this.props.hungerStatus)}
            {renderNeedStatusIcon('tiredness', this.props.fatigueStatus)}
            {this.state.sanctuary &&
              <svg className={styles.heroSanctuary} viewBox="0 0 24 28" role="img" aria-label="Sanctuary">
                <title>Sanctuary</title>
                <path d="M12 1 L22 4.5 V13 C22 20 17.5 25 12 27 C6.5 25 2 20 2 13 V4.5 Z"
                  fill={SANCTUARY_COLOR} stroke="#0c0e10" strokeWidth="1.6" strokeLinejoin="round" />
              </svg>}
          </div>
        </div>

        <div className={styles.heroWorldStatus} aria-label="World time">
          <strong>Day {world.day ?? '—'}</strong>
          <span>{world.time_of_day || '—'}</span>
        </div>
      </header>
    );
  }
}
