import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import {
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

interface NPCPanelProps {
  npcData,
}

export default class NPCPanel extends React.Component<NPCPanelProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };
    
  }

  render() {
    let imagePath = '/static/art/' + this.props.npcData.image  + '_single.png';

    var effects = this.props.npcData.effects.join();
    const landscape = isLandscapeMobile();

    return (
      <MobilePanelScreen
        panelType={'npc'}
        title={'NPC'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={<MobileSummaryCard imageSrc={imagePath} title={this.props.npcData.name} imageSize={landscape ? 58 : 82} />}
          right={<MobileStatsList rows={[
            { label: 'State', value: this.props.npcData.state },
            { label: 'Effects', value: effects },
          ]} />} />
      </MobilePanelScreen>
    );
  }
}
