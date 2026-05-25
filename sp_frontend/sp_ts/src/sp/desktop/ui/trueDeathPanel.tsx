import * as React from "react";
import halfpanel from "ui_comp/halfpanel.png";
import okbutton from "ui_comp/okbutton.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";

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
  constructor(props) {
    super(props);

    this.state = {
    };
       
    this.handleOkClick = this.handleOkClick.bind(this);
  }

  handleOkClick() {
    Global.network.sendRecreateHero();
    window.location.reload();
  }

  render() {
    let imageName = this.props.heroRank.toLowerCase().replace(/\s/g, '');
    let imagePath = '/static/art/' + imageName + '_single.png';

    var halfPanelStyle = {
      top: '50%',
      left: '50%',
      width: '323px',
      height: '430px',
      marginTop: '-215px',
      marginLeft: '-161px',
      position: 'fixed',
      zIndex: 7
    } as React.CSSProperties

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


    const okButtonStyle = {
      transform: 'translate(-186px, 290px)',
      position: 'fixed'
    } as React.CSSProperties

    return (
      <div style={halfPanelStyle}>
        <img src={halfpanel} />
        <img src={imagePath} style={heroStyle} />
        <span style={spanNameStyle}>The legend of {this.props.heroName} has ended.</span>
        <table style={tableStyle}>
          <tbody>
            <tr>
              <td>Final Score: </td>
              <td>{(this.props.scoreTotal || this.props.totalXp).toLocaleString()}</td>
            </tr>
            <tr>
              <td>Total Xp Earned: </td>
              <td>{this.props.totalXp}</td>
            </tr>
            <tr>
              <td>Days Survived: </td>
              <td>{this.props.daysSurvived || 0}</td>
            </tr>
            <tr>
              <td>Waves Survived: </td>
              <td>{this.props.wavesSurvived || 0}</td>
            </tr>
            <tr>
              <td>Legendary Kills: </td>
              <td>{this.props.legendaryKills || 0}</td>
            </tr>
            <tr>
              <td>Fate: </td>
              <td>{this.props.fate}</td>
            </tr>
            {this.props.scoreBreakdown &&
              <React.Fragment>
                <tr>
                  <td>Survival score: </td>
                  <td>{this.props.scoreBreakdown.survival || 0}</td>
                </tr>
                <tr>
                  <td>Progression score: </td>
                  <td>{this.props.scoreBreakdown.progression || 0}</td>
                </tr>
                <tr>
                  <td>Valor/combat score: </td>
                  <td>{this.props.scoreBreakdown.valor || 0}</td>
                </tr>
              </React.Fragment>}
          </tbody>
        </table>
        <img src={okbutton} style={okButtonStyle} onClick={this.handleOkClick}/>
      </div>
    );
  }
}
