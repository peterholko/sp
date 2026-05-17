import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import MobileInventoryGrid from "./mobileInventoryGrid";
import { Global } from "../../core/global";
import { Util } from "../../core/util";
import { GameEvent } from "../../core/gameEvent";
import buildbutton from "ui_comp/buildbutton.png";
import {
  MobileCard,
  MobilePanelActions,
  MobileRequirementGrid,
  MobileSplitPanelLayout,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

interface FoundedInventoryProps {
  id: integer,
  items: any,
  reqs: any,
  panelType: string,
  hideExitButton: boolean,
  hideSelect: boolean,
  handleSelect: Function,
  selectedItemId: integer,
}

export default class FoundedInventoryPanel extends React.Component<FoundedInventoryProps, any> {
  constructor(props) {
    super(props);

    this.state = {
      selectedItemId: this.props.selectedItemId
    };

    this.handleSelect = this.handleSelect.bind(this);
    this.handleBuildClick = this.handleBuildClick.bind(this);
  }

  handleSelect(eventData) {
    console.log('handleSelect ' + eventData);

    Global.selectedItemOwnerId = eventData.ownerId;
    Global.selectedItemId = eventData.itemId;

    this.setState({ selectedItemId: eventData.itemId });

    this.props.handleSelect(eventData);
  }

  handleBuildClick() {
    Global.network.sendBuild(Global.heroId, this.props.id);
    Global.gameEmitter.emit(GameEvent.START_BUILD_CLICK, {});
  }

  render() {
    const objId = this.props.id;
    const reqs = this.props.reqs || [];
    const items = this.props.items || [];
    const showBuildButton = reqs.every(req => req.cquantity == 0);
    const landscape = isLandscapeMobile();

    if (Util.isSprite(Global.objectStates[objId].image)) {
      var imageName = Global.objectStates[objId].image + '_single.png';
    } else {
      var imageName = Global.objectStates[objId].image + '.png';
    }

    const actions = showBuildButton
      ? [{ key: 'build', label: 'Build', icon: buildbutton, onClick: this.handleBuildClick }]
      : [];

    return (
      <MobilePanelScreen
        panelType={this.props.panelType}
        title={this.props.panelType}
        hideExitButton={this.props.hideExitButton}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={
            <>
              <MobileSummaryCard
                imageSrc={'/static/art/' + imageName}
                title={Global.objectStates[objId].name}
                subtitle="Foundation"
                imageSize={landscape ? 58 : 82} />
              <MobileRequirementGrid title="Requirements" requirements={reqs} showCurrent={true} />
              <MobilePanelActions actions={actions} />
            </>
          }
          right={
            <MobileCard compact={landscape}>
              <div style={{ color: '#c9aa71', fontFamily: 'Verdana', fontSize: '11px', fontWeight: 'bold', marginBottom: '8px', textTransform: 'uppercase' }}>
                Materials {items.length} / 10
              </div>
              <div style={{ marginTop: '8px' }}>
                <MobileInventoryGrid
                  ownerId={objId}
                  items={items}
                  selectedItemId={this.state.selectedItemId}
                  onSelect={this.handleSelect}
                  compact={landscape}
                  emptyLabel="No materials" />
              </div>
            </MobileCard>
          } />
      </MobilePanelScreen>
    );
  }
}
