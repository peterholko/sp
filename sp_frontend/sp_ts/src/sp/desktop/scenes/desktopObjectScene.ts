// Desktop overlays: blue Safe Logout wards and the distinct green live sanctuary border.

import Phaser from 'phaser';

import { Global } from '../../core/global';
import { NetworkEvent } from '../../core/networkEvent';
import { ObjectState } from '../../core/objectState';
import { ObjectScene } from '../../core/scenes/objectScene';
import {
  SanctuaryWardPresentation,
  WardSegment,
  sanctuaryWardPresentation,
} from './sanctuaryWardGeometry';
import {
  SanctuaryZoneBorderPresentation,
  sanctuaryZoneBorderPresentation,
} from './sanctuaryZoneBorderPresentation';

interface SanctuaryWardVisual {
  perimeter: Phaser.GameObjects.Graphics;
  marker: Phaser.GameObjects.Container;
  perimeterTween: Phaser.Tweens.Tween;
  markerTween: Phaser.Tweens.Tween;
  signature: string;
}

interface SanctuaryZoneBorderVisual {
  perimeter: Phaser.GameObjects.Graphics;
  perimeterTween: Phaser.Tweens.Tween;
  signature: string;
}

const WARD_BLUE = 0x72d6e8;
const WARD_GOLD = 0xe4c66f;
const WARD_INK = 0x10283a;
const WARD_PERIMETER_DEPTH = -1;
const WARD_MARKER_DEPTH = 4;
const ZONE_BORDER_GREEN = 0x4cc06a;
const ZONE_BORDER_ACCENT = 0x8fe0a0;
const ZONE_BORDER_DEPTH = -1;

/** Desktop-only presentation layer; mobile continues to register ObjectScene. */
export class DesktopObjectScene extends ObjectScene {
  private sanctuaryWards: Record<string, SanctuaryWardVisual> = {};
  private sanctuaryZoneBorders: Record<string, SanctuaryZoneBorderVisual> = {};
  private wardListenersRegistered = false;

  create(): void {
    super.create();

    this.registerWardListeners();
    this.syncSanctuaryWards();
    this.syncSanctuaryZoneBorders();
    this.events.once(Phaser.Scenes.Events.SHUTDOWN, this.shutdownSanctuaryOverlays, this);
    this.events.once(Phaser.Scenes.Events.DESTROY, this.shutdownSanctuaryOverlays, this);
  }

  update(time: number): void {
    super.update(time);
  }

  private registerWardListeners(): void {
    if (this.wardListenersRegistered) {
      return;
    }

    for (const event of [
      NetworkEvent.PROTECTED_SETTLEMENTS,
      NetworkEvent.PERCEPTION,
      NetworkEvent.NEW_PERCEPTION,
      NetworkEvent.OBJ_PERCEPTION,
      NetworkEvent.CHANGES,
    ]) {
      Global.gameEmitter.on(event, this.syncSanctuaryWards, this);
    }
    for (const event of [
      NetworkEvent.SANCTUARY_STATE,
      NetworkEvent.SANCTUARY_BORDER_VISIBILITY,
      NetworkEvent.PROTECTED_SETTLEMENTS,
      NetworkEvent.PERCEPTION,
      NetworkEvent.NEW_PERCEPTION,
      NetworkEvent.OBJ_PERCEPTION,
      NetworkEvent.CHANGES,
    ]) {
      Global.gameEmitter.on(event, this.syncSanctuaryZoneBorders, this);
    }
    for (const event of [
      NetworkEvent.SAFE_LOGOUT_RESET,
      NetworkEvent.SAFE_LOGOUT_COMPLETE,
      NetworkEvent.SERVER_OFFLINE,
      NetworkEvent.NETWORK_ERROR,
    ]) {
      Global.gameEmitter.on(event, this.clearSanctuaryWards, this);
      Global.gameEmitter.on(event, this.clearSanctuaryZoneBorders, this);
    }
    this.wardListenersRegistered = true;
  }

