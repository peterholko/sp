import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import {
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";

interface WorkQueueEntryPanelProps {
  workQueueEntryData,
}

export default class WorkQueueEntryPanel extends React.Component<WorkQueueEntryPanelProps, any> {
  private timer;

  constructor(props) {
    super(props);

    this.state = {
      maxProgress: this.props.workQueueEntryData.work_time,
      progress: this.props.workQueueEntryData.progress,
    };

    this.startTimer = this.startTimer.bind(this)
    this.stopTimer = this.stopTimer.bind(this)
  }

  componentDidMount() {
    this.startTimer();
  }

  componentWillUnmount() {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = null;
    }
  }

  startTimer() {
    console.log('Start Timer Work Queue Entry Panel');
    this.timer = setInterval(() => {
      console.log("progress: " + this.state.progress);
      console.log("maxProgress: " + this.state.maxProgress);

      if (this.state.progress >= this.state.maxProgress) {
        console.log('progress >>> maxProgress');
        this.stopTimer();
      } else {
        this.setState({ progress: this.state.progress + 1 });
      }
    }, 1000);
  }

  stopTimer() {
    console.log('Stop Timer Work Queue Entry Panel');
    clearInterval(this.timer)
    this.timer = null;
  }

  render() {
    const landscape = isLandscapeMobile();
    const progress = <progress max={this.state.maxProgress} value={this.state.progress}>{this.state.progress}</progress>;

    return (
      <MobilePanelScreen
        panelType={'workqueueentry'}
        title={'Work Entry'}
        hideExitButton={false}
        contentStyle={landscape ? { padding: '8px 0' } : undefined}>
        <MobileSplitPanelLayout
          left={
            <MobileSummaryCard
              imageSrc={'/static/art/items/' + this.props.workQueueEntryData.item_image + '.png'}
              title={this.props.workQueueEntryData.item_name}
              subtitle={this.props.workQueueEntryData.work_type}
              imageSize={48} />
          }
          right={
            <MobileStatsList rows={[
              { label: 'Progress', value: progress },
            ]} />
          } />
      </MobilePanelScreen>
    );
  }
}


