import * as React from "react";
import HalfPanel from "./halfPanel";
import attrsbutton from "ui_comp/attrsbutton.png";
import skillsbutton from "ui_comp/skillsbutton.png";
import { GameEvent } from "../../core/gameEvent";
import { Network } from "../../core/network";
import SmallButton from "./smallButton";
import { Global } from "../../core/global";
import { NetworkEvent } from "../../core/networkEvent";
import { getNeedStatusIcon, NeedKind } from "./needStatus";
import { getHalfPanelOffsetMarginTop } from "../../core/uiLayout";
import { characterImageUrl } from "../../core/portraitCatalog";
import { villagerPanelPresentation } from "../../core/villagerPanelPresentation";

interface VillagerPanelProps {
  villagerData,
  activity,
  needsData
}

export default class VillagerPanel extends React.Component<VillagerPanelProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };
   
    this.handleAttrsClick = this.handleAttrsClick.bind(this)
    this.handleSkillsClick = this.handleSkillsClick.bind(this)
  }

  handleAttrsClick() {
    Global.network.sendInfoAttrs(this.props.villagerData.id);
  }

  handleSkillsClick() {
    Global.network.sendInfoSkills(this.props.villagerData.id);
  }

  renderNeedValue(kind: NeedKind, value?: string) {
    const statusIcon = getNeedStatusIcon(kind, value);
    const statusIconStyle = {
      width: '12px',
      height: '12px',
      marginRight: '5px',
      verticalAlign: 'middle'
    } as React.CSSProperties

    return (
      <td>
        {statusIcon && <img src={statusIcon} style={statusIconStyle} />}
        <span>{value}</span>
      </td>
    );
  }

  render() {
    const attrsY = getHalfPanelOffsetMarginTop(80);
    const skillsY = getHalfPanelOffsetMarginTop(130);

    const imagePath = characterImageUrl(
      this.props.villagerData.portrait,
      this.props.villagerData.image,
    );

    
    var effects = this.props.villagerData.effects.join();

    const status = villagerPanelPresentation(
      this.props.villagerData,
      this.props.activity,
      this.props.needsData,
    );

    /*for(var i = 0; i < this.props.villagerData.effects.length; i++) {
      effects = effects + ', ' + this.props.villagerData.effects[i];
    }*/

    const heroStyle = {
      transform: 'translate(-197px, 20px)',
      position: 'fixed',
      width: '72px',
      height: '72px',
      boxSizing: 'border-box',
      border: '2px solid rgba(201, 170, 113, 0.82)',
      borderRadius: '5px',
      objectFit: 'cover',
      boxShadow: '0 1px 5px rgba(0, 0, 0, 0.7)'
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
      transform: 'translate(20px, -250px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px'
    } as React.CSSProperties

    const effectsStyle = {
      transform: 'translate(20px, -50px)',
      position: 'fixed',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '300px'
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

    return (
      <HalfPanel left={true} 
                 panelType={'villager'} 
                 hideExitButton={false}>
        <img src={imagePath} style={heroStyle} />
        <span style={spanNameStyle}>{this.props.villagerData.name} (Villager)</span>
        <table style={tableStyle}>
          <tbody>
	  <tr>
            <td>Activity: </td>
            <td>{status.activity}</td>
          </tr>

           <tr>
            <td>Order: </td>
            <td>{status.order}</td>
          </tr>
 
          <tr>
            <td>Thirst: </td>
            {this.renderNeedValue("thirst", status.thirst)}
          </tr>    
          <tr>
            <td>Hunger: </td>
            {this.renderNeedValue("hunger", status.hunger)}
          </tr>     
          <tr>
            <td>Tiredness: </td>
            {this.renderNeedValue("tiredness", status.tiredness)}
          </tr>                       
          <tr>
            <td>Hp: </td>
            <td>{status.hp}</td>
          </tr>
          <tr>
            <td>Stamina: </td>
            <td>{status.stamina}</td>
          </tr>
          <tr>
            <td>Speed: </td>
            <td>{status.speed}</td>
          </tr>
          <tr>
            <td>State: </td>
            <td>{status.state}</td>
          </tr>
          <tr>
            <td>Shelter: </td>
            <td>{this.props.villagerData.shelter}</td>
          </tr>
          <tr>
            <td>Structure: </td>
            <td>{this.props.villagerData.structure}</td>
          </tr>
          <tr>
            <td>Effects: </td>
            <td>{effects}</td>
          </tr>
         
          </tbody>
        </table>

        <SmallButton handler={this.handleAttrsClick}
          imageName="attrsbutton"
          style={attrsStyle} />

        <SmallButton handler={this.handleSkillsClick}
          imageName="skillsbutton"
          style={skillsStyle} />
        
      </HalfPanel>
    );
  }
}