  private unregisterWardListeners(): void {
    if (!this.wardListenersRegistered) {
      return;
    }

    for (const event of [
      NetworkEvent.PROTECTED_SETTLEMENTS,
      NetworkEvent.PERCEPTION,
      NetworkEvent.NEW_PERCEPTION,
      NetworkEvent.OBJ_PERCEPTION,
      NetworkEvent.CHANGES,
    ]) {
      Global.gameEmitter.off(event, this.syncSanctuaryWards, this);
    }
    for (const event of [
      NetworkEvent.SANCTUARY_STATE,
      NetworkEvent.SANCTUARY_BORDER_VISIBILITY,
      NetworkEvent.PROTECTED_SETTLEMENTS,
      NetworkEvent.PERCEPTION,
      NetworkEvent.NEW_PERCEPTION,
      NetworkEvent.OBJ_PERCEPTION,
      NetworkEvent.CHANGES,
    ]) {
      Global.gameEmitter.off(event, this.syncSanctuaryZoneBorders, this);
    }
    for (const event of [
      NetworkEvent.SAFE_LOGOUT_RESET,
      NetworkEvent.SAFE_LOGOUT_COMPLETE,
      NetworkEvent.SERVER_OFFLINE,
      NetworkEvent.NETWORK_ERROR,
    ]) {
      Global.gameEmitter.off(event, this.clearSanctuaryWards, this);
      Global.gameEmitter.off(event, this.clearSanctuaryZoneBorders, this);
    }
    this.wardListenersRegistered = false;
  }

  private syncSanctuaryWards(): void {
    const activeAnchors = new Set<string>();

    for (const objectId in Global.objectStates) {
      const objectState = Global.objectStates[objectId] as ObjectState;
      const presentation = sanctuaryWardPresentation(
        objectState,
        Global.protectedSettlements,
      );
      if (!presentation) {
        continue;
      }

      const anchorId = presentation.settlement.monolith_id.toString();
      activeAnchors.add(anchorId);
      const signature = this.wardSignature(objectState, presentation);
      if (this.sanctuaryWards[anchorId]?.signature === signature) {
        continue;
      }

      this.destroySanctuaryWard(anchorId);
      this.sanctuaryWards[anchorId] = this.createSanctuaryWard(
        presentation,
        signature,
      );
    }

    for (const anchorId in this.sanctuaryWards) {
      if (!activeAnchors.has(anchorId)) {
        this.destroySanctuaryWard(anchorId);
      }
    }
  }

  private wardSignature(
    objectState: ObjectState,
    presentation: SanctuaryWardPresentation,
  ): string {
    return [
      objectState.x,
      objectState.y,
      presentation.settlement.player_id,
      presentation.settlement.sanctuary_radius,
    ].join(':');
  }

  private syncSanctuaryZoneBorders(): void {
    const activeAnchors = new Set<string>();

    for (const objectId in Global.objectStates) {
      const objectState = Global.objectStates[objectId] as ObjectState;
      const presentation = sanctuaryZoneBorderPresentation(
        objectState,
        Global.sanctuaryZones,
        Global.protectedSettlements,
        Global.sanctuaryBorderVisible,
      );
      if (!presentation) {
        continue;
      }

      const anchorId = presentation.zone.monolith_id.toString();
      activeAnchors.add(anchorId);
      const signature = this.sanctuaryZoneBorderSignature(objectState, presentation);
      if (this.sanctuaryZoneBorders[anchorId]?.signature === signature) {
        continue;
      }

      this.destroySanctuaryZoneBorder(anchorId);
      this.sanctuaryZoneBorders[anchorId] = this.createSanctuaryZoneBorder(
        presentation,
        signature,
      );
    }

    for (const anchorId in this.sanctuaryZoneBorders) {
      if (!activeAnchors.has(anchorId)) {
        this.destroySanctuaryZoneBorder(anchorId);
      }
    }
  }

