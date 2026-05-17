
import * as React from "react";
import transferbutton from "ui_comp/transferbutton.png";
import BaseInventoryPanel from "./baseInventoryPanel";
import { Global } from "../../core/global";
import itemframe from "ui_comp/itemframe.png";
import refinebutton from "ui_comp/refinebutton.png";
import addqueuebutton from "ui_comp/addqueuebutton.png";
import InventoryItem from "./inventoryItem";
import { EXP_RECIPE_NONE, TRIGGER_STRUCTURE_REFINING_ITEM } from "../../core/config";
import selectitemborder from "ui_comp/selectitemborder.png";
import wideframe from "ui_comp/wide_frame2.png";
import cancelbutton from "ui_comp/exitbutton.png";
import okbutton from "ui_comp/okbutton.png";
import { NetworkEvent } from "../../core/networkEvent";
import MobilePanelScreen from "./mobilePanelScreen";
import MobileInventoryGrid from "./mobileInventoryGrid";
import {
    MobileCard,
    MobilePanelActions,
    MobileSplitPanelLayout,
    MobileStatsList,
    MobileSummaryCard,
} from "./mobilePanelLayout";


interface StructureRefinePanelProps {
    structureId: number,
    structureInventory: any,
    refineItemData: any,
    producedItemData: any,
    infoRefineItemTriggered: boolean,
}

export default class StructureRefinePanel extends React.Component<StructureRefinePanelProps, any> {

    private timer;

    constructor(props) {
        super(props);

        Global.selectedItemId = -1;
        Global.selectedItemOwnerId = -1;

        var maxProgress = -1;
        var progress = -1;

        if (this.props.refineItemData && this.props.refineItemData.progress) {
            maxProgress = this.props.refineItemData.refine_time;
            progress = this.props.refineItemData.progress;
        }

        this.state = {
            refinerItems: props.refinerItems,
            structureItems: props.structureItems,
            showRefinerItems: false,
            hideLeftSelect: true,
            hideSelectRefineItem: true,
            hideSelectProduceItem: true,
            selectedProduceItemName: "",
            progress: progress,
            maxProgress: maxProgress,
        };

        this.handleSelect = this.handleSelect.bind(this);
        this.handleRefineClick = this.handleRefineClick.bind(this);
        this.handleRefineQueueClick = this.handleRefineQueueClick.bind(this);
        this.handleRefineItemSelect = this.handleRefineItemSelect.bind(this);
        this.handleProduceItemSelect = this.handleProduceItemSelect.bind(this);
        this.handleCancelClick = this.handleCancelClick.bind(this);

        Global.gameEmitter.on(NetworkEvent.REFINE, this.handleRefine, this);
        Global.gameEmitter.on(NetworkEvent.INFO_STRUCTURE_REFINE, this.handleInfoStructureRefine, this);
    }

    componentDidMount() {
        if (this.state.progress > -1) {
            this.startTimer();
        }
    }

    componentWillUnmount() {
        console.log('******* componentWillUnmount refine panel');
        if (this.timer) {
            console.log('Stop Timer Refine Panel');
            clearInterval(this.timer);
            this.timer = null;
        }
        Global.gameEmitter.removeListener(NetworkEvent.REFINE, this.handleRefine);
        Global.gameEmitter.removeListener(NetworkEvent.INFO_STRUCTURE_REFINE, this.handleInfoStructureRefine);
    }

    handleRefine(eventData) {
        console.log('handleRefine ' + JSON.stringify(eventData));
        this.setState({
            maxProgress: eventData.refine_time,
        });

        this.startTimer();
    }

    handleCancelClick() {
        Global.network.sendCancelAction();

        this.setState({
            progress: -1,
            maxProgress: -1,
        });

        this.stopTimer();
    }

    handleInfoStructureRefine(message) {

        if (message.refine_time && message.refine_time.progress == 0) {
            this.setState({
                progress: 0,
            });
        }

        if (message.refine_time == null) {
            this.stopTimer();

            this.setState({
                progress: -1,
                maxProgress: -1,
            });
        }
    }

    handleSelect(eventData) {
        console.log('handleSelect ' + JSON.stringify(eventData));
        this.setState({
            hideLeftSelect: false,
            hideSelectRefineItem: true,
        });

        Global.network.sendInfoStructureRefineItem(this.props.structureId, eventData.itemId);
    }

    handleRefineItemSelect(eventData) {
        console.log('handleRefineItemSelect ' + JSON.stringify(eventData));

        this.setState({
            hideSelectRefineItem: false,
            hideSelectProduceItem: true,
            hideLeftSelect: true,
        });

        Global.network.sendInfoItem(this.props.structureId, eventData.itemId, "refine");
    }

    handleProduceItemSelect(eventData) {
        console.log('handleProduceItemSelect ' + JSON.stringify(eventData));

        this.setState({
            hideSelectProduceItem: false,
            hideSelectRefineItem: true,
            hideLeftSelect: true,
            selectedProduceItemName: eventData.itemName
        })

        Global.infoItemAction = TRIGGER_STRUCTURE_REFINING_ITEM;
        Global.network.sendInfoItemByName(eventData.itemName);
    }

    handleRefineClick() {
        //Global.isStructureRefining = true;
        Global.network.sendStructureRefine(this.props.structureId, this.props.refineItemData.id);
        //Global.gameEmitter.emit(GameEvent.REFINE_CLICK);
    }

    handleRefineQueueClick() {
        Global.network.sendAddRefineEntry(this.props.structureId, this.props.refineItemData.id);
    }

