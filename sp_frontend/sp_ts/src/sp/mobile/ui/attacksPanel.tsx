import * as React from "react";
import { mobileCombatHints } from "../mobileCombatHints";

const MAX_ATTACKS = 6;

interface AttacksProp {
  attacks,
  combatState?: any,
}

export default class AttacksPanel extends React.Component<AttacksProp, any> {
  render() {
    const combatState = this.props.combatState || {};
    const history = (combatState.attack_history || this.props.attacks || []).slice(-MAX_ATTACKS);
    const combos = combatState.matching_combos || [];
    const availableFinisher = combatState.available_finisher;
    const targetEffects = Array.isArray(combatState.target_effects) ? combatState.target_effects : [];
    const counterHint = combatState.counter_hint;
    const enemyIntent = combatState.enemy_intent;
    const compactHints = mobileCombatHints(enemyIntent, counterHint);

    if (!history.length && !combos.length && !availableFinisher
        && !targetEffects.length && !counterHint && !enemyIntent) {
      return null;
    }

    const panelStyle: React.CSSProperties = {
      position: 'fixed',
      left: '50%',
      bottom: 'calc(198px + env(safe-area-inset-bottom, 0px))',
      transform: 'translateX(-50%)',
      zIndex: 14,
      width: 'min(300px, calc(100vw - 16px))',
      boxSizing: 'border-box',
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      gap: '4px',
      padding: '6px 8px',
      border: '1px solid rgba(201, 170, 113, .45)',
      borderRadius: '6px',
      background: 'rgba(10, 12, 15, .92)',
      boxShadow: '0 4px 14px rgba(0,0,0,.48)',
      pointerEvents: 'none',
    };
    const rowStyle: React.CSSProperties = {
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      flexWrap: 'wrap',
      gap: '4px',
      color: '#d8d1c5',
      fontFamily: 'Verdana',
      fontSize: '10px',
      textAlign: 'center',
    };
    const pipStyle: React.CSSProperties = { width: '20px', height: '20px', imageRendering: 'pixelated' };
    const badgeStyle: React.CSSProperties = {
      padding: '2px 6px',
      borderRadius: '9px',
      background: 'rgba(91,18,24,.9)',
      color: '#ffd6d6',
      fontWeight: 700,
    };
    const hintRowStyle: React.CSSProperties = {
      ...rowStyle,
      alignItems: 'stretch',
      flexDirection: 'column',
      gap: '1px',
      lineHeight: 1.15,
      width: '100%',
    };
    const intentStyle: React.CSSProperties = {
      color: '#e6d7bd',
      fontWeight: 700,
    };
    const counterStyle: React.CSSProperties = {
      color: '#f2d27a',
    };

    return (
      <aside style={panelStyle} aria-label="Combat information">
        {(compactHints.intent || compactHints.counter) &&
          <div style={hintRowStyle} aria-label="Enemy combat hints">
            {compactHints.intent &&
              <span style={intentStyle} title={enemyIntent}>⚔ {compactHints.intent}</span>}
            {compactHints.counter &&
              <span style={counterStyle} title={counterHint}>Try: {compactHints.counter}</span>}
          </div>}

        {history.length > 0 &&
          <div style={rowStyle} aria-label="Attack history">
            {history.map((attack, index) =>
              <img key={`${attack}-${index}`} src={`/static/art/ui/small_${attack}.png`}
                style={pipStyle} alt={attack} />)}
          </div>}

        {(availableFinisher || combos.length > 0) &&
          <div style={rowStyle}>
            {availableFinisher && <strong style={{ color: '#ffd45a' }}>Ready: {availableFinisher}</strong>}
            {!availableFinisher && combos.slice(0, 1).map((combo, index) => (
              <span key={index}>
                Next: {(combo.remaining_attacks || []).join(' + ')} → {combo.name}
              </span>
            ))}
          </div>}

        {targetEffects.length > 0 &&
          <div style={rowStyle} aria-label="Target effects">
            {targetEffects.map((effect) => <span key={effect} style={badgeStyle}>{effect}</span>)}
          </div>}
      </aside>
    );
  }
}
