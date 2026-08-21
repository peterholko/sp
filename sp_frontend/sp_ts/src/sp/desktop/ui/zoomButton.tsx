import * as React from "react";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { desktopCameraZoom, desktopZoomControl } from "../../core/config";

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
    const control = desktopZoomControl(this.state.zoom);
    this.setState({ zoom: control.nextZoom });
    Global.gameEmitter.emit(GameEvent.CAMERA_ZOOM, {
      zoom: control.nextZoom,
      source: 'user',
    });
  };

  render() {
    const control = desktopZoomControl(this.state.zoom);
    const buttonStyle: React.CSSProperties = {
      position: 'fixed',
      bottom: '160px',
      left: '53px',
      width: '44px',
      height: '32px',
      backgroundColor: 'rgba(8, 10, 12, 0.82)',
      border: '1px solid rgba(201, 170, 113, 0.38)',
      borderRadius: '4px',
      color: '#c9aa71',
      fontFamily: 'Verdana',
      fontSize: '22px',
      fontWeight: 'bold',
      lineHeight: '1',
      cursor: 'pointer',
      zIndex: 50,
      pointerEvents: 'auto',
    };

    return (
      <button
        type="button"
        style={buttonStyle}
        onClick={this.handleClick}
        title={control.title}
        aria-label={control.title}
      >
        {control.label}
      </button>
    );
  }
}
