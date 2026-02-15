
import * as React from "react";
import { Global } from "../global";
import cancelbutton from "ui_comp/exitbutton.png";
import wideframe from "ui_comp/wide_frame2.png";
import { NetworkEvent } from "../networkEvent";
import InventoryItem from "./inventoryItem";
import okbutton from "ui_comp/okbutton.png";
import { TRIGGER_REFINING_ITEM } from "../config";
import { GameEvent } from "../gameEvent";

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
        var producedItems = [];

        for (var i = 0; i < this.props.producedItemData.length; i++) {
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
        }

        const showProducedItems = producedItems.length > 0;

        const windowHeight = window.innerHeight;
        const isLargeWindow = windowHeight > 700;

        const transferSmallY = '110px';
        const transferLargeY = '370px';

        const infoSmallY = '0px';
        const infoLargeY = '260px';

        const zIndex = Global.zIndexManager.getTop() + 1;

        const wideFrameStyle = {
            top: '50%',
            left: '50%',
            marginTop: isLargeWindow ? infoLargeY : infoSmallY,
            marginLeft: '-30px',
            position: 'fixed',
            transform: 'translate(-223px, -155px)',
            zIndex: zIndex
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

        const refineItemNameStyle = {
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

        const refineItemStyle = {
            top: '50%',
            left: '50%',
            marginTop: '-25px',
            marginLeft: '0px',
            position: 'fixed',
            transform: 'translate(-24px, -60px)',
            zIndex: zIndex,
        } as React.CSSProperties

        const refineItemTableStyle = {
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

        const arrowStyle = {
            top: '50%',
            left: '50%',
            marginTop: '-25px',
            marginLeft: '-25px',
            position: 'fixed',
            transform: 'translate(0px, -60px)',
            zIndex: zIndex,
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

        return (
            <div style={wideFrameStyle}>
                <img src={wideframe} />
                {!showProducedItems &&
                    <div>

                        <span style={refineItemNameStyle}>{this.props.refineItemData.name}</span>

                        <img src={'/static/art/items/' + this.props.refineItemData.image + '.png'} style={refineItemStyle} />

                        <table style={refineItemTableStyle}>
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
                }

                {showProducedItems &&
                    <div>
                        <span style={refinedItemNameStyle}>Refined Items</span>
                        {producedItems}
                        <img src={okbutton} style={okButtonStyle} onClick={this.handleOkClick}/>
                    </div>
                }
            </div>
        );
    }
}
