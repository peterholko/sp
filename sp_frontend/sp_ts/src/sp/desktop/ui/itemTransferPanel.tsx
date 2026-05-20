
import * as React from "react";
import transferbutton from "ui_comp/transferbutton.png";
import BaseInventoryPanel from "./baseInventoryPanel";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { Network } from "../../core/network";
import { STRUCTURE, FOUNDED, PLANNING_UPGRADE } from "../../core/config";
import FoundedInventoryPanel from "./foundedInventoryPanel";
import SmallButton from "./smallButton";
import UpgradeInventoryPanel from "./upgradeInventoryPanel";
import { getHalfPanelOffsetMarginTop } from "../../core/uiLayout";

interface ITPProps {
  leftInventoryData,
  rightInventoryData,
  reqs
}

export default class ItemTransferPanel extends React.Component<ITPProps, any> {
  constructor(props) {
    super(props);

    Global.selectedItemId = -1;
    Global.selectedItemOwnerId = -1;

    this.state = {
      hideLeftSelect: true,
      hideRightSelect: true,
      leftSelectedItemId: -1,
      rightSelectedItemId: -1
    };

    this.handleSelect = this.handleSelect.bind(this);
    this.handleItemTransferClick = this.handleItemTransferClick.bind(this);
  }

  handleSelect(eventData) {
    if (Global.selectedItemOwnerId == this.props.leftInventoryData.id) {
      this.setState({
        hideLeftSelect: false,
        hideRightSelect: true,
        leftSelectedItemId: eventData.itemId,
        rightSelectedItemId: -1
      });
    } else {
      this.setState({
        hideLeftSelect: true,
        hideRightSelect: false,
        leftSelectedItemId: -1,
        rightSelectedItemId: eventData.itemId
      });
    }
  }

  handleItemTransferClick(event: React.MouseEvent) {
    console.log('Item Transfer Click');
    if (Global.selectedItemId != -1) {
      var sourceId;
      var targetId;

      if (Global.selectedItemOwnerId == this.props.leftInventoryData.id) {
        sourceId = this.props.leftInventoryData.id;
        targetId = this.props.rightInventoryData.id;
      } else {
        sourceId = this.props.rightInventoryData.id;
        targetId = this.props.leftInventoryData.id;
      }
      this.setState({
        hideLeftSelect: true,
        hideRightSelect: true
      });

      Global.network.sendItemTransfer(Global.selectedItemId, sourceId, targetId);

      //Reset Global selected item / owner
      Global.selectedItemId = -1;
      Global.selectedItemOwnerId = -1;
      Global.selectedItemName = '';
    }
  }

  render() {
    console.log("Left: ");
    console.log(this.props.leftInventoryData);
    console.log("Right: ");
    console.log(this.props.rightInventoryData);
    var objState = Global.objectStates[this.props.rightInventoryData.id];

    // default to false if objState is null or undefined
    var isFounded = objState && objState.class == STRUCTURE && objState.state == FOUNDED;
    var isPlanningUpgrade = objState && objState.class == STRUCTURE && objState.state == PLANNING_UPGRADE;

    const transferY = getHalfPanelOffsetMarginTop(155);
    const itemNameY = getHalfPanelOffsetMarginTop(305);

    const transferStyle = {
      top: '50%',
      left: '50%',
      marginTop: transferY,
      marginLeft: '-25px',
      position: 'fixed',
      zIndex: Global.zIndexManager.getTop() + 3 // 3 layers above top due to multiple panels in this component
    } as React.CSSProperties

    const itemNameZIndex = Global.zIndexManager.getTop() + 2;

    const leftItemNameStyle = {
      transform: 'translate(-323px, 0px)',
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px',
      zIndex: itemNameZIndex,
      top: '50%',
      left: '50%',
      marginTop: itemNameY
    } as React.CSSProperties

    const rightItemNameStyle = {
      position: 'fixed',
      textAlign: 'center',
      color: 'white',
      fontFamily: 'Verdana',
      fontSize: '12px',
      width: '323px',
      zIndex: itemNameZIndex,
      top: '50%',
      left: '50%',
      marginTop: itemNameY
    } as React.CSSProperties

    return (
      <div>
        <BaseInventoryPanel left={true}
          id={this.props.leftInventoryData.id}
          items={this.props.leftInventoryData.items}
          capacity={this.props.leftInventoryData.cap}
          totalWeight={this.props.leftInventoryData.tw}
          panelType={'itemTransfer'}
          hideExitButton={true}
          hideSelect={this.state.hideLeftSelect}
          showEquipped={true}
          handleSelect={this.handleSelect}
          selectedItemId={this.state.leftSelectedItemId} />
          
        {!isFounded &&
          <BaseInventoryPanel left={false}
            id={this.props.rightInventoryData.id}
            items={this.props.rightInventoryData.items}
            capacity={this.props.rightInventoryData.cap}
            totalWeight={this.props.rightInventoryData.tw}
            panelType={'itemTransfer'}
            hideExitButton={false}
            hideSelect={this.state.hideRightSelect}
            showEquipped={true}
            handleSelect={this.handleSelect}
            selectedItemId={this.state.rightSelectedItemId} />}

        {isFounded &&
          <FoundedInventoryPanel id={this.props.rightInventoryData.id}
            items={this.props.rightInventoryData.items}
            reqs={this.props.reqs}
            panelType={'itemTransfer'}
            hideExitButton={false}
            hideSelect={this.state.hideRightSelect}
            handleSelect={this.handleSelect}
            selectedItemId={this.state.rightSelectedItemId} />}

        {isPlanningUpgrade &&
          <UpgradeInventoryPanel id={this.props.rightInventoryData.id}
            items={this.props.rightInventoryData.items}
            reqs={this.props.reqs}
            panelType={'itemTransfer'}
            hideExitButton={false}
            hideSelect={this.state.hideRightSelect}
            handleSelect={this.handleSelect} />}

        {!this.state.hideLeftSelect &&
          <span style={leftItemNameStyle}>{Global.selectedItemName}</span>}

        {!this.state.hideRightSelect &&
          <span style={rightItemNameStyle}>{Global.selectedItemName}</span>}

        <SmallButton handler={this.handleItemTransferClick}
          imageName="transferbutton"
          style={transferStyle} />

      </div>
    );
  }
}