    startTimer() {
        this.timer = setInterval(() => {

            if (this.state.progress >= this.state.maxProgress) {
                this.stopTimer();
            } else {
                this.setState({ progress: this.state.progress + 1 });
            }
        }, 1000);
    }

    stopTimer() {
        clearInterval(this.timer)
        this.timer = null;
    }

    render() {
        var showRefineItemPanel = this.state.progress > -1;
        var refineItemId = -1;
        var refineItemName = "";

        if (this.props.refineItemData && this.props.refineItemData.id != null) {
            const item = (this.props.structureInventory.items || []).find(item => item.id == this.props.refineItemData.id);
            if (item) {
                refineItemId = item.id;
                refineItemName = item.name;
            }
        }

        const mobileDisabledItems = (this.props.structureInventory.items || [])
            .filter((item) => !item.refineable)
            .map((item) => item.id);

        const producedGridItems = this.props.refineItemData && this.props.refineItemData.produces
            ? this.props.refineItemData.produces.map((item) => ({
                id: item.name,
                name: item.name,
                image: item.image,
                quantity: 1,
            }))
            : [];

        const handleMobileSelect = (eventData) => {
            Global.selectedItemOwnerId = eventData.ownerId;
            Global.selectedItemId = eventData.itemId;
            Global.selectedItemName = eventData.itemName;
            this.handleSelect(eventData);
        };

        const handleMobileProduceSelect = (eventData) => {
            this.handleProduceItemSelect(eventData);
        };

        const headingStyle: React.CSSProperties = {
            color: '#c9aa71',
            fontFamily: 'Cinzel, Verdana, serif',
            fontSize: '15px',
            fontWeight: 'bold',
            lineHeight: 1.2,
        };

        const itemHeaderStyle: React.CSSProperties = {
            display: 'flex',
            alignItems: 'center',
            gap: '10px',
            marginTop: '8px',
        };

        const itemImageStyle: React.CSSProperties = {
            width: '48px',
            height: '48px',
            objectFit: 'contain',
            imageRendering: 'pixelated',
            flex: '0 0 auto',
        };

        const refineStats = this.props.refineItemData ? [
            { label: 'Refine Skill', value: this.props.refineItemData.refining_skill },
            { label: 'Skill Level', value: this.props.refineItemData.refining_skill_req },
            { label: 'Refine Time', value: `${this.props.refineItemData.refine_time} sec` },
        ] : [];

        const progressCard = showRefineItemPanel && this.props.refineItemData ? (
            <MobileCard compact>
                <div style={itemHeaderStyle}>
                    <img src={'/static/art/items/' + this.props.refineItemData.image + '.png'} style={itemImageStyle} />
                    <div style={headingStyle}>{this.props.refineItemData.name}</div>
                </div>
                <progress style={{ width: '100%', marginTop: '10px' }} max={this.state.maxProgress} value={this.state.progress}>{this.state.progress}</progress>
                <MobilePanelActions
                    compact
                    actions={[{
                        key: 'cancel',
                        label: 'Cancel',
                        icon: cancelbutton,
                        onClick: this.handleCancelClick,
                    }]}
                />
            </MobileCard>
        ) : null;

        const inventoryCard = (
            <MobileCard compact>
                <div style={headingStyle}>Structure Inventory</div>
                <div style={{ marginTop: '8px' }}>
                    <MobileInventoryGrid
                        ownerId={this.props.structureId}
                        items={this.props.structureInventory.items || []}
                        selectedItemId={refineItemId}
                        disabledItems={mobileDisabledItems}
                        onSelect={handleMobileSelect}
                        compact
                    />
                </div>
            </MobileCard>
        );

        const refineDetail = this.props.refineItemData ? (
            <React.Fragment>
                <MobileSummaryCard
                    imageSrc={'/static/art/items/' + this.props.refineItemData.image + '.png'}
                    title={refineItemName || this.props.refineItemData.name}
                    subtitle="Selected refine item"
                />
                <MobileStatsList rows={refineStats} compact />
            </React.Fragment>
        ) : (
            <MobileCard compact>
                <div style={headingStyle}>Refine Item</div>
                <div style={{ color: '#777d82', fontSize: '12px', marginTop: '8px' }}>Select a refineable item.</div>
            </MobileCard>
        );

        const produceCard = (
            <MobileCard compact>
                <div style={headingStyle}>Produces</div>
                <div style={{ marginTop: '8px' }}>
                    <MobileInventoryGrid
                        ownerId={this.props.structureId}
                        items={producedGridItems}
                        selectedItemId={this.state.selectedProduceItemName}
                        onSelect={handleMobileProduceSelect}
                        emptyLabel="No output selected"
                        compact
                    />
                </div>
            </MobileCard>
        );

        const refineActions = (
            <MobilePanelActions
                compact
                actions={[
                    {
                        key: 'refine',
                        label: 'Refine',
                        icon: refinebutton,
                        onClick: this.handleRefineClick,
                        disabled: !this.props.refineItemData,
                    },
                    {
                        key: 'queue',
                        label: 'Queue',
                        icon: addqueuebutton,
                        onClick: this.handleRefineQueueClick,
                        disabled: !this.props.refineItemData,
                    },
                ]}
            />
        );

        return (
            <MobilePanelScreen panelType="structure_refine" title="Structure Refine">
                <MobileSplitPanelLayout
                    left={
                        <React.Fragment>
                            {progressCard}
                            {inventoryCard}
                        </React.Fragment>
                    }
                    right={
                        <React.Fragment>
                            {refineDetail}
                            {produceCard}
                            {refineActions}
                        </React.Fragment>
                    }
                />
            </MobilePanelScreen>
        );
    }
}
