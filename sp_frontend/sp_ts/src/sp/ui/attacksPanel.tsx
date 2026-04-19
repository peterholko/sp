
import * as React from "react";
import attackspanel from "ui_comp/attacksframe.png"
import { COMBO_LIST } from "../config";

const MAX_ATTACKS = 6;

interface AttacksProp {
  attacks
}

function getMatchingCombos(attacks: string[]) {
  if (attacks.length === 0) return [];

  const matches = [];

  for (const combo of COMBO_LIST) {
    const seq = combo.attacks;

    // Check if current attacks match the start of this combo
    if (attacks.length < seq.length) {
      let isPrefix = true;
      for (let i = 0; i < attacks.length; i++) {
        if (attacks[i] !== seq[i]) {
          isPrefix = false;
          break;
        }
      }
      if (isPrefix) {
        const remaining = seq.slice(attacks.length);
        matches.push({ name: combo.name, remaining, effect: combo.effect });
      }
    }
  }

  return matches;
}

export default class AttacksPanel extends React.Component<AttacksProp, any> {
  constructor(props) {
    super(props);
  }

  render() {
    var attacks = [];
    var startingIndex = 0;

    if (this.props.attacks.length > MAX_ATTACKS) {
      startingIndex = this.props.attacks.length - MAX_ATTACKS;
    }

    var renderingIndex = 0;

    for (var i = startingIndex; i < this.props.attacks.length; i++) {
      var xPos = 3 + renderingIndex * 17;
      renderingIndex++;

      const style = {
        transform: 'translate(' + xPos + 'px, ' + 3 + 'px)',
        position: 'fixed'
      } as React.CSSProperties

      attacks.push(<img key={i} src={'/static/art/ui/small_' + this.props.attacks[i] + '.png'}
        style={style} />)
    }

    const combos = getMatchingCombos(this.props.attacks);

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

    const hintsStyle = {
      position: 'fixed',
      bottom: '110px',
      left: '50%',
      marginLeft: '-130px',
      zIndex: 6,
      display: 'flex',
      flexDirection: 'column',
      gap: '2px',
    } as React.CSSProperties

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
        {combos.length > 0 &&
          <div style={hintsStyle}>
            {combos.map((combo, idx) => (
              <div key={idx} style={hintRowStyle}>
                <span style={arrowStyle}>→</span>
                {combo.remaining.map((atk, j) => (
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
