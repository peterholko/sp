import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import { Global } from "../../core/global";
import {
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

interface AttrsPanelProps {
  attrsData,
}

export default class AttrPanel extends React.Component<AttrsPanelProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };
   
  }

  render() {
    var objId = this.props.attrsData.id;
    var imageName = Global.objectStates[objId].image;
    imageName = imageName.replace(/ /g, '') + '_single.png';
    var name = Global.objectStates[objId].name;

    const rows = [];
    const landscape = isLandscapeMobile();

    for(var attr in this.props.attrsData.attrs) {
      rows.push({ label: attr, value: this.props.attrsData.attrs[attr] });
    }

    return (
      <MobilePanelScreen
        panelType={'attrs'}
        title={'Attributes'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={<MobileSummaryCard imageSrc={'/static/art/' + imageName} title={name} imageSize={landscape ? 58 : 82} />}
          right={<MobileStatsList rows={rows} />} />
      </MobilePanelScreen>
    );
  }
}
