
import * as React from "react";
import transferbutton from "ui_comp/transferbutton.png";
import BaseInventoryPanel from "./baseInventoryPanel";
import { Global } from "../../core/global";
import HalfPanel from "./halfPanel";
import itemframe from "ui_comp/itemframe.png";
import refinebutton from "ui_comp/refinebutton.png";
import InventoryItem from "./inventoryItem";
import { EXP_RECIPE_NONE, TRIGGER_STRUCTURE_REFINING_ITEM } from "../../core/config";
import selectitemborder from "ui_comp/selectitemborder.png";
import wideframe from "ui_comp/wide_frame2.png";
import cancelbutton from "ui_comp/exitbutton.png";
import okbutton from "ui_comp/okbutton.png";
import { NetworkEvent } from "../../core/networkEvent";


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
        var inventoryItems = [];
        var inventoryOwner = -1;
        var itemToRefine;
        var showRefineItemPanel = this.state.progress > -1;

        var itemFrameResources = [];
        var producedItems = [];

        const windowHeight = window.innerHeight;
        const isLargeWindow = windowHeight > 700;

        const infoSmallY = '0px';
        const infoLargeY = '260px';

        const zIndex = Global.zIndexManager.getTop() + 1;

        const sourceStyle = {
            transform: 'translate(-323px, 25px)',
            position: 'fixed',
            textAlign: 'center',
            color: 'white',
            fontFamily: 'Verdana',
            fontSize: '12px',
            width: '323px'
        } as React.CSSProperties

        const refineItemStyle = {
            transform: 'translate(-185px, 50px)',
            position: 'fixed'
        } as React.CSSProperties

        const refineButtonStyle = {
            transform: 'translate(-210px, 290px)',
            position: 'fixed'
        } as React.CSSProperties

        const refineQueueButtonStyle = {
            transform: 'translate(-160px, 290px)',
            position: 'fixed'
        } as React.CSSProperties

        const refineItemNameStyle = {
            transform: 'translate(-323px, 100px)',
            position: 'fixed',
            textAlign: 'center',
            color: 'white',
            fontFamily: 'Verdana',
            fontSize: '12px',
            width: '323px'
        } as React.CSSProperties

        var selectStyle = {
            transform: 'translate(-185px, 50px)',
            position: 'fixed'
        } as React.CSSProperties

        const tableStyle = {
            transform: 'translate(45px, -150px)',
            position: 'fixed',
            color: 'white',
            fontFamily: 'Verdana',
            fontSize: '12px'
        } as React.CSSProperties

        const wideFrameStyle = {
            top: '50%',
            left: '50%',
            marginTop: isLargeWindow ? infoLargeY : infoSmallY,
            marginLeft: '-30px',
            position: 'fixed',
            transform: 'translate(-223px, -155px)',
            zIndex: zIndex
        } as React.CSSProperties

        const refiningItemNameStyle = {
            top: '50%',
            left: '50%',
            marginTop: '-25px',
            marginLeft: '0px',
            position: 'fixed',
            transform: 'translate(-150px, -95px)',
            zIndex: zIndex,
            textAlign: 'center',
            color: 'white',
            fontFamily: 'Verdana',
            fontSize: '12px',
            width: '300px'
        } as React.CSSProperties

        const refiningItemStyle = {
            top: '50%',
            left: '50%',
            marginTop: '-25px',
            marginLeft: '0px',
            position: 'fixed',
            transform: 'translate(-24px, -60px)',
            zIndex: zIndex,
        } as React.CSSProperties

        const cancelButtonStyle = {
            top: '50%',
            left: '50%',
            marginTop: '-25px',
            marginLeft: '0px',
            position: 'fixed',
            transform: 'translate(-24px, 95px)',
            zIndex: zIndex
        } as React.CSSProperties

        const refiningItemTableStyle = {
            top: '50%',
            left: '50%',
            marginTop: '-25px',
            marginLeft: '0px',
            position: 'fixed',
            textAlign: 'left',
            color: 'white',
            fontFamily: 'Verdana',
            fontSize: '12px',
            width: '200px',
            transform: 'translate(-135px, 0px)',
            zIndex: zIndex,
            userSelect: 'none'
        } as React.CSSProperties

        const refinedItemNameStyle = {
            top: '50%',
            left: '50%',
            marginTop: '-25px',
            marginLeft: '0px',
            position: 'fixed',
            transform: 'translate(-150px, -95px)',
            zIndex: zIndex,
            textAlign: 'center',
            color: 'white',
            fontFamily: 'Verdana',
            fontSize: '12px',
            width: '300px'
        } as React.CSSProperties

        const okButtonStyle = {
            transform: 'translate(225px, -75px)',
            position: 'fixed'
        } as React.CSSProperties

        var selectProduceStyle;
        var refineItemId = -1;
        var refineItemName = "";

        if (this.props.refineItemData && this.props.refineItemData.id != null) {

            var item = this.props.structureInventory.items.find(item => item.id == this.props.refineItemData.id) || this.props.structureInventory.items.find(item => item.id == this.props.refineItemData.id);

            refineItemId = item.id;
            refineItemName = item.name;

            var itemId = item.id;
            var itemName = item.name;
            var image = item.image;
            var quantity = item.quantity;

            var xPos = 138;
            var yPos = -310;

            itemToRefine = <InventoryItem key={itemId}
                ownerId={this.props.structureId}
                itemId={itemId}
                itemName={itemName}
                image={image}
                quantity={quantity}
                index={this.props.refineItemData.id}
                xPos={xPos}
                yPos={yPos}
                handleSelect={this.handleRefineItemSelect}
            />
        }

        for (var i = 0; i < 4; i++) {
            var xPos = i * 60 - 275;
            var yPos = 140;

            var itemFrameResource = {
                transform: 'translate(' + xPos + 'px, ' + yPos + 'px',
                position: 'fixed'
            } as React.CSSProperties

            itemFrameResources.push(
                <img src={itemframe} key={i} style={itemFrameResource} />
            )
        }

        if (this.props.refineItemData && this.props.refineItemData.produces && this.props.refineItemData.produces.length > 0) {
            for (var i = 0; i < this.props.refineItemData.produces.length; i++) {
                var xPos = i * 60 + 49;
                var yPos = -220;

                var itemId = this.props.refineItemData.produces[i].id;
                var itemName = this.props.refineItemData.produces[i].name;
                var image = this.props.refineItemData.produces[i].image;

                producedItems.push(
                    <InventoryItem key={i}
                        ownerId={this.props.structureId}
                        itemId={-1}
                        itemName={itemName}
                        image={image}
                        quantity={1}
                        index={i}
                        xPos={xPos}
                        yPos={yPos}
                        handleSelect={this.handleProduceItemSelect} />
                );

                if (itemName == this.state.selectedProduceItemName) {
                    var xPosProduce = -275 + (i * 60);
                    var yPosProduce = 140;

                    selectProduceStyle = {
                        transform: 'translate(' + xPosProduce + 'px, ' + yPosProduce + 'px)',
                        position: 'fixed'
                    } as React.CSSProperties
                }
            }
        }

        var disabledItems = [];

        for (var i = 0; i < inventoryItems.length; i++) {
            if (!inventoryItems[i].refineable) {
                disabledItems.push(inventoryItems[i].id);
            }
        }

        /*for (var i = 0; i < this.props.producedItemData.length; i++) {
            var xPos = i * 60 + 226 - ((this.props.producedItemData.length - 1) * 30);
            var yPos = -200;

            var itemId = this.props.producedItemData[i].id;
            var itemName = this.props.producedItemData[i].name;
            var image = this.props.producedItemData[i].image;
            var quantity = this.props.producedItemData[i].quantity;

            producedItems.push(
                <InventoryItem key={i}
                    ownerId={Global.heroId}
                    itemId={itemId}
                    itemName={itemName}
                    image={image}
                    quantity={quantity}
                    index={i}
                    xPos={xPos}
                    yPos={yPos}
                    handleSelect={this.handleProduceItemSelect} />
            );
        }*/

        const showProducedItems = producedItems.length > 0;

        return (
            <div>
                <BaseInventoryPanel left={true}
                    id={this.props.structureId}
                    items={this.props.structureInventory.items}
                    panelType={'structure_refine'}
                    hideExitButton={true}
                    hideSelect={false}
                    handleSelect={this.handleSelect}
                    selectedItemId={refineItemId}
                    disabledItems={disabledItems} />

                <HalfPanel left={false}
                    panelType={'structure_refine'}
                    hideExitButton={false}>

                    <span style={sourceStyle}>Refine Item</span>
                    <img src={itemframe} style={refineItemStyle} />
                    <span style={refineItemNameStyle}>{refineItemName}</span>

                    {itemToRefine}

                    {itemFrameResources}
                    {producedItems}

                    {this.props.refineItemData &&
                        <table style={tableStyle}>
                            <tbody>
                                <tr>
                                    <td>Refine Skill: </td>
                                    <td>{this.props.refineItemData.refining_skill}</td>
                                </tr>
                                <tr>
                                    <td>Refine Skill Level: </td>
                                    <td>{this.props.refineItemData.refining_skill_req}</td>
                                </tr>
                                <tr>
                                    <td>Refine Time: </td>
                                    <td>{this.props.refineItemData.refine_time} sec</td>
                                </tr>
                            </tbody>
                        </table>
                    }

                    <img src={refinebutton}
                        style={refineButtonStyle}
                        onClick={this.handleRefineClick} />

                    <img src={transferbutton}
                        style={refineQueueButtonStyle}
                        onClick={this.handleRefineQueueClick} />

                    {!this.state.hideSelectRefineItem &&
                        <img src={selectitemborder} style={selectStyle} />
                    }

                    {!this.state.hideSelectProduceItem &&
                        <img src={selectitemborder} style={selectProduceStyle} />
                    }

                </HalfPanel>

                {showRefineItemPanel &&
                    <div style={wideFrameStyle}>
                        <img src={wideframe} />
                        <div>

                            <span style={refiningItemNameStyle}>{this.props.refineItemData.name}</span>

                            <img src={'/static/art/items/' + this.props.refineItemData.image + '.png'} style={refiningItemStyle} />

                            <table style={refiningItemTableStyle}>
                                <tbody>
                                    <tr>
                                        <td>Refine Progress: </td>
                                        <td><progress max={this.state.maxProgress} value={this.state.progress}>{this.state.progress}</progress></td>
                                    </tr>
                                </tbody>
                            </table>
                            <img src={cancelbutton}
                                style={cancelButtonStyle}
                                onClick={this.handleCancelClick} />
                        </div>

                    </div>
                }
            </div>
        );
    }
}
