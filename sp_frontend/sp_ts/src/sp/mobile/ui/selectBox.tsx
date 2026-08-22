import * as React from "react";
import selectbox from "ui_comp/selectbox.png";
import selectboxborder from "ui_comp/selectboxborder.png";
import { Global } from "../../core/global";
import { GameEvent } from "../../core/gameEvent";
import { DEAD } from "../../core/config";
import { isDroppedBagObject } from "../../core/droppedBagPresentation";
import styles from "./../ui.module.css";

interface SelectedKey {
  type: string,
  id?: integer,
  x?: integer,
  y?: integer,
}

interface SelectBoxProps {
  pos: integer,
  selectedKey: SelectedKey,
  imageName: string | null,
  imageStyle?: React.CSSProperties,
  showBorder: boolean,
  showGravestone: boolean,
  label?: string,
}

interface SpriteFrame {
  imageName: string,
  frameIndex: number,
  width: number,
  height: number,
}

export default class SelectBox extends React.Component<SelectBoxProps, any> {
  private canvasRef = React.createRef<HTMLCanvasElement>();
  private drawRequestId = 0;
  private lastDrawFrameKey = '';

  componentDidMount() {
    this.drawDeadFrame();
  }

  componentDidUpdate() {
    this.drawDeadFrame();
  }

  handleClick = () => {
    Global.gameEmitter.emit(GameEvent.SELECTBOX_CLICK, {
      pos: this.props.pos,
      selectedKey: this.props.selectedKey,
    });
  };

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
    if (!this.props.showGravestone || this.props.selectedKey.id === undefined) return null;

    const objectState = Global.objectStates[this.props.selectedKey.id];
    const imageDef = objectState ? Global.imageDefList[objectState.image] : null;
    if (!objectState || objectState.state != DEAD || !imageDef?.animations?.dead || !imageDef.frames) {
      return null;
    }

    const frameIndex = this.getFirstFrame(imageDef.animations.dead);
    const width = imageDef.frames.width;
    const height = imageDef.frames.height;
    if (frameIndex === undefined || width === undefined || height === undefined) return null;

    return { imageName: objectState.image + '.png', frameIndex, width, height };
  }

  drawFallbackGravestone(ctx: CanvasRenderingContext2D, canvas: HTMLCanvasElement, requestId: number) {
    const gravestone = new Image();
    gravestone.onload = () => {
      if (requestId != this.drawRequestId) return;
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
    if (!ctx) return;

    const frameKey = `${frame.imageName}:${frame.frameIndex}:${frame.width}:${frame.height}`;
    if (frameKey == this.lastDrawFrameKey) return;
    this.lastDrawFrameKey = frameKey;

    const requestId = ++this.drawRequestId;
    canvas.width = frame.width;
    canvas.height = frame.height;
    ctx.imageSmoothingEnabled = false;

    const image = new Image();
    image.onload = () => {
      if (requestId != this.drawRequestId) return;
      const columns = Math.floor(image.naturalWidth / frame.width);
      const rows = Math.floor(image.naturalHeight / frame.height);
      if (columns <= 0 || frame.frameIndex >= columns * rows) {
        this.drawFallbackGravestone(ctx, canvas, requestId);
        return;
      }
      const sourceX = (frame.frameIndex % columns) * frame.width;
      const sourceY = Math.floor(frame.frameIndex / columns) * frame.height;
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(image, sourceX, sourceY, frame.width, frame.height, 0, 0, frame.width, frame.height);
    };
    image.onerror = () => this.drawFallbackGravestone(ctx, canvas, requestId);
    image.src = '/static/art/' + frame.imageName;
  }

  previewStyle(): React.CSSProperties {
    const objectState = this.props.selectedKey.id === undefined
      ? null
      : Global.objectStates[this.props.selectedKey.id];
    const droppedBag = isDroppedBagObject(objectState);
    return {
      position: 'absolute',
      inset: droppedBag ? '12px' : '3px',
      width: droppedBag ? '30px' : '48px',
      height: droppedBag ? '30px' : '48px',
      objectFit: 'contain',
      imageRendering: 'pixelated',
      pointerEvents: 'none',
      ...this.props.imageStyle,
    };
  }

  render() {
    const deadFrame = this.getDeadSpriteFrame();
    const label = this.props.label || (this.props.selectedKey.type === 'tile' ? 'Tile' : 'Target');

    return (
      <button type="button" className={styles.selectBox} onClick={this.handleClick} aria-label={label}>
        <img src={selectbox} className={styles.selectBoxLayer} alt="" aria-hidden="true" />
        {!this.props.showGravestone && this.props.imageName &&
          <img src={'/static/art/' + this.props.imageName} style={this.previewStyle()} alt="" aria-hidden="true" />}
        {this.props.showGravestone && deadFrame &&
          <canvas ref={this.canvasRef} className={styles.selectBoxLayer} />}
        {this.props.showGravestone && !deadFrame &&
          <img src="/static/art/gravestone.png" className={styles.selectBoxLayer} alt="" aria-hidden="true" />}
        {this.props.showBorder &&
          <img src={selectboxborder} className={styles.selectBoxLayer} alt="Selected" />}
      </button>
    );
  }
}
