
import * as React from "react";
import HalfPanel from "./halfPanel";

interface WantedItemPanelProps {
    wantedItemData,
}

export default class WantedItemPanel extends React.Component<WantedItemPanelProps, any> {
    constructor(props) {
        super(props);

        this.state = {
        };
    }

    render() {
        console.log("WantedItemPanel render");
        console.log(this.props.wantedItemData);
        const itemName = this.props.wantedItemData.itemName;
        const imageName = itemName.replace(/\s/g, '').toLowerCase() + '.png';

        const itemStyle = {
            transform: 'translate(-185px, 25px)',
            position: 'fixed'
        } as React.CSSProperties

        const spanNameStyle = {
            transform: 'translate(-323px, 85px)',
            position: 'fixed',
            textAlign: 'center',
            color: 'white',
            fontFamily: 'Verdana',
            fontSize: '12px',
            width: '323px'
        } as React.CSSProperties

        const tableStyle = {
            transform: 'translate(20px, -250px)',
            position: 'fixed',
            color: 'white',
            fontFamily: 'Verdana',
            fontSize: '12px',
            width: '300px'
        } as React.CSSProperties

        return (
            <HalfPanel left={true}
                panelType={'wanteditempanel'}
                hideExitButton={false}>
                <img src={'/static/art/items/' + imageName} style={itemStyle} />
                <span style={spanNameStyle}>
                    {itemName} x {this.props.wantedItemData.quantity}
                </span>
                <table style={tableStyle}>
                    <tbody>
                        {(this.props.wantedItemData.class != null) &&
                            <tr>
                                <td>Class: </td>
                                <td>{this.props.wantedItemData.class}</td>
                            </tr>}

                        {(this.props.wantedItemData.subclass != null) &&
                            <tr>
                                <td>Subclass: </td>
                                <td>{this.props.wantedItemData.subclass}</td>
                            </tr>}

                        {(this.props.wantedItemData.name != null) &&
                            <tr>
                                <td>Name: </td>
                                <td>{this.props.wantedItemData.name}</td>
                            </tr>}

                        <tr>
                            <td>Price per unit: </td>
                            <td>{this.props.wantedItemData.price}</td>
                        </tr>

                        <tr>
                            <td>Buying Quantity: </td>
                            <td>{this.props.wantedItemData.quantity}</td>
                        </tr>

                    </tbody>
                </table>

            </HalfPanel>
        )
    }
}