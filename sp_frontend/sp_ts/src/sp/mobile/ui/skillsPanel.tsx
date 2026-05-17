
import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import { Global } from "../../core/global";
import {
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

interface SkillsPanelProps {
  skillsData,
}

export default class SkillsPanel extends React.Component<SkillsPanelProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };
   
  }

  render() {
    var objId = this.props.skillsData.id;
    var imageName = Global.objectStates[objId].image;
    imageName = imageName.replace(/ /g, '') + '_single.png';
    var name = Global.objectStates[objId].name;

    const rows = [];
    const landscape = isLandscapeMobile();

    for(var skill in this.props.skillsData.skills) {
      const data = this.props.skillsData.skills[skill];
      rows.push({ label: skill, value: `Lvl ${data.level} | ${data.xp}/${data.next}` });
    }

    return (
      <MobilePanelScreen
        panelType={'skills'}
        title={'Skills'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={<MobileSummaryCard imageSrc={'/static/art/' + imageName} title={name} imageSize={landscape ? 58 : 82} />}
          right={<MobileStatsList rows={rows} />} />
      </MobilePanelScreen>
    );
  }
}
