import * as React from "react";
import MobilePanelScreen from "./mobilePanelScreen";
import {
  MobileSplitPanelLayout,
  MobileStatsList,
  MobileSummaryCard,
  isLandscapeMobile,
} from "./mobilePanelLayout";
import WorkQueueProgressBar from "../../core/workQueueProgressBar";

interface WorkQueueEntryPanelProps {
  workQueueEntryData,
}

export default class WorkQueueEntryPanel extends React.Component<WorkQueueEntryPanelProps, any> {
  render() {
    const landscape = isLandscapeMobile();
    const progress = <WorkQueueProgressBar
      action_id={this.props.workQueueEntryData.action_id}
      action_duration_ms={this.props.workQueueEntryData.action_duration_ms}
      action_elapsed_ms={this.props.workQueueEntryData.action_elapsed_ms}
      label={this.props.workQueueEntryData.item_name + ' progress'} />;

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

