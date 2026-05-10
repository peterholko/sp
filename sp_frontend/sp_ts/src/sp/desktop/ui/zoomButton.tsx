import * as React from "react";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { desktopCameraZoom } from "../../core/config";

interface State {
  zoom: number;
}

export default class ZoomButton extends React.Component<{}, State> {
  state: State = { zoom: desktopCameraZoom() };

  componentDidMount() {
    Global.gameEmitter.on(GameEvent.CAMERA_ZOOM, this.handleZoomEvent, this);
  }

  componentWillUnmount() {
    Global.gameEmitter.off(GameEvent.CAMERA_ZOOM, this.handleZoomEvent, this);
  }

  handleZoomEvent = (data) => {
    if (data && typeof data.zoom === 'number' && data.zoom !== this.state.zoom) {
      this.setState({ zoom: data.zoom });
    }
  };

  handleClick = () => {
    const next = this.state.zoom >= 2 ? 1 : 2;
    this.setState({ zoom: next });
    Global.gameEmitter.emit(GameEvent.CAMERA_ZOOM, { zoom: next });
  };

  render() {
    const buttonStyle: React.CSSProperties = {
      position: 'fixed',
      top: '24px',
      left: '270px',
      width: '44px',
      height: '32px',
      backgroundColor: 'rgba(8, 10, 12, 0.82)',
      border: '1px solid rgba(201, 170, 113, 0.38)',
      borderRadius: '4px',
      color: '#c9aa71',
      fontFamily: 'Verdana',
      fontSize: '12px',
      fontWeight: 'bold',
      cursor: 'pointer',
      zIndex: 50,
      pointerEvents: 'auto',
    };

    return (
      <button type="button" style={buttonStyle} onClick={this.handleClick} title="Toggle zoom">
        {this.state.zoom >= 2 ? '1×' : '2×'}
      </button>
    );
  }
}
