import * as React from "react";
import attrsbutton from "ui_comp/attrsbutton.png";
import skillsbutton from "ui_comp/skillsbutton.png";
import upgradebutton from "ui_comp/upgradebutton.png";
import { Global } from "../../core/global";
import MobilePanelScreen from "./mobilePanelScreen";

interface HeroPanelProps {
  heroData,
}

export default class HeroPanel extends React.Component<HeroPanelProps, any> {
  constructor(props) {
    super(props);
    this.handleAttrsClick = this.handleAttrsClick.bind(this);
    this.handleSkillsClick = this.handleSkillsClick.bind(this);
    this.handleAdvanceClick = this.handleAdvanceClick.bind(this);
  }

  handleAttrsClick() {
    Global.network.sendInfoAttrs(Global.heroId);
  }

  handleSkillsClick() {
    Global.network.sendInfoSkills(Global.heroId);
  }

  handleAdvanceClick() {
    Global.network.sendInfoAdvance(Global.heroId);
  }

  renderActionButton(image, label, handler) {
    const buttonStyle: React.CSSProperties = {
      minHeight: '44px',
      border: '1px solid rgba(201, 170, 113, 0.55)',
      borderRadius: '4px',
      background: '#25282b',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
    };

    return (
      <button type="button" style={buttonStyle} aria-label={label} title={label} onClick={handler}>
        <img src={image} style={{ width: '40px', height: '40px' }} />
      </button>
    );
  }

  render() {
    const imageName = Global.objectStates[Global.heroId].image.toLowerCase().replace(/\s/g, '');
    const imagePath = '/static/art/' + imageName + '_single.png';
    const formatEffectValue = (effectValue) => {
      if (typeof effectValue === "number") {
        return effectValue > 0 ? '+' + String(effectValue) : String(effectValue);
      }

      return String(effectValue);
    };
    const effects = [];

    for (let i = 0; i < this.props.heroData.effects.length; i++) {
      const effectInfo = this.props.heroData.effects[i];
      const effectName = effectInfo.effect;
      const effectAttrs = effectInfo.attrs || {};
      let duration = '';
      let displayedAttrs = 0;

      for (const effectKey in effectAttrs) {
        const effectValue = effectAttrs[effectKey];
        if (effectKey == "Duration") {
          duration = effectValue < 0 ? '' : String(effectValue);
        }
      }

      for (const effectKey in effectAttrs) {
        let effectValue = effectAttrs[effectKey];
        if (effectKey == "Duration") continue;

        effectValue = formatEffectValue(effectValue);
        displayedAttrs += 1;

        effects.push(
          <tr key={effectName + effectKey}>
            <td colSpan={2}>{effectName} [{effectValue} {effectKey}{duration ? ' ' + duration : ''}]</td>
          </tr>
        );
      }

      if (displayedAttrs == 0) {
        effects.push(
          <tr key={effectName}>
            <td colSpan={2}>{duration ? effectName + ' [Duration ' + duration + ']' : effectName}</td>
          </tr>
        );
      }
    }

    const summaryStyle: React.CSSProperties = {
      display: 'flex',
      alignItems: 'center',
      gap: '12px',
      marginBottom: '14px',
    };

    const heroStyle: React.CSSProperties = {
      width: '58px',
      height: '58px',
      objectFit: 'contain',
      imageRendering: 'pixelated',
    };

    const nameStyle: React.CSSProperties = {
      color: '#f2e7cf',
      fontFamily: 'Cinzel, Verdana, serif',
      fontSize: '18px',
      fontWeight: 'bold',
      lineHeight: 1.2,
    };

    const tableStyle: React.CSSProperties = {
      width: '100%',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      borderCollapse: 'separate',
      borderSpacing: '0 7px',
    };

    const footerStyle: React.CSSProperties = {
      display: 'grid',
      gridTemplateColumns: 'repeat(3, 1fr)',
      gap: '8px',
    };

    return (
      <MobilePanelScreen
        panelType="hero"
        title="Hero"
        footer={
          <div style={footerStyle}>
            {this.renderActionButton(attrsbutton, 'Attributes', this.handleAttrsClick)}
            {this.renderActionButton(skillsbutton, 'Skills', this.handleSkillsClick)}
            {this.renderActionButton(upgradebutton, 'Advance', this.handleAdvanceClick)}
          </div>
        }
      >
        <div style={summaryStyle}>
          <img src={imagePath} style={heroStyle} />
          <div style={nameStyle}>{this.props.heroData.name}</div>
        </div>
        <table style={tableStyle}>
          <tbody>
            <tr><td>Health:</td><td>{this.props.heroData.hp}</td></tr>
            <tr><td>Stamina:</td><td>{this.props.heroData.stamina}</td></tr>
            <tr><td>Thirst:</td><td>{this.props.heroData.thirst}</td></tr>
            <tr><td>Hunger:</td><td>{this.props.heroData.hunger}</td></tr>
            <tr><td>Fatigue:</td><td>{this.props.heroData.tiredness}</td></tr>
            <tr><td>State:</td><td>{this.props.heroData.state}</td></tr>
            <tr><td>Damage:</td><td>{this.props.heroData.total_dmg}</td></tr>
            <tr><td>Defense:</td><td>{this.props.heroData.total_def}</td></tr>
            <tr><td>Vision:</td><td>{this.props.heroData.vision}</td></tr>
            {effects}
          </tbody>
        </table>
      </MobilePanelScreen>
    );
  }
}
