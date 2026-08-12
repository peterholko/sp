import * as React from "react";

interface DroppedBagExpiryProps {
  expiresIn?: number | null;
}

interface DroppedBagExpiryState {
  seconds: number | null;
}

export default class DroppedBagExpiry extends React.Component<
  DroppedBagExpiryProps,
  DroppedBagExpiryState
> {
  private timer?: ReturnType<typeof setInterval>;
  private deadlineMs: number | null;

  constructor(props: DroppedBagExpiryProps) {
    super(props);
    const seconds = this.normalizeSeconds(props.expiresIn);
    this.deadlineMs = seconds == null ? null : Date.now() + seconds * 1000;
    this.state = { seconds };
  }

  componentDidMount() {
    this.timer = setInterval(() => this.refresh(), 1000);
  }

  componentDidUpdate(prevProps: DroppedBagExpiryProps) {
    if (prevProps.expiresIn !== this.props.expiresIn && this.props.expiresIn != null) {
      const seconds = this.normalizeSeconds(this.props.expiresIn);
      this.deadlineMs = seconds == null ? null : Date.now() + seconds * 1000;
      this.setState({ seconds });
    }
  }

  componentWillUnmount() {
    if (this.timer) clearInterval(this.timer);
  }

  normalizeSeconds(value?: number | null): number | null {
    return typeof value == 'number' && Number.isFinite(value)
      ? Math.max(0, Math.ceil(value))
      : null;
  }

  refresh() {
    if (this.deadlineMs == null) return;
    const seconds = Math.max(0, Math.ceil((this.deadlineMs - Date.now()) / 1000));
    if (seconds != this.state.seconds) this.setState({ seconds });
  }

  render() {
    const seconds = this.state.seconds;
    if (seconds == null) return null;

    const minutes = Math.floor(seconds / 60);
    const remainder = seconds % 60;
    const time = minutes + ':' + remainder.toString().padStart(2, '0');
    const urgent = seconds <= 60;
    const style: React.CSSProperties = {
      color: urgent ? '#ff9b87' : '#e8c887',
      fontFamily: 'Verdana',
      fontSize: '11px',
      fontWeight: urgent ? 'bold' : 'normal',
      textAlign: 'center',
    };

    return (
      <div style={style} title="Adding more items does not reset this timer.">
        {seconds == 0 ? 'Abandoned bag expired' : 'Abandoned loot disappears in ' + time}
      </div>
    );
  }
}
