import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import {
  MobilePanelActions,
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
  resourceImageForName,
} from "./mobilePanelLayout";

interface TileResourceDetailPanelProps {
  tileData,
}

export default class TileResourceDetailPanel extends React.Component<TileResourceDetailPanelProps, any> {
  constructor(props) {
    super(props);

    if (this.props.tileData.resources.length > 0) {
      this.state = {
        resource: this.props.tileData.resources[0],
        index: 0
      };
    } else {
      this.state = {
        resource: null,
        index: 0
      }
    }

    this.handleLeftClick = this.handleLeftClick.bind(this);
    this.handleRightClick = this.handleRightClick.bind(this);
  }

  handleLeftClick(event) {
    if (this.state.index != 0) {
      const newIndex = this.state.index - 1;
      this.setState({
        resource: this.props.tileData.resources[newIndex],
        index: newIndex
      })
    }
  }

  handleRightClick(event) {
    if (this.state.index != (this.props.tileData.resources.length - 1)) {
      const newIndex = this.state.index + 1;
      this.setState({
        resource: this.props.tileData.resources[newIndex],
        index: newIndex
      })
    }
  }

  render() {
    const zeroResources = this.props.tileData.resources.length == 0;

    var imageName;
    var resourceTitle;
    if (!zeroResources) {
      imageName = this.state.resource.name.replace(/\s/g, '').toLowerCase();
      resourceTitle = this.state.resource.name;
    } else {
      resourceTitle = 'No resources found.'
    }

    var properties = [];

    if (!zeroResources) {
      if (this.state.resource.properties) {
        for (var i = 0; i < this.state.resource.properties.length; i++) {
          properties.push(`+${this.state.resource.properties[i].value} ${this.state.resource.properties[i].name}`);
        }
      }
    }
    const landscape = isLandscapeMobile();
    const atFirst = this.state.index == 0;
    const atLast = this.state.index == (this.props.tileData.resources.length - 1);

    return (
      <MobilePanelScreen
        panelType={'tile_resource_detail'}
        title={'Resource'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={
            <MobileSummaryCard
              imageSrc={!zeroResources ? '/static/art/items/' + imageName + '.png' : undefined}
              title={resourceTitle}
              subtitle={!zeroResources ? `${this.state.index + 1} / ${this.props.tileData.resources.length}` : null}
              imageSize={48} />
          }
          right={
            <>
              {!zeroResources &&
                <MobileStatsList rows={[
                  { label: 'Quantity', value: this.state.resource.quantity_label },
                  { label: 'Yield', value: this.state.resource.yield_label },
                  { label: 'Properties', value: properties.join(', '), hidden: properties.length == 0 },
                ]} />}
              {!zeroResources &&
                <MobilePanelActions actions={[
                  { key: 'previous', label: 'Previous resource', icon: leftbutton, onClick: this.handleLeftClick, disabled: atFirst },
                  { key: 'next', label: 'Next resource', icon: rightbutton, onClick: this.handleRightClick, disabled: atLast },
                ]} />}
            </>
          } />
      </MobilePanelScreen>
    );
  }
}
