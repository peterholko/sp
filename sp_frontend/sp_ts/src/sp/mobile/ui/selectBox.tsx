import * as React from "react";
import selectbox from "ui_comp/selectbox.png";
import selectboxborder from "ui_comp/selectboxborder.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { DEAD } from "../../core/config";

interface SelectedKey {
  type: string,
  id?: integer,
  x?: integer,
  y?: integer,
}

interface SelectBoxProps {
  pos: integer,
  selectedKey: SelectedKey,
  imageName: string,
  style: React.CSSProperties,
  imageStyle?: React.CSSProperties,
  showBorder: boolean,
  showGravestone: boolean
}

interface SpriteFrame {
  imageName: string,
  frameIndex: number,
  width: number,
  height: number
}

export default class SelectBox extends React.Component<SelectBoxProps, any> {
  private canvasRef: React.RefObject<HTMLCanvasElement>;
  private drawRequestId: number;
  private lastDrawFrameKey: string;

  constructor(props) {
    super(props);

    this.handleClick = this.handleClick.bind(this);
    this.canvasRef = React.createRef();
    this.drawRequestId = 0;
    this.lastDrawFrameKey = '';
  }

  componentDidMount() {
    this.drawDeadFrame();
  }

  componentDidUpdate() {
    this.drawDeadFrame();
  }

  handleClick() {
    console.log(this.props.selectedKey);

    var eventData = {
      'pos': this.props.pos,
      'selectedKey': this.props.selectedKey
    };

    Global.gameEmitter.emit(GameEvent.SELECTBOX_CLICK, eventData);
  }

  getFirstFrame(animation): number | undefined {
    if (Array.isArray(animation)) {
      return typeof animation[0] == 'number' ? animation[0] : undefined;
    }

    if (animation && Array.isArray(animation.frames)) {
      return typeof animation.frames[0] == 'number' ? animation.frames[0] : undefined;
    }

    return undefined;
  }

  getDeadSpriteFrame(): SpriteFrame | null {
    if (!this.props.showGravestone || this.props.selectedKey.id === undefined) {
      return null;
    }

    const objectState = Global.objectStates[this.props.selectedKey.id];

    if (!objectState || objectState.state != DEAD) {
      return null;
    }

    const imageDef = Global.imageDefList[objectState.image];

    if (!imageDef || !imageDef.animations || !imageDef.animations.dead || !imageDef.frames) {
      return null;
    }

    const frameIndex = this.getFirstFrame(imageDef.animations.dead);
    const width = imageDef.frames.width;
    const height = imageDef.frames.height;

    if (frameIndex === undefined || width === undefined || height === undefined) {
      return null;
    }

    return {
      imageName: objectState.image + '.png',
      frameIndex,
      width,
      height
    };
  }

  getCanvasStyle(frame: SpriteFrame): React.CSSProperties {
    return {
      ...(this.props.imageStyle || this.props.style),
      width: frame.width + 'px',
      height: frame.height + 'px'
    };
  }

  drawFallbackGravestone(ctx: CanvasRenderingContext2D, canvas: HTMLCanvasElement, requestId: number) {
    const gravestone = new Image();

    gravestone.onload = () => {
      if (requestId != this.drawRequestId) {
        return;
      }

      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(gravestone, 0, 0, canvas.width, canvas.height);
    };

    gravestone.src = '/static/art/gravestone.png';
  }

  drawDeadFrame() {
    const frame = this.getDeadSpriteFrame();
    const canvas = this.canvasRef.current;

    if (!frame || !canvas) {
      this.lastDrawFrameKey = '';
      return;
    }

    const ctx = canvas.getContext('2d');

    if (!ctx) {
      return;
    }

    const frameKey = frame.imageName + ':' + frame.frameIndex + ':' + frame.width + ':' + frame.height;

    if (frameKey == this.lastDrawFrameKey) {
      return;
    }

    this.lastDrawFrameKey = frameKey;

    const requestId = ++this.drawRequestId;
    canvas.width = frame.width;
    canvas.height = frame.height;
    ctx.imageSmoothingEnabled = false;

    const image = new Image();

    image.onload = () => {
      if (requestId != this.drawRequestId) {
        return;
      }

      const columns = Math.floor(image.naturalWidth / frame.width);
      const rows = Math.floor(image.naturalHeight / frame.height);
      const totalFrames = columns * rows;

      if (columns <= 0 || frame.frameIndex >= totalFrames) {
        this.drawFallbackGravestone(ctx, canvas, requestId);
        return;
      }

      const sourceX = (frame.frameIndex % columns) * frame.width;
      const sourceY = Math.floor(frame.frameIndex / columns) * frame.height;

      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(image, sourceX, sourceY, frame.width, frame.height, 0, 0, frame.width, frame.height);
    };

    image.onerror = () => {
      this.drawFallbackGravestone(ctx, canvas, requestId);
    };

    image.src = '/static/art/' + frame.imageName;
  }

  render() {
    const deadFrame = this.getDeadSpriteFrame();

    return (
      <div onClick={this.handleClick}>
        <img src={selectbox} style={this.props.style} />
        {!this.props.showGravestone &&
          <img src={'/static/art/' + this.props.imageName} style={this.props.imageStyle || this.props.style} /> }
        {this.props.showGravestone && deadFrame &&
          <canvas ref={this.canvasRef} style={this.getCanvasStyle(deadFrame)} />}
        {this.props.showGravestone &&
          !deadFrame &&
          <img src={'/static/art/gravestone.png'} style={this.props.style} />}
        {this.props.showBorder &&
          <img src={selectboxborder} style={this.props.style} />}
      </div>
    );
  }
}
