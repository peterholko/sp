import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import { Global } from "../../core/global";
import leftbutton from "ui_comp/leftbutton.png";
import rightbutton from "ui_comp/rightbutton.png";
import hirebutton from "ui_comp/okbutton.png";
import { GameEvent } from "../../core/gameEvent";
import {
  MobilePanelActions,
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

interface MHPProps {
  hireData,
}

export default class MerchantHirePanel extends React.Component<MHPProps, any> {
  constructor(props) {
    super(props);

    this.state = {
      villager : this.props.hireData[0],
      index : 0
    };

    this.handleLeftClick = this.handleLeftClick.bind(this);
    this.handleRightClick = this.handleRightClick.bind(this);
    this.handleHireClick = this.handleHireClick.bind(this);
  }

  handleLeftClick(event) {
    if(this.state.index != 0) {
      const newIndex = this.state.index - 1;
      this.setState({villager: this.props.hireData[newIndex],
                     index: newIndex})
    } 
  }

  handleRightClick(event) {
    if(this.state.index != (this.props.hireData.length - 1)) {
      const newIndex = this.state.index + 1;
      this.setState({villager: this.props.hireData[newIndex],
                     index: newIndex})
    } 
  }

  handleHireClick() {
    Global.network.sendHire(Global.merchantSellTarget, this.state.villager.id);
    Global.gameEmitter.emit(GameEvent.MERCHANT_HIRE_CLICK, {});
  }

  render() {
    var imageName = this.state.villager.image.toLowerCase() + '_single.png';

    var topStats = [];

    topStats.push({'name': 'Creativity', 'value': this.state.villager.creativity});
    topStats.push({'name': 'Dexterity', 'value': this.state.villager.dexterity});
    topStats.push({'name': 'Endurance', 'value': this.state.villager.endurance});
    topStats.push({'name': 'Focus', 'value': this.state.villager.focus});
    topStats.push({'name': 'Intellect', 'value': this.state.villager.intellect});
    topStats.push({'name': 'Spirit', 'value': this.state.villager.spirit});
    topStats.push({'name': 'Strength', 'value': this.state.villager.strength});
    topStats.push({'name': 'Toughness', 'value': this.state.villager.toughness});

    topStats.sort((a, b) => (a.value < b.value) ? 1 : -1);

    var skillRows = [];

    for(var skill in this.state.villager.skills) {
      skillRows.push(`${skill} ${this.state.villager.skills[skill]}`);
    }
    const landscape = isLandscapeMobile();
    const atFirst = this.state.index == 0;
    const atLast = this.state.index == (this.props.hireData.length - 1);

    return (
      <MobilePanelScreen
        panelType={'hire'}
        title={'Hire'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={<MobileSummaryCard imageSrc={'/static/art/' + imageName} title={this.state.villager.name} subtitle={`Wage ${this.state.villager.wage}`} imageSize={landscape ? 58 : 82} />}
          right={
            <>
              <MobileStatsList rows={[
                { label: 'Skills', value: skillRows.join(', ') },
                { label: 'Top Stats', value: `${topStats[0].name} (${topStats[0].value}), ${topStats[1].name} (${topStats[1].value}), ${topStats[2].name} (${topStats[2].value})` },
              ]} />
              <MobilePanelActions actions={[
                { key: 'previous', label: 'Previous hire', icon: leftbutton, onClick: this.handleLeftClick, disabled: atFirst },
                { key: 'hire', label: 'Hire', icon: hirebutton, onClick: this.handleHireClick },
                { key: 'next', label: 'Next hire', icon: rightbutton, onClick: this.handleRightClick, disabled: atLast },
              ]} />
            </>
          } />
      </MobilePanelScreen>
    );
  }
}