  private sanctuaryZoneBorderSignature(
    objectState: ObjectState,
    presentation: SanctuaryZoneBorderPresentation,
  ): string {
    return [objectState.x, objectState.y, presentation.zone.radius].join(':');
  }

  private createSanctuaryWard(
    presentation: SanctuaryWardPresentation,
    signature: string,
  ): SanctuaryWardVisual {
    const perimeter = this.add.graphics();
    perimeter.setDepth(WARD_PERIMETER_DEPTH);
    this.drawPerimeter(perimeter, presentation.segments);

    const marker = this.createWardMarker(
      presentation.center.x,
      presentation.center.y - 42,
    );
    marker.setDepth(WARD_MARKER_DEPTH);

    const perimeterTween = this.tweens.add({
      targets: perimeter,
      alpha: { from: 0.48, to: 0.82 },
      duration: 2200,
      ease: 'Sine.InOut',
      yoyo: true,
      repeat: -1,
    });
    const markerTween = this.tweens.add({
      targets: marker,
      y: marker.y - 3,
      alpha: { from: 0.86, to: 1 },
      duration: 1800,
      ease: 'Sine.InOut',
      yoyo: true,
      repeat: -1,
    });

    return { perimeter, marker, perimeterTween, markerTween, signature };
  }

  private drawPerimeter(
    graphics: Phaser.GameObjects.Graphics,
    segments: WardSegment[],
  ): void {
    graphics.lineStyle(6, WARD_BLUE, 0.1);
    graphics.beginPath();
    for (const segment of segments) {
      graphics.moveTo(segment.start.x, segment.start.y);
      graphics.lineTo(segment.end.x, segment.end.y);
    }
    graphics.strokePath();

    graphics.lineStyle(2, WARD_BLUE, 0.72);
    for (const segment of segments) {
      this.strokeDashedSegment(graphics, segment, 12, 7);
    }

    const runePoints = new Map<string, { x: number; y: number }>();
    for (const segment of segments) {
      runePoints.set(`${segment.start.x}:${segment.start.y}`, segment.start);
      runePoints.set(`${segment.end.x}:${segment.end.y}`, segment.end);
    }
    graphics.fillStyle(WARD_GOLD, 0.68);
    Array.from(runePoints.values()).forEach((point, index) => {
      if (index % 3 === 0) {
        graphics.fillCircle(point.x, point.y, 2.2);
      }
    });
  }

  private createSanctuaryZoneBorder(
    presentation: SanctuaryZoneBorderPresentation,
    signature: string,
  ): SanctuaryZoneBorderVisual {
    const perimeter = this.add.graphics();
    perimeter.setDepth(ZONE_BORDER_DEPTH);
    this.drawSanctuaryZoneBorder(perimeter, presentation.segments);

    const perimeterTween = this.tweens.add({
      targets: perimeter,
      alpha: { from: 0.5, to: 0.82 },
      duration: 2600,
      ease: 'Sine.InOut',
      yoyo: true,
      repeat: -1,
    });

    return { perimeter, perimeterTween, signature };
  }

  private drawSanctuaryZoneBorder(
    graphics: Phaser.GameObjects.Graphics,
    segments: WardSegment[],
  ): void {
    graphics.lineStyle(6, ZONE_BORDER_GREEN, 0.1);
    graphics.beginPath();
    for (const segment of segments) {
      graphics.moveTo(segment.start.x, segment.start.y);
      graphics.lineTo(segment.end.x, segment.end.y);
    }
    graphics.strokePath();

    graphics.lineStyle(2, ZONE_BORDER_GREEN, 0.72);
    for (const segment of segments) {
      this.strokeDashedSegment(graphics, segment, 12, 7);
    }

    const accentPoints = new Map<string, { x: number; y: number }>();
    for (const segment of segments) {
      accentPoints.set(`${segment.start.x}:${segment.start.y}`, segment.start);
      accentPoints.set(`${segment.end.x}:${segment.end.y}`, segment.end);
    }
    graphics.fillStyle(ZONE_BORDER_ACCENT, 0.58);
    Array.from(accentPoints.values()).forEach((point, index) => {
      if (index % 4 === 0) {
        graphics.fillCircle(point.x, point.y, 1.8);
      }
    });
  }

