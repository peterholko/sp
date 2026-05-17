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

    var imageName = this.props.villagerData.image;
    imageName = imageName.replace(/ /g, '') + '_single.png';

    
    var effects = this.props.villagerData.effects.join();

    var activity;

    if(this.props.activity && this.props.activity.id == this.props.villagerData.id) {
      activity = this.props.activity.activity;
    } else {
      activity = this.props.villagerData.activity;
    }

    var needs;

    if(this.props.needsData && this.props.needsData.id == this.props.villagerData.id) {
      needs = this.props.needsData;
    } else {
      needs = this.props.villagerData;
    }

    /*for(var i = 0; i < this.props.villagerData.effects.length; i++) {
      effects = effects + ', ' + this.props.villagerData.effects[i];
    }*/

    const heroStyle = {
      transform: 'translate(-197px, 25px)',
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
        <img src={'/static/art/' + imageName} style={heroStyle} />
        <span style={spanNameStyle}>{this.props.villagerData.name} (Villager)</span>
        <table style={tableStyle}>
          <tbody>
	  <tr>
            <td>Activity: </td>
            <td>{activity}</td>
          </tr>

           <tr>
            <td>Order: </td>
            <td>{this.props.villagerData.order}</td>
          </tr>
 
          <tr>
            <td>Thirst: </td>
            {this.renderNeedValue("thirst", needs.thirst)}
          </tr>    
          <tr>
            <td>Hunger: </td>
            {this.renderNeedValue("hunger", needs.hunger)}
          </tr>     
          <tr>
            <td>Tiredness: </td>
            {this.renderNeedValue("tiredness", needs.tiredness)}
          </tr>                       
          <tr>
            <td>Hp: </td>
            <td>{this.props.villagerData.hp} /  
                {this.props.villagerData.base_hp}</td>
          </tr>
          <tr>
            <td>Stamina: </td>
            <td>{this.props.villagerData.stamina} /  
                {this.props.villagerData.base_stamina}</td>
          </tr>
          <tr>
            <td>Speed: </td>
            <td>{this.props.villagerData.base_speed}</td>
          </tr>
          <tr>
            <td>State: </td>
            <td>{this.props.villagerData.state}</td>
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
