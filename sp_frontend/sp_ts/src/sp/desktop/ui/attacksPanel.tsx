
import * as React from "react";
import attackspanel from "ui_comp/attacksframe.png"
import { isWideScreen } from "../../core/config";

const MAX_ATTACKS = 6;

interface AttacksProp {
  attacks,
  combatState?: any,
}

export default class AttacksPanel extends React.Component<AttacksProp, any> {
  constructor(props) {
    super(props);
  }

  render() {
    var attacks = [];
    var startingIndex = 0;
    const combatState = this.props.combatState || {};
    const attackHistory = combatState.attack_history || this.props.attacks || [];
    const combos = combatState.matching_combos || [];
    const availableFinisher = combatState.available_finisher;
    const counterHint = combatState.counter_hint;
    const enemyIntent = combatState.enemy_intent;

    if (attackHistory.length > MAX_ATTACKS) {
      startingIndex = attackHistory.length - MAX_ATTACKS;
    }

    var renderingIndex = 0;

    for (var i = startingIndex; i < attackHistory.length; i++) {
      var xPos = 3 + renderingIndex * 17;
      renderingIndex++;

      const style = {
        transform: 'translate(' + xPos + 'px, ' + 3 + 'px)',
        position: 'fixed'
      } as React.CSSProperties

      attacks.push(<img key={i} src={'/static/art/ui/small_' + attackHistory[i] + '.png'}
        style={style} />)
    }

    const attacksStyle = {
      bottom: '85px',
      left: '50%',
      marginLeft: '-130px',
      position: 'fixed',
      zIndex: 6
    } as React.CSSProperties

    const panelStyle = {
      position: 'fixed'
    } as React.CSSProperties

    const wide = isWideScreen();

    const hintsStyle = (wide ? {
      position: 'fixed',
      bottom: 'calc(50% - 500px)',
      left: 'calc(50% + 612px)',
      zIndex: 6,
      display: 'flex',
      flexDirection: 'column',
      gap: '2px',
      alignItems: 'flex-start',
    } : {
      position: 'fixed',
      bottom: '110px',
      left: '50%',
      marginLeft: '-130px',
      zIndex: 6,
      display: 'flex',
      flexDirection: 'column',
      gap: '2px',
    }) as React.CSSProperties

    const intentStyle = (wide ? {
      position: 'fixed',
      bottom: 'calc(50% - 500px + 90px)',
      left: 'calc(50% + 612px)',
      zIndex: 6,
      display: 'flex',
      flexDirection: 'column',
      gap: '2px',
      alignItems: 'flex-start',
    } : {
      ...hintsStyle,
      bottom: '145px',
    }) as React.CSSProperties

    const hintRowStyle = {
      display: 'flex',
      alignItems: 'center',
      gap: '3px',
      background: 'rgba(0,0,0,0.7)',
      borderRadius: '3px',
      padding: '2px 6px',
      whiteSpace: 'nowrap',
    } as React.CSSProperties

    const hintNameStyle = {
      color: '#ffd700',
      fontFamily: 'Verdana',
      fontSize: '11px',
      userSelect: 'none',
    } as React.CSSProperties

    const hintLabelStyle = {
      color: '#d4d4d4',
      fontFamily: 'Verdana',
      fontSize: '10px',
      userSelect: 'none',
    } as React.CSSProperties

    const hintEffectStyle = {
      color: '#ff6b6b',
      fontFamily: 'Verdana',
      fontSize: '10px',
      fontStyle: 'italic',
      userSelect: 'none',
    } as React.CSSProperties

    const arrowStyle = {
      color: '#888',
      fontFamily: 'Verdana',
      fontSize: '10px',
      userSelect: 'none',
    } as React.CSSProperties

    return (
      <div>
        {(combos.length > 0 || availableFinisher) &&
          <div style={hintsStyle}>
            {availableFinisher &&
              <div style={hintRowStyle}>
                <span style={hintNameStyle}>Combo ready</span>
                <span style={arrowStyle}>=</span>
                <span style={hintEffectStyle}>{availableFinisher}</span>
              </div>}
            {combos.map((combo, idx) => (
              <div key={idx} style={hintRowStyle}>
                <span style={arrowStyle}>-&gt;</span>
                {(combo.remaining_attacks || []).map((atk, j) => (
                  <img key={j} src={'/static/art/ui/small_' + atk + '.png'}
                    style={{ width: '14px', height: '14px' }} />
                ))}
                <span style={arrowStyle}>=</span>
                <span style={hintNameStyle}>{combo.name}</span>
                {combo.effect &&
                  <span style={hintEffectStyle}>({combo.effect})</span>}
              </div>
            ))}
          </div>
        }
        <div style={attacksStyle}>
          <img src={attackspanel} style={panelStyle} />
          {attacks}
        </div>
      </div>
    );
  }
}
