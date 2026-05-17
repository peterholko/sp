
import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import {
    MobileSplitPanelLayout,
    MobileStatsList,
    MobileSummaryCard,
    isLandscapeMobile,
} from "./mobilePanelLayout";

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
        const landscape = isLandscapeMobile();

        return (
            <MobilePanelScreen
                panelType={'wanteditempanel'}
                title={'Wanted Item'}
                hideExitButton={false}
                contentStyle={landscape ? { padding: '8px 0' } : undefined}>
                <MobileSplitPanelLayout
                    left={
                        <MobileSummaryCard
                            imageSrc={'/static/art/items/' + imageName}
                            title={itemName}
                            subtitle={`Quantity ${this.props.wantedItemData.quantity}`}
                            imageSize={48} />
                    }
                    right={
                        <MobileStatsList rows={[
                            { label: 'Class', value: this.props.wantedItemData.class, hidden: this.props.wantedItemData.class == null },
                            { label: 'Subclass', value: this.props.wantedItemData.subclass, hidden: this.props.wantedItemData.subclass == null },
                            { label: 'Name', value: this.props.wantedItemData.name, hidden: this.props.wantedItemData.name == null },
                            { label: 'Price/unit', value: this.props.wantedItemData.price },
                            { label: 'Buying Qty', value: this.props.wantedItemData.quantity },
                        ]} />
                    } />
            </MobilePanelScreen>
        )
    }
}
