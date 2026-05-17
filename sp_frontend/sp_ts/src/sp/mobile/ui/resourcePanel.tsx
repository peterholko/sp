import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import {
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

interface ResourceProps {
  resourceData,
}

export default class ResourcePanel extends React.Component<ResourceProps, any> {
  constructor(props) {
    super(props);

    this.state = {
    };
   
  }

  render() {
    const landscape = isLandscapeMobile();
    const properties = [];

    if(this.props.resourceData.properties) {
      for(var i = 0; i < this.props.resourceData.properties.length; i++) {
        properties.push(`+${this.props.resourceData.properties[i].value} ${this.props.resourceData.properties[i].name}`);
      }
    }


    return (
      <MobilePanelScreen
        panelType={'resource'}
        title={'Resource'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={<MobileSummaryCard imageSrc={'/static/art/items/' + this.props.resourceData.image + '.png'} title={this.props.resourceData.name} imageSize={48} />}
          right={<MobileStatsList rows={[
            { label: 'Quantity', value: this.props.resourceData.quantityLabel },
            { label: 'Yield', value: this.props.resourceData.yieldLabel },
            { label: 'Properties', value: properties.join(', '), hidden: properties.length == 0 },
          ]} />} />
      </MobilePanelScreen>
    );
  }
}
