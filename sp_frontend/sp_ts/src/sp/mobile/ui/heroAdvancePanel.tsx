import * as React from "react";
import HalfPanel from "./halfPanel";
import { Global } from "../../core/global";
import advancebutton from "ui_comp/upgradebutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import { Network } from "../../core/network";
import SmallButton from "./smallButton";

interface HAPProps {
  advanceData,
}

export default class HeroAdvancePanel extends React.Component<HAPProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };

    this.handleAdvanceClick = this.handleAdvanceClick.bind(this);
  }

  handleAdvanceClick() {
    Global.network.sendAdvance(Global.heroId);
  }

  render() {
    let heroRank = this.props.advanceData.rank;
    let maxRank = this.props.advanceData.next_rank == 'Max Rank';

    //TODO Don't render image for Max Rank

    let heroImage = heroRank.toLowerCase().replace(/\s/g, '');
    let heroImagePath = '/static/art/' + heroImage + '_single.png';

    let nextRankName = this.props.advanceData.next_rank;
    let nextRankImage = nextRankName.toLowerCase().replace(/\s/g, '');
    let nextRankImagePath = '/static/art/' + nextRankImage + '_single.png';

    let nextRankXp = 'XP: ' + this.props.advanceData.total_xp + ' / ' + this.props.advanceData.req_xp;

    let heroStyleX = -290;
    let heroNameStyleX = -300;

    if (maxRank) {
      heroStyleX = -200;
      heroNameStyleX = -210;
    }

    const heroStyle = {
      transform: 'translate(' + heroStyleX + 'px, 100px)',
      position: 'fixed'
    } as React.CSSProperties

    const nextRankImageStyle = {
      transform: 'translate(-110px, 100px)',
      position: 'fixed'
    } as React.CSSProperties

    const heroNameStyle = {
      transform: 'translate(' + heroNameStyleX + 'px, 175px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '100px'
    } as React.CSSProperties

    const maxRankNameStyle = {
      transform: 'translate(' + heroNameStyleX + 'px, 217px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '100px'
    } as React.CSSProperties

    const nextRankNameStyle = {
      transform: 'translate(-120px, 175px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '100px'
    } as React.CSSProperties

    const reqXpStyle = {
      transform: 'translate(-323px, 225px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px'
    } as React.CSSProperties

    const rightArrowStyle = {
      transform: 'translate(-185px, 125px)',
      position: 'fixed'
    } as React.CSSProperties

    const advanceStyle = {
      transform: 'translate(-185px, 285px)',
      position: 'fixed'
    } as React.CSSProperties

    return (
      <HalfPanel left={false}
        panelType={'advance'}
        hideExitButton={false}>
        <img src={heroImagePath} style={heroStyle} />

        {!maxRank && (
          <img src={nextRankImagePath} style={nextRankImageStyle} />
        )}

        {!maxRank && (
          <img src={rightbutton} style={rightArrowStyle} />
        )}

        {!maxRank && (
          <span style={reqXpStyle}>{nextRankXp}</span>
        )}
        <span style={heroNameStyle}>{heroRank}</span>

        {maxRank && (
          <span style={maxRankNameStyle}>Max Rank</span>
        )}

        {!maxRank && (
          <span style={nextRankNameStyle}>{nextRankName}</span>
        )}

        {!maxRank && (
          <SmallButton handler={this.handleAdvanceClick}
            imageName="upgradebutton"
            style={advanceStyle} />
        )}

      </HalfPanel>
    );
  }
}

