import * as React from "react";
import attrsbutton from "ui_comp/attrsbutton.png";
import skillsbutton from "ui_comp/skillsbutton.png";
import { Global } from "../../core/global";
import { getNeedStatusIcon, NeedKind } from "./needStatus";
import MobilePanelScreen from "./mobilePanelScreen";

interface VillagerPanelProps {
  villagerData,
  activity,
  needsData
}

export default class VillagerPanel extends React.Component<VillagerPanelProps, any> {
  constructor(props) {
    super(props);
    this.handleAttrsClick = this.handleAttrsClick.bind(this);
    this.handleSkillsClick = this.handleSkillsClick.bind(this);
  }

  handleAttrsClick() {
    Global.network.sendInfoAttrs(this.props.villagerData.id);
  }

  handleSkillsClick() {
    Global.network.sendInfoSkills(this.props.villagerData.id);
  }

  renderNeedValue(kind: NeedKind, value?: string) {
    const statusIcon = getNeedStatusIcon(kind, value);
    const statusIconStyle: React.CSSProperties = {
      width: '12px',
      height: '12px',
      marginRight: '5px',
      verticalAlign: 'middle'
    };

    return (
      <td>
        {statusIcon && <img src={statusIcon} style={statusIconStyle} />}
        <span>{value}</span>
      </td>
    );
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
    let imageName = this.props.villagerData.image;
    imageName = imageName.replace(/ /g, '') + '_single.png';

    const effects = this.props.villagerData.effects.join();
    const activity = this.props.activity && this.props.activity.id == this.props.villagerData.id
      ? this.props.activity.activity
      : this.props.villagerData.activity;
    const needs = this.props.needsData && this.props.needsData.id == this.props.villagerData.id
      ? this.props.needsData
      : this.props.villagerData;

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
      gridTemplateColumns: 'repeat(2, 1fr)',
      gap: '8px',
    };

    return (
      <MobilePanelScreen
        panelType="villager"
        title="Villager"
        footer={
          <div style={footerStyle}>
            {this.renderActionButton(attrsbutton, 'Attributes', this.handleAttrsClick)}
            {this.renderActionButton(skillsbutton, 'Skills', this.handleSkillsClick)}
          </div>
        }
      >
        <div style={summaryStyle}>
          <img src={'/static/art/' + imageName} style={heroStyle} />
          <div style={nameStyle}>{this.props.villagerData.name}</div>
        </div>
        <table style={tableStyle}>
          <tbody>
            <tr><td>Activity:</td><td>{activity}</td></tr>
            <tr><td>Order:</td><td>{this.props.villagerData.order}</td></tr>
            <tr><td>Thirst:</td>{this.renderNeedValue("thirst", needs.thirst)}</tr>
            <tr><td>Hunger:</td>{this.renderNeedValue("hunger", needs.hunger)}</tr>
            <tr><td>Tiredness:</td>{this.renderNeedValue("tiredness", needs.tiredness)}</tr>
            <tr><td>Hp:</td><td>{this.props.villagerData.hp} / {this.props.villagerData.base_hp}</td></tr>
            <tr><td>Stamina:</td><td>{this.props.villagerData.stamina} / {this.props.villagerData.base_stamina}</td></tr>
            <tr><td>Speed:</td><td>{this.props.villagerData.base_speed}</td></tr>
            <tr><td>State:</td><td>{this.props.villagerData.state}</td></tr>
            <tr><td>Shelter:</td><td>{this.props.villagerData.shelter}</td></tr>
            <tr><td>Structure:</td><td>{this.props.villagerData.structure}</td></tr>
            <tr><td>Effects:</td><td>{effects}</td></tr>
          </tbody>
        </table>
      </MobilePanelScreen>
    );
  }
}
