import * as React from "react";
import HalfPanel from "./halfPanel";
import attrsbutton from "ui_comp/attrsbutton.png";
import skillsbutton from "ui_comp/skillsbutton.png";
import upgradebutton from "ui_comp/upgradebutton.png";
import { Global } from "../../core/global";
import { Network } from "../../core/network";
import SmallButton from "./smallButton";
import { getHalfPanelOffsetMarginTop } from "../../core/uiLayout";

interface HeroPanelProps {
  heroData,
}

export default class HeroPanel extends React.Component<HeroPanelProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };

    this.handleAttrsClick = this.handleAttrsClick.bind(this)
    this.handleSkillsClick = this.handleSkillsClick.bind(this)
    this.handleAdvanceClick = this.handleAdvanceClick.bind(this)
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

  render() {
    let imageName = Global.objectStates[Global.heroId].image.toLowerCase().replace(/\s/g, '');
    let imagePath = '/static/art/' + imageName + '_single.png';

    const attrsY = getHalfPanelOffsetMarginTop(80);
    const skillsY = getHalfPanelOffsetMarginTop(130);
    const advanceY = getHalfPanelOffsetMarginTop(180);

    const heroStyle = {
      transform: 'translate(-195px, 25px)',
      position: 'fixed'
    } as React.CSSProperties


    const spanNameStyle = {
      transform: 'translate(-323px, 90px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    const tableStyle = {
      transform: 'translate(20px, -240px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      borderCollapse: 'separate',
      borderSpacing: '10px 0'
    } as React.CSSProperties

    const attrsStyle = {
      top: '50%',
      left: '50%',
      marginTop: attrsY,
      marginLeft: '-68px',
      position: 'fixed',
      zIndex: 7
    } as React.CSSProperties

    const skillsStyle = {
      top: '50%',
      left: '50%',
      marginTop: skillsY,
      marginLeft: '-68px',
      position: 'fixed',
      zIndex: 7
    } as React.CSSProperties

    const advanceStyle = {
      top: '50%',
      left: '50%',
      marginTop: advanceY,
      marginLeft: '-68px',
      position: 'fixed',
      zIndex: 7
    } as React.CSSProperties

    const formatEffectValue = (effectValue) => {
      if (typeof effectValue === "number") {
        return effectValue > 0 ? '+' + String(effectValue) : String(effectValue);
      }

      return String(effectValue);
    };

    var effects = [];

    for (var i = 0; i < this.props.heroData.effects.length; i++) {
      var effectInfo = this.props.heroData.effects[i];
      var effectName = effectInfo.effect;
      var effectAttrs = effectInfo.attrs || {};
      var duration = '';
      var displayedAttrs = 0;

      for (var effectKey in effectAttrs) {
        var effectValue = effectAttrs[effectKey];
        
        if(effectKey == "Duration") {
          if(effectValue < 0) {
            duration = '';
          } else {
            duration = String(effectValue);
          }
        }
      }

      for (var effectKey in effectAttrs) {
        var effectValue = effectAttrs[effectKey];

        // Skip duration
        if(effectKey == "Duration") {
          continue;
        }

        effectValue = formatEffectValue(effectValue);
        displayedAttrs += 1;

        effects.push(<tr key={effectName + effectKey}>
          <td colSpan={2}>{effectName} [{effectValue} {effectKey}{duration ? ' ' + duration : ''}]</td>
        </tr>)
      }

      if (displayedAttrs == 0) {
        effects.push(<tr key={effectName}>
          <td colSpan={2}>{duration ? effectName + ' [Duration ' + duration + ']' : effectName}</td>
        </tr>)
      }
    }

    return (
      <HalfPanel left={true}
        panelType={'hero'}
        hideExitButton={false}>
        <img src={imagePath} style={heroStyle} />
        <span style={spanNameStyle}>{this.props.heroData.name}</span>
        <table style={tableStyle}>
          <tbody>
            <tr>
              <td>Health: </td>
              <td>{this.props.heroData.hp}</td>
            </tr>
            <tr>
              <td>Stamina: </td>
              <td>{this.props.heroData.stamina}</td>
            </tr>
            <tr>
              <td>Thirst: </td>
              <td>{this.props.heroData.thirst}</td>
            </tr>
            <tr>
              <td>Hunger: </td>
              <td>{this.props.heroData.hunger}</td>
            </tr>
            <tr>
              <td>Fatigue: </td>
              <td>{this.props.heroData.tiredness}</td>
            </tr>
            <tr>
              <td>State: </td>
              <td>{this.props.heroData.state}</td>
            </tr>
            <tr>
              <td>Damage: </td>
              <td>{this.props.heroData.total_dmg}</td>
            </tr>
            <tr>
              <td>Defense: </td>
              <td>{this.props.heroData.total_def}</td>
            </tr>
            <tr>
              <td>Vision: </td>
              <td>{this.props.heroData.vision}</td>
            </tr>            
            {effects}

          </tbody>
        </table>

        <SmallButton handler={this.handleAttrsClick}
          imageName="attrsbutton"
          style={attrsStyle} />

        <SmallButton handler={this.handleSkillsClick}
          imageName="skillsbutton"
          style={skillsStyle} />

        <SmallButton handler={this.handleAdvanceClick}
          imageName="upgradebutton"
          style={advanceStyle} />

      </HalfPanel>
    );
  }
}
