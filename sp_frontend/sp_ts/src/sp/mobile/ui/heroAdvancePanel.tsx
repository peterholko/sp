import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import { Global } from "../../core/global";
import advancebutton from "ui_comp/upgradebutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import {
  MobilePanelActions,
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

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

    const landscape = isLandscapeMobile();
    const arrowStyle: React.CSSProperties = {
      width: '42px',
      height: '42px',
      objectFit: 'contain',
      alignSelf: 'center',
      transform: landscape ? 'rotate(0deg)' : 'rotate(90deg)',
    };
    const nextRankStyle: React.CSSProperties = {
      border: '1px solid rgba(201, 170, 113, 0.28)',
      borderRadius: '6px',
      background: 'rgba(0, 0, 0, 0.18)',
      padding: landscape ? '8px 10px' : '10px 12px',
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      gap: '12px',
      boxSizing: 'border-box',
    };

    return (
      <MobilePanelScreen
        panelType={'advance'}
        title={'Advance'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={<MobileSummaryCard imageSrc={heroImagePath} title={heroRank} status={maxRank ? 'Max Rank' : null} imageSize={landscape ? 58 : 82} />}
          right={
            <>
              {!maxRank &&
                <div style={nextRankStyle}>
                  <MobileSummaryCard imageSrc={nextRankImagePath} title={nextRankName} imageSize={landscape ? 50 : 72} />
                  <img src={rightbutton} style={arrowStyle} />
                </div>}
              <MobileStatsList rows={[
                { label: 'XP', value: nextRankXp, hidden: maxRank },
                { label: 'Status', value: 'Max Rank', hidden: !maxRank },
              ]} />
              {!maxRank &&
                <MobilePanelActions actions={[
                  { key: 'advance', label: 'Advance', icon: advancebutton, onClick: this.handleAdvanceClick },
                ]} />}
            </>
          } />
      </MobilePanelScreen>
    );
  }
}
