import * as React from "react";
import { Global } from "../../core/global";
import MobilePanelScreen from "./mobilePanelScreen";

interface TrueDeathPanelProps {
  heroName: string,
  heroRank: string,
  totalXp: integer,
  scoreTotal?: number,
  scoreBreakdown?: any,
  daysSurvived?: number,
  wavesSurvived?: number,
  highestPressureLevel?: number,
  legendaryKills?: number,
  hideoutsCleared?: number,
  fate: string,
}

export default class TrueDeathPanel extends React.Component<TrueDeathPanelProps, any> {
  handleOkClick = () => {
    Global.network.sendRecreateHero();
    window.location.reload();
  };

  render() {
    const imageName = this.props.heroRank.toLowerCase().replace(/\s/g, '');
    const rows = [
      ['Final Score', (this.props.scoreTotal || this.props.totalXp).toLocaleString()],
      ['Total XP Earned', this.props.totalXp],
      ['Days Survived', this.props.daysSurvived || 0],
      ['Waves Survived', this.props.wavesSurvived || 0],
      ['Legendary Kills', this.props.legendaryKills || 0],
      ['Fate', this.props.fate],
    ];
    if (this.props.scoreBreakdown) {
      rows.push(
        ['Survival score', this.props.scoreBreakdown.survival || 0],
        ['Progression score', this.props.scoreBreakdown.progression || 0],
        ['Valor / combat score', this.props.scoreBreakdown.valor || 0],
      );
    }

    const footer = (
      <button type="button" onClick={this.handleOkClick} style={{
        width: '100%', minHeight: '46px', border: '1px solid #8f754e', borderRadius: '5px',
        background: '#25282b', color: '#f2e7cf', fontFamily: 'Cinzel, Verdana, serif', fontWeight: 700,
      }}>
        Begin a New Legend
      </button>
    );

    return (
      <MobilePanelScreen panelType="true_death" title="Your Legend Has Ended" hideExitButton footer={footer}>
        <div style={{ textAlign: 'center' }}>
          <img src={`/static/art/${imageName}_single.png`} alt={this.props.heroName}
            style={{ width: '112px', height: '112px', objectFit: 'contain', imageRendering: 'pixelated' }} />
          <p style={{ color: '#f2e7cf', fontSize: '14px', margin: '4px 0 14px' }}>
            The legend of {this.props.heroName} has ended.
          </p>
          <dl style={{ margin: 0, textAlign: 'left' }}>
            {rows.map(([label, value]) => (
              <div key={String(label)} style={{
                display: 'flex', justifyContent: 'space-between', gap: '12px', padding: '7px 3px',
                borderBottom: '1px solid rgba(255,255,255,.1)', fontSize: '12px',
              }}>
                <dt style={{ color: '#aaa' }}>{label}</dt>
                <dd style={{ margin: 0, color: '#f2e7cf', textAlign: 'right' }}>{value}</dd>
              </div>
            ))}
          </dl>
        </div>
      </MobilePanelScreen>
    );
  }
}