  private strokeDashedSegment(
    graphics: Phaser.GameObjects.Graphics,
    segment: WardSegment,
    dashLength: number,
    gapLength: number,
  ): void {
    const dx = segment.end.x - segment.start.x;
    const dy = segment.end.y - segment.start.y;
    const length = Math.sqrt((dx * dx) + (dy * dy));
    if (length === 0) {
      return;
    }

    for (let offset = 0; offset < length; offset += dashLength + gapLength) {
      const startRatio = offset / length;
      const endRatio = Math.min(offset + dashLength, length) / length;
      graphics.beginPath();
      graphics.moveTo(
        segment.start.x + (dx * startRatio),
        segment.start.y + (dy * startRatio),
      );
      graphics.lineTo(
        segment.start.x + (dx * endRatio),
        segment.start.y + (dy * endRatio),
      );
      graphics.strokePath();
    }
  }

  private createWardMarker(x: number, y: number): Phaser.GameObjects.Container {
    const marker = this.add.container(x, y);
    const shield = this.add.graphics();

    shield.fillStyle(WARD_INK, 0.94);
    shield.lineStyle(2, WARD_GOLD, 0.95);
    shield.beginPath();
    shield.moveTo(-14, -16);
    shield.lineTo(14, -16);
    shield.lineTo(12, 5);
    shield.lineTo(0, 18);
    shield.lineTo(-12, 5);
    shield.closePath();
    shield.fillPath();
    shield.strokePath();

    // Crescent moon cut from two circles, plus a small ward spark.
    shield.fillStyle(0xbdeef5, 1);
    shield.fillCircle(-2, -2, 7);
    shield.fillStyle(WARD_INK, 1);
    shield.fillCircle(2, -5, 7);
    shield.fillStyle(WARD_GOLD, 1);
    shield.fillCircle(7, -10, 1.7);

    // Presentation-only so this scene cannot intercept MapScene tile clicks.
    // The selected-object panels provide the explanatory protected label.
    marker.add(shield);
    return marker;
  }

  private destroySanctuaryWard(anchorId: string): void {
    const ward = this.sanctuaryWards[anchorId];
    if (!ward) {
      return;
    }

    ward.perimeterTween.stop();
    ward.markerTween.stop();
    this.tweens.killTweensOf(ward.perimeter);
    this.tweens.killTweensOf(ward.marker);
    ward.perimeter.destroy();
    ward.marker.destroy(true);
    delete this.sanctuaryWards[anchorId];
  }

  private clearSanctuaryWards(): void {
    for (const anchorId of Object.keys(this.sanctuaryWards)) {
      this.destroySanctuaryWard(anchorId);
    }
  }

  private destroySanctuaryZoneBorder(anchorId: string): void {
    const border = this.sanctuaryZoneBorders[anchorId];
    if (!border) {
      return;
    }

    border.perimeterTween.stop();
    this.tweens.killTweensOf(border.perimeter);
    border.perimeter.destroy();
    delete this.sanctuaryZoneBorders[anchorId];
  }

  private clearSanctuaryZoneBorders(): void {
    for (const anchorId of Object.keys(this.sanctuaryZoneBorders)) {
      this.destroySanctuaryZoneBorder(anchorId);
    }
  }

  private shutdownSanctuaryOverlays(): void {
    this.unregisterWardListeners();
    this.clearSanctuaryWards();
    this.clearSanctuaryZoneBorders();
  }
}
