
import * as React from "react";
import { Global } from "../../core/global";
import cancelbutton from "ui_comp/exitbutton.png";
import wideframe from "ui_comp/wide_frame2.png";
import { NetworkEvent } from "../../core/networkEvent";
import InventoryItem from "./inventoryItem";
import okbutton from "ui_comp/okbutton.png";
import { TRIGGER_REFINING_ITEM } from "../../core/config";
import { GameEvent } from "../../core/gameEvent";
import MobilePanelScreen from "./mobilePanelScreen";
import MobileInventoryGrid from "./mobileInventoryGrid";
import {
    MobileCard,
    MobilePanelActions,
    MobileSplitPanelLayout,
    MobileStatsList,
    MobileSummaryCard,
} from "./mobilePanelLayout";

interface RefinePanelProps {
    refineItemData: any,
    refineTime: number,
    producedItemData: any
}

export default class RefinePanel extends React.Component<RefinePanelProps, any> {
    private timer;

    constructor(props) {
        super(props);

        this.state = {
            progress: 0,
            maxProgress: this.props.refineTime,
        };

        this.startTimer = this.startTimer.bind(this)
        this.stopTimer = this.stopTimer.bind(this)

        this.handleProduceItemSelect = this.handleProduceItemSelect.bind(this);
        this.handleCancelClick = this.handleCancelClick.bind(this)
        this.handleOkClick = this.handleOkClick.bind(this)

        Global.gameEmitter.on(NetworkEvent.INFO_REFINE, this.handleInfoRefine, this);
        Global.gameEmitter.on(NetworkEvent.REFINE, this.handleRefine, this);
    }

    componentDidMount() {
        console.log('componentDidMount ' + JSON.stringify(this.props.refineItemData));
        this.startTimer();
    }

    componentWillUnmount() {
        this.stopTimer();
        Global.gameEmitter.removeListener(NetworkEvent.INFO_REFINE, this.handleInfoRefine);
        Global.gameEmitter.removeListener(NetworkEvent.REFINE, this.handleRefine);
    }

    handleProduceItemSelect(eventData) {
        console.log('handleProduceItemSelect ' + JSON.stringify(eventData));
        Global.infoItemAction = TRIGGER_REFINING_ITEM;
        Global.network.sendInfoItem(eventData.itemId, "None");
    }

    handleCancelClick() {
        Global.network.sendCancelAction();
        Global.gameEmitter.emit(GameEvent.CANCEL_REFINE_CLICK);

        this.setState({
            progress: 0,
        });

        this.stopTimer();
    }

    handleInfoRefine(message) {
        console.log('handleInfoRefine ' + JSON.stringify(message));

        /*if (message.refining_item && message.refining_item.progress == 0) {
            this.setState({
                progress: 0,
            });
        }

        if (message.produced_items.length > 0 && message.refining_item == null) {
            // No more items to refine
            this.stopTimer();

            this.setState({
                progress: -1,
            });
        }*/
    }

    handleRefine(eventData) {
        console.log('handleRefine ' + JSON.stringify(eventData));
        this.setState({
          maxProgress: eventData.refine_time,
        });
    
        this.startTimer();
      }

    handleOkClick() {
        Global.gameEmitter.emit(GameEvent.REFINE_OK_CLICK, {});
    }

    startTimer() {
        this.timer = setInterval(() => {
            console.log("progress: " + this.state.progress);
            console.log("maxProgress: " + this.state.maxProgress);

            if (this.state.progress >= this.state.maxProgress) {
                console.log('progress >>> maxProgress');
            } else {
                this.setState({ progress: this.state.progress + 1 });
            }
        }, 1000);
    }

    stopTimer() {
        clearInterval(this.timer)
    }

    render() {
        const producedItems = this.props.producedItemData || [];
        const showProducedItems = producedItems.length > 0;

        const progressCard = (
            <MobileCard compact>
                <div style={{ color: '#c9aa71', fontFamily: 'Verdana', fontSize: '11px', fontWeight: 'bold', textTransform: 'uppercase', marginBottom: '7px' }}>
                    Progress
                </div>
                <progress style={{ width: '100%' }} max={this.state.maxProgress} value={this.state.progress}>{this.state.progress}</progress>
                <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '4px 10px', color: '#f2e7cf', fontSize: '11px', marginTop: '8px' }}>
                    <span style={{ color: '#c9aa71' }}>Elapsed</span><span>{this.state.progress}</span>
                    <span style={{ color: '#c9aa71' }}>Total</span><span>{this.state.maxProgress}</span>
                </div>
            </MobileCard>
        );

        const producedCard = (
            <MobileCard compact>
                <div style={{ color: '#c9aa71', fontFamily: 'Verdana', fontSize: '11px', fontWeight: 'bold', textTransform: 'uppercase', marginBottom: '7px' }}>
                    Refined Items
                </div>
                <MobileInventoryGrid
                    ownerId={Number(Global.heroId)}
                    items={producedItems}
                    onSelect={this.handleProduceItemSelect}
                    emptyLabel="No refined items"
                    compact
                />
            </MobileCard>
        );

        const actions = (
            <MobilePanelActions
                compact
                actions={showProducedItems ? [
                    {
                        key: 'ok',
                        label: 'OK',
                        icon: okbutton,
                        onClick: this.handleOkClick,
                    },
                ] : [
                    {
                        key: 'cancel',
                        label: 'Cancel',
                        icon: cancelbutton,
                        onClick: this.handleCancelClick,
                    },
                ]}
            />
        );

        return (
            <MobilePanelScreen panelType="refine" title="Refine" hideExitButton>
                <MobileSplitPanelLayout
                    left={
                        showProducedItems ?
                            producedCard
                            :
                            <React.Fragment>
                                <MobileSummaryCard
                                    imageSrc={'/static/art/items/' + this.props.refineItemData.image + '.png'}
                                    title={this.props.refineItemData.name}
                                    subtitle="Refining"
                                />
                                {progressCard}
                            </React.Fragment>
                    }
                    right={
                        <React.Fragment>
                            {showProducedItems ?
                                <MobileSummaryCard
                                    title="Refine Complete"
                                    subtitle={`${producedItems.length} item${producedItems.length == 1 ? '' : 's'} produced`}
                                />
                                :
                                <MobileCard compact>
                                    <div style={{ color: '#f2e7cf', fontSize: '12px', lineHeight: 1.35 }}>
                                        Refining is in progress. Cancel stops the current action.
                                    </div>
                                </MobileCard>
                            }
                            {actions}
                        </React.Fragment>
                    }
                />
            </MobilePanelScreen>
        );
    }
}
