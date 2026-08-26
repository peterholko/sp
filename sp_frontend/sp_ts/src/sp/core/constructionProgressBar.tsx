import * as React from 'react';
import {
  ConstructionProgressSample,
  constructionProgressTimeline,
} from './constructionProgress';

interface ConstructionProgressBarProps {
  structureId: string | number;
  fallbackWorkDone?: number;
  fallbackTotalWork?: number;
}

interface ConstructionProgressBarState {
  sample: ConstructionProgressSample | null;
}

export default class ConstructionProgressBar extends React.PureComponent<
  ConstructionProgressBarProps,
  ConstructionProgressBarState
> {
  private unsubscribe: (() => void) | null = null;

  constructor(props: ConstructionProgressBarProps) {
    super(props);
    this.state = {
      sample: constructionProgressTimeline.sample(props.structureId),
    };
  }

  componentDidMount(): void {
    this.subscribe(this.props.structureId);
  }

  componentDidUpdate(previousProps: ConstructionProgressBarProps): void {
    if (previousProps.structureId != this.props.structureId) {
      this.unsubscribe?.();
      this.subscribe(this.props.structureId);
    }
  }

  componentWillUnmount(): void {
    this.unsubscribe?.();
  }

  private subscribe(structureId: string | number): void {
    this.unsubscribe = constructionProgressTimeline.subscribe(structureId, sample => {
      this.setState({ sample });
    });
  }

  render(): React.ReactNode {
    const totalWork = this.state.sample?.totalWork ?? this.props.fallbackTotalWork ?? 1;
    const workDone = this.state.sample?.workDone ?? this.props.fallbackWorkDone ?? 0;
    const safeTotal = Math.max(1, totalWork);
    const safeWorkDone = Math.max(0, Math.min(safeTotal, workDone));

    return <progress max={safeTotal} value={safeWorkDone}>{safeWorkDone}</progress>;
  }
}
