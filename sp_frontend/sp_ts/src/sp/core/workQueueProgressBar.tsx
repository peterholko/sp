import * as React from 'react';
import { ActionProgressSnapshot, AnchoredActionProgress } from './actionProgress';
import {
  reconcileWorkQueueProgress,
  workQueueProgressFraction,
} from './workQueueProgress';

interface WorkQueueProgressBarProps extends ActionProgressSnapshot {
  style?: React.CSSProperties;
  className?: string;
  label?: string;
}

interface WorkQueueProgressBarState {
  progress: AnchoredActionProgress | null;
  nowMs: number;
}

function monotonicNow(): number {
  return typeof performance !== 'undefined' ? performance.now() : Date.now();
}

/**
 * Smooth client-side presentation of a server-authoritative queue action.
 * The server supplies both the cycle identity and its elapsed time; animation
 * between snapshots is derived from the browser's monotonic clock.
 */
export default class WorkQueueProgressBar extends React.PureComponent<
  WorkQueueProgressBarProps,
  WorkQueueProgressBarState
> {
  private animationFrame: number | null = null;

  constructor(props: WorkQueueProgressBarProps) {
    super(props);
    const nowMs = monotonicNow();
    this.state = {
      progress: reconcileWorkQueueProgress(null, props, nowMs),
      nowMs,
    };
  }

  componentDidMount() {
    this.scheduleAnimation();
  }

  componentDidUpdate(prevProps: WorkQueueProgressBarProps) {
    const timingChanged =
      prevProps.action_id !== this.props.action_id ||
      prevProps.action_duration_ms !== this.props.action_duration_ms ||
      prevProps.action_elapsed_ms !== this.props.action_elapsed_ms;

    if (!timingChanged) {
      return;
    }

    const nowMs = monotonicNow();
    const progress = reconcileWorkQueueProgress(this.state.progress, this.props, nowMs);
    this.cancelAnimation();
    this.setState({ progress, nowMs }, () => this.scheduleAnimation());
  }

  componentWillUnmount() {
    this.cancelAnimation();
  }

  private scheduleAnimation = () => {
    if (this.animationFrame != null || !this.state.progress) {
      return;
    }

    if (workQueueProgressFraction(this.state.progress, this.state.nowMs) >= 1) {
      return;
    }

    this.animationFrame = window.requestAnimationFrame(this.animate);
  };

  private cancelAnimation = () => {
    if (this.animationFrame == null) {
      return;
    }

    window.cancelAnimationFrame(this.animationFrame);
    this.animationFrame = null;
  };

  private animate = (nowMs: number) => {
    this.animationFrame = null;
    this.setState({ nowMs }, () => this.scheduleAnimation());
  };

  render() {
    if (!this.state.progress) {
      return null;
    }

    const fraction = workQueueProgressFraction(this.state.progress, this.state.nowMs);
    return (
      <progress
        className={this.props.className}
        max={1}
        value={fraction}
        style={this.props.style}
        aria-label={this.props.label || 'Work progress'}>
        {Math.round(fraction * 100)}%
      </progress>
    );
  }
}
