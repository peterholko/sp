import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import {
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

interface TerrainFeaturePanelProps {
  tileData,
}

export default class TerrainFeaturePanel extends React.Component<TerrainFeaturePanelProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };

  }
  render() {
    const feature = this.props.tileData.terrain_features[0];
    const landscape = isLandscapeMobile();

    return (
      <MobilePanelScreen
        panelType={'terrain_features'}
        title={'Terrain Feature'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={<MobileSummaryCard imageSrc={'/static/art/features/' + feature.image + '.png'} title={feature.name} imageSize={landscape ? 72 : 96} />}
          right={<MobileStatsList rows={[
            { label: 'Bonus', value: feature.bonus },
          ]} />} />
      </MobilePanelScreen>
    );
  }
}
