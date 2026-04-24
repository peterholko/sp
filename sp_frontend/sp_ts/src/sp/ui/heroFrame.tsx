import * as React from "react";
import { Global } from "../global";
import heroring from "ui_comp/heroring.png";
import hpframe from "ui_comp/hpframe.png";
import statbg from "ui_comp/statbg.png";
import hpbar from "ui_comp/hpbar.png";
import stabar from "ui_comp/stabar.png";
import manabar from "ui_comp/manabar.png";
import greenstatus from "ui_comp/greenstatus.png";
import yellowstatus from "ui_comp/yellowstatus.png";
import redstatus from "ui_comp/redstatus.png";
import { NetworkEvent } from "../networkEvent";
import { STAT_BAR_WIDTH, STAT_BAR_HEIGHT } from "../config";

interface HeroFrameProps {
  heroStats: any,
  hungerStatus: string,
  thirstStatus: string,
  fatigueStatus: string
}

export default class HeroFrame extends React.Component<HeroFrameProps, any> {
  constructor(props) {
    super(props);

    this.state = {
      hideHero : true,
    };
  }

  componentDidMount() {
    Global.gameEmitter.on(NetworkEvent.PERCEPTION, this.handlePerception, this);
  }

  componentWillUnmount() {
    // avoid leaks / duplicate handlers
    Global.gameEmitter.off(NetworkEvent.PERCEPTION, this.handlePerception, this);
  }

  handlePerception() {
    this.setState({hideHero: false});
  }

  render() {
    let imagePath = '';

    const hpRatio = Global.heroMaxHp > 0 ? this.props.heroStats.hp / Global.heroMaxHp : 0;
    const hpBarWidth = hpRatio * STAT_BAR_WIDTH;

    const staRatio = Global.heroMaxStamina > 0 ? this.props.heroStats.stamina / Global.heroMaxStamina : 0;
    const staBarWidth = staRatio * STAT_BAR_WIDTH;

    const baseMana = this.props.heroStats.base_mana || Global.heroMaxMana || 0;
    const mana = this.props.heroStats.mana !== undefined ? this.props.heroStats.mana : Global.heroMana;
    const showMana = baseMana > 0;
    const manaRatio = showMana ? mana / baseMana : 0;
    const manaBarWidth = manaRatio * STAT_BAR_WIDTH;

    if(Global.heroId in Global.objectStates) {
      let imageName = Global.objectStates[Global.heroId].image.toLowerCase().replace(/\s/g, '');
      imagePath = '/static/art/' + imageName  + '_single.png';
    }

    const heroringStyle = {
      transform: 'translate(8px, 21px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const hpframeStyle = {
      transform: 'translate(41px, 10px)',
      zIndex: 2,
      position: 'fixed'
    } as React.CSSProperties

    const hpbgStyle = {
      transform: 'translate(95px, 17px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const stabgStyle = {
      transform: 'translate(95px, 35px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const manabgStyle = {
      transform: 'translate(95px, 53px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const hpBarStyle  = {
      transform: 'translate(97px, 19px)',
      width: hpBarWidth + 'px',
      height: STAT_BAR_HEIGHT + 'px',
      zIndex: 4,
      position: 'fixed' 
    } as React.CSSProperties
 
    const staBarStyle  = {
      transform: 'translate(97px, 37px)',
      width: staBarWidth + 'px',
      height: STAT_BAR_HEIGHT + 'px',
      zIndex: 4,
      position: 'fixed' 
    } as React.CSSProperties
  
    const manaBarStyle  = {
      transform: 'translate(97px, 55px)',
      width: manaBarWidth + 'px',
      height: STAT_BAR_HEIGHT + 'px',
      zIndex: 4,
      position: 'fixed' 
    } as React.CSSProperties
 
    const heroStyle = {
      transform: 'translate(13px, 24px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const thirstStatusStyle = {
      transform: 'translate(100px, 75px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties  

    const hungerStatusStyle = {
      transform: 'translate(150px, 75px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const fatigueStatusStyle = {
      transform: 'translate(200px, 75px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const tStyle = {
      transform: 'translate(90px, 79px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const hStyle = {
      transform: 'translate(140px, 79px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    const fStyle = {
      transform: 'translate(190px, 79px)',
      zIndex: 3,
      position: 'fixed'
    } as React.CSSProperties

    let thirstStatusIcon;
    let hungerStatusIcon;
    let fatigueStatusIcon;

    if(this.props.thirstStatus == 'Hydrated' || this.props.thirstStatus == 'Refreshed') {
      thirstStatusIcon = greenstatus;
    } else if(this.props.thirstStatus == 'Slightly Thirsty' || this.props.thirstStatus == 'Thirsty') {
      thirstStatusIcon = yellowstatus;
    } else if(this.props.thirstStatus == 'Parched' || this.props.thirstStatus == 'Dehydrated') {
      thirstStatusIcon = redstatus;
    }

    if(this.props.hungerStatus == 'Satiated' || this.props.hungerStatus == 'Nourished') {
      hungerStatusIcon = greenstatus;
    } else if(this.props.hungerStatus == 'Hungry' || this.props.hungerStatus == 'Peckish') {
      hungerStatusIcon = yellowstatus;
    } else if(this.props.hungerStatus == 'Famished' || this.props.hungerStatus == 'Ravenous') {
      hungerStatusIcon = redstatus;
    }

    if(this.props.fatigueStatus == 'Energized' || this.props.fatigueStatus == 'Restored') {
      fatigueStatusIcon = greenstatus;
    } else if(this.props.fatigueStatus == 'Weary' || this.props.fatigueStatus == 'Tired') {
      fatigueStatusIcon = yellowstatus;
    } else if(this.props.fatigueStatus == 'Exhausted' || this.props.fatigueStatus == 'Depleted') {
      fatigueStatusIcon = redstatus;
    }

    return (
      
      <div>
          <img src={heroring} style={heroringStyle}/>
          <img src={hpframe} style={hpframeStyle}/>

          <img src={statbg} style={hpbgStyle}/>
          <img src={hpbar} style={hpBarStyle}/>
          <img src={statbg} style={stabgStyle}/>
          <img src={stabar} style={staBarStyle}/>
          {showMana &&
            <>
              <img src={statbg} style={manabgStyle}/>
              <img src={manabar} style={manaBarStyle}/>
            </>
          }

          <span style={tStyle}>T</span>
          <span style={hStyle}>H</span>
          <span style={fStyle}>F</span>

          <img src={thirstStatusIcon} style={thirstStatusStyle}/>
          <img src={hungerStatusIcon} style={hungerStatusStyle}/>
          <img src={fatigueStatusIcon} style={fatigueStatusStyle}/>

          {!this.state.hideHero && 
            <img src={imagePath} style={heroStyle}/>
          }
      </div>
    );
  }
}
