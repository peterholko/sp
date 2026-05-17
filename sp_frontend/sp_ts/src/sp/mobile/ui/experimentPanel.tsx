
import * as React from "react";
import transferbutton from "ui_comp/transferbutton.png";
import BaseInventoryPanel from "./baseInventoryPanel";
import { Global } from "../../core/global";
import itemframe from "ui_comp/itemframe.png";
import experimentbutton from "ui_comp/experimentbutton.png";
import okbutton from "ui_comp/okbutton.png";
import recipe from "art_comp/items/recipe.png";
import { Network } from "../../core/network";
import InventoryItem from "./inventoryItem";
import { EXP_RECIPE_NONE } from "../../core/config";
import selectitemborder from "ui_comp/selectitemborder.png";
import MobilePanelScreen from "./mobilePanelScreen";
import MobileInventoryGrid from "./mobileInventoryGrid";
import {
  MobileCard,
  MobilePanelActions,
  MobileSplitPanelLayout,
  MobileSummaryCard,
} from "./mobilePanelLayout";

interface ETPProps {
  expData
}

export default class ExperimentTransferPanel extends React.Component<ETPProps, any> {
  constructor(props) {
    super(props);

    Global.selectedItemId = -1;
    Global.selectedItemOwnerId = -1;

    const selectExpResStyle = {
      position: "fixed"
    } as React.CSSProperties

    this.state = {
      redrawSelect: true,
      hideLeftSelect: true,
      hideRightSelect: true,
      hideSelectExpItem: true,
      selectExpResStyle: selectExpResStyle,
      inventorySelectedItemId: -1
    };

    this.handleSelect = this.handleSelect.bind(this);
    this.handleExpItemSelect = this.handleExpItemSelect.bind(this);
    this.handleExpResSelect = this.handleExpResSelect.bind(this);

    this.handleSetExpItemClick = this.handleSetExpItemClick.bind(this);
    this.handleSetExpResourceClick = this.handleSetExpResourceClick.bind(this);
    this.handleExperimentClick = this.handleExperimentClick.bind(this);
    this.handleOkClick = this.handleOkClick.bind(this);
  }

  handleSelect(eventData) {
    this.setState({
      hideLeftSelect: false,
      hideSelectExpItem: true,
      inventorySelectedItemId: eventData.itemId,
    });
  }

  handleExpItemSelect(eventData) {
    console.log('handleExpItemSelect ' + JSON.stringify(eventData));

    Global.selectedItemOwnerId = eventData.ownerId;
    Global.selectedItemId = eventData.itemId;

    var redrawSelect = this.state.redrawSelect;

    this.setState({
      redrawSelect: !redrawSelect,
      inventorySelectedItemId: -1
    })
  }

  handleExpResSelect(eventData) {
    console.log('handleExpResSelect ' + JSON.stringify(eventData));

    Global.selectedItemOwnerId = eventData.ownerId;
    Global.selectedItemId = eventData.itemId;

    var redrawSelect = this.state.redrawSelect;

    this.setState({
      redrawSelect: !redrawSelect,
      inventorySelectedItemId: -1
    })
  }

  handleSetExpItemClick() {
    console.log('Set Experiment Item Click');
    Global.network.sendSetExpItem(this.props.expData.id, Global.selectedItemId);
  }

  handleSetExpResourceClick() {
    console.log('Set Experiment Item Click');
    Global.network.sendSetExpResource(this.props.expData.id, Global.selectedItemId);
  }

  handleExperimentClick() {
    Global.network.sendExperiment(this.props.expData.id);
  }

  handleOkClick() {
    Global.network.sendResetExperiment(this.props.expData.id);
  }

  render() {
    var showNewRecipe = this.props.expData.hasOwnProperty("recipe");

    const handleInventorySelect = (eventData) => {
      Global.selectedItemOwnerId = eventData.ownerId;
      Global.selectedItemId = eventData.itemId;
      Global.selectedItemName = eventData.itemName;
      this.handleSelect(eventData);
    };

    const headingStyle: React.CSSProperties = {
      color: '#c9aa71',
      fontFamily: 'Cinzel, Verdana, serif',
      fontSize: '15px',
      fontWeight: 'bold',
      lineHeight: 1.2,
    };

    const stateStyle: React.CSSProperties = {
      color: '#f2e7cf',
      fontSize: '12px',
      lineHeight: 1.35,
      marginTop: '8px',
    };

    const availableResources = (
      <MobileCard compact>
        <div style={headingStyle}>Available Resources</div>
        <div style={{ marginTop: '8px' }}>
          <MobileInventoryGrid
            ownerId={this.props.expData.id}
            items={this.props.expData.validresources || []}
            selectedItemId={this.state.inventorySelectedItemId}
            onSelect={handleInventorySelect}
            compact
          />
        </div>
      </MobileCard>
    );

    const sourceItemCard = (
      <MobileCard compact>
        <div style={headingStyle}>Source Item</div>
        <div style={{ marginTop: '8px' }}>
          <MobileInventoryGrid
            ownerId={this.props.expData.id}
            items={this.props.expData.expitem || []}
            selectedItemId={Global.selectedItemId}
            onSelect={this.handleExpItemSelect}
            emptyLabel="No source item set"
            compact
          />
        </div>
      </MobileCard>
    );

    const reagentCard = (
      <MobileCard compact>
        <div style={headingStyle}>Reagents</div>
        <div style={{ marginTop: '8px' }}>
          <MobileInventoryGrid
            ownerId={this.props.expData.id}
            items={this.props.expData.expresources || []}
            selectedItemId={Global.selectedItemId}
            onSelect={this.handleExpResSelect}
            emptyLabel="No reagents set"
            compact
          />
        </div>
        <div style={stateStyle}>{this.props.expData.expstate}</div>
      </MobileCard>
    );

    const experimentActions = (
      <MobilePanelActions
        compact
        actions={showNewRecipe ? [
          {
            key: 'ok',
            label: 'OK',
            icon: okbutton,
            onClick: this.handleOkClick,
          },
        ] : [
          {
            key: 'set-item',
            label: 'Set Item',
            icon: transferbutton,
            onClick: this.handleSetExpItemClick,
          },
          {
            key: 'set-reagent',
            label: 'Set Reagent',
            icon: transferbutton,
            onClick: this.handleSetExpResourceClick,
          },
          {
            key: 'experiment',
            label: 'Experiment',
            icon: experimentbutton,
            onClick: this.handleExperimentClick,
          },
        ]}
      />
    );

    return (
      <MobilePanelScreen panelType="experiment" title="Experiment">
        <MobileSplitPanelLayout
          left={
            <React.Fragment>
              {availableResources}
            </React.Fragment>
          }
          right={
            showNewRecipe ?
              <React.Fragment>
                <MobileSummaryCard
                  imageSrc={recipe}
                  title={this.props.expData.recipe.name}
                  subtitle="Eureka"
                  imageSize={48}
                />
                {experimentActions}
              </React.Fragment>
              :
              <React.Fragment>
                {sourceItemCard}
                {reagentCard}
                {experimentActions}
              </React.Fragment>
          }
        />
      </MobilePanelScreen>
    );
  }
}
