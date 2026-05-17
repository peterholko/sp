import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import {
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

interface ObjPanelProps {
  objData,
}

export default class ObjPanel extends React.Component<ObjPanelProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };

  }

  render() {
    let imagePath = '/static/art/' + this.props.objData.image + '.png';

    let hideSoulshards = true;

    if (this.props.objData.subclass == 'monolith') {
      hideSoulshards = false;
    }
    const landscape = isLandscapeMobile();

    return (
      <MobilePanelScreen
        panelType={'obj'}
        title={'Object'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={<MobileSummaryCard imageSrc={imagePath} title={this.props.objData.name} imageSize={landscape ? 58 : 82} />}
          right={<MobileStatsList rows={[
            { label: 'State', value: this.props.objData.state },
            { label: 'Soulshards', value: this.props.objData.soulshards, hidden: hideSoulshards },
          ]} />} />
      </MobilePanelScreen>
    );
  }
}
