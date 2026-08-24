/**
 * @author       Peter Holko
 * @copyright    2018 - 2019 Peter Holko
 */

import { Util } from '../util';
import { Global } from '../global';
import { Tile } from '../objects/tile';
import { GameSprite } from '../objects/gameSprite';
import { GameEvent } from '../gameEvent';
import { MapScene } from './mapScene';
import { WeatherScene } from './weatherScene';
import { NetworkEvent } from '../networkEvent';
import { ObjectState } from '../objectState';
import { MultiImage } from '../multiImage';
import { Network } from '../network';
import {
  HERO, DEAD, SPRITE, CONTAINER, IMAGE, WALL, UNIT, STRUCTURE,
  VILLAGER, GATHERING, BUILDING, UPGRADING, desktopCameraZoom,
  isDesktop,
} from '../config';
import { GameImage } from '../objects/gameImage';
import { GameContainer } from '../objects/gameContainer';
import {
  fireAnimationPresentation,
  fireAnimationStartFrame,
} from '../fireAnimationPresentation';
import {
  isVisibilitySource,
  needsZeroVisionShroud,
} from '../visibilitySourcePolicy';
import {
  anchorActionProgress,
  requiresAuthoritativeActionProgress,
} from '../actionProgress';
import {
  ConstructionProgressSample,
  constructionProgressTimeline,
} from '../constructionProgress';
import {
  VillagerActivityIcon,
  villagerActivityIcon,
} from '../villagerResourceActivity';
import { badgeCenterAboveProgressBar } from '../villagerActivityLayout';
import {
  isPerceivedMapObject,
  isPresentMapObject,
} from '../mapObjectPresence';
import {
  DROPPED_BAG_RENDER_SCALE,
  isDroppedBagObject,
} from '../droppedBagPresentation';
import { usesFoundationGraphic } from '../structureConstructionPresentation';
import {
  objectStateText,
  repeatingObjectStateText,
} from '../objectStateText';
import {
  combatFloatingTextPresentation,
  shouldAnnounceCombo,
} from '../combatFloatingTextPresentation';

type RenderObject = GameSprite | GameImage | GameContainer;

const UNIT_DAMAGE_TEXT_COLOR = '#FF0000';
const STRUCTURE_DAMAGE_TEXT_COLOR = '#B87333';

interface PendingImageTask {
  objectId: string;
  image: string;
  renderKind?: string;
}

interface ActionProgressBar {
  graphics: Phaser.GameObjects.Graphics;
  state: string;
  startTime: number;
  durationMs: number;
  actionId?: number;
  authoritativeElapsedMs?: number;
}

interface VillagerActivityBadge {
  icon: Phaser.GameObjects.Image;
  outline: Phaser.GameObjects.Image;
}

interface StateTextTimer {
  state: string;
  value: string;
  timer: ReturnType<typeof setInterval>;
}

const ACTION_PROGRESS_WIDTH = 34;
const ACTION_PROGRESS_HEIGHT = 5;
const ACTION_PROGRESS_Y_OFFSET = -20;
const ACTION_PROGRESS_DEPTH = 24;
const ACTION_PROGRESS_FILL_COLOR = 0xffc857;
const ACTION_PROGRESS_DEFAULT_DURATION_MS = 3000;
const ACTION_PROGRESS_DURATIONS_MS: Record<string, number> = {
  building: 2500,
  upgrading: 2500,
  gathering: 15000,
  exploring: 2000,
  surveying: 2000,
  investigating: 2000,
  refining: 30000,
  eating: 3000,
  drinking: 3000,
  healing: 2000
};
const ACTION_PROGRESS_STATES = new Set([
  ...Object.keys(ACTION_PROGRESS_DURATIONS_MS),
  'prospecting',
]);
const VILLAGER_ACTIVITY_TEXTURE_PREFIX = 'villager-activity-';
const VILLAGER_ACTIVITY_ICONS: VillagerActivityIcon[] = [
  'gathering',
  'logging',
  'mining',
  'hunting',
  'fishing',
  'building',
  'eating',
  'drinking',
  'sleeping',
  'following',
  'fetching-tool',
  'hauling',
];
const VILLAGER_ACTIVITY_SIZE = 24;
const VILLAGER_ACTIVITY_OUTLINE_SIZE = 28;
const VILLAGER_ACTIVITY_OUTLINE_COLOR = 0xffd45a;
const VILLAGER_ACTIVITY_OUTLINE_ALPHA = 0.9;
const VILLAGER_ACTIVITY_Y_OFFSET = -23;
const VILLAGER_ACTIVITY_PROGRESS_GAP = 2;
const VILLAGER_ACTIVITY_OUTLINE_DEPTH = 25;
const VILLAGER_ACTIVITY_DEPTH = 26;

export class ObjectScene extends Phaser.Scene {

  private renderToggle = false;

  public objectList: Record<string, RenderObject> = {};

  private pendingImageTasks: Record<string, PendingImageTask> = {};

  private shroudTiles = [];

  private wallList: Record<string, ObjectState> = {};

  private multiImages: Record<string, Array<MultiImage>> = {};

  private stateTimerList: Record<string, StateTextTimer> = {};
  private fireAnimationSprites: Record<string, Phaser.GameObjects.Sprite> = {};
  private activeMoveTweens: Record<string, Phaser.Tweens.Tween> = {};
  private actionProgressBars: Record<string, ActionProgressBar> = {};
  private villagerActivityBadges: Record<string, VillagerActivityBadge> = {};

  private lastVillagerActivity: Record<string, string> = {};
  private lastFinisherShakeAt = 0;
  private comboAnnouncementTimes = new Map<string, number>();

  constructor() {
    super({
      key: "ObjectScene",
      active: true
    });

    this.processTextState = this.processTextState.bind(this);
  }

  preload(): void {
    this.load.image('selecthex', './static/art/hover-hex.png');
    this.load.image('foundation', './static/art/foundation.png');
    this.load.image('gravestone', './static/art/gravestone.png');
    this.load.image('rubble', './static/art/rubble.png');
    this.load.image('shroud', './static/art/shroud.png');
    for (const icon of VILLAGER_ACTIVITY_ICONS) {
      this.load.image(
        VILLAGER_ACTIVITY_TEXTURE_PREFIX + icon,
        './static/art/ui/activity/' + icon + '.png',
      );
    }

    this.load.image('shroud-n-ne-se', './static/art/shroud-n-ne-se.png');
    this.load.image('shroud-ne-se-s', './static/art/shroud-ne-se-s.png');
    this.load.image('shroud-ne-se', './static/art/shroud-ne-se.png');
    this.load.image('shroud-nw-n-ne', './static/art/shroud-nw-n-ne.png');
    this.load.image('shroud-s-sw-nw', './static/art/shroud-s-sw-nw.png');
    this.load.image('shroud-s-sw', './static/art/shroud-s-sw.png');
    this.load.image('shroud-se-s-sw', './static/art/shroud-se-s-sw.png');
    this.load.image('shroud-se-s', './static/art/shroud-se-s.png');
    this.load.image('shroud-sw-nw-n', './static/art/shroud-sw-nw-n.png');
    this.load.image('shroud-sw-nw', './static/art/shroud-sw-nw.png');
    this.load.image('shroud-ne-n', './static/art/shroud-ne-n.png');
    this.load.image('shroud-ne', './static/art/shroud-ne.png');
    this.load.image('shroud-nw', './static/art/shroud-nw.png');
    this.load.image('shroud-se', './static/art/shroud-se.png');
    this.load.image('shroud-sw', './static/art/shroud-sw.png');
    this.load.image('shroud-n', './static/art/shroud-n.png');
    this.load.image('shroud-s', './static/art/shroud-s.png');
    this.load.image('shroud-se-sw', './static/art/shroud-se-sw.png');
    this.load.image('shroud-ne-nw', './static/art/shroud-ne-nw.png');
    this.load.image('shroud-s-ne', './static/art/shroud-s-ne.png');
    this.load.image('shroud-one-tile', './static/art/shroud-one-tile.png');

    this.load.image('unknownunit', './static/art/unknown_unit.png');

    this.load.spritesheet('shadowbolt', './static/art/shadowbolt.png', { frameWidth: 72, frameHeight: 72, endFrame: 5 });
    this.load.spritesheet('burning', './static/art/burning.png', { frameWidth: 72, frameHeight: 72, endFrame: 14 });

  }

  create(): void {
    console.log('Object Scene Create');

    this.cameras.main.setZoom(desktopCameraZoom());
    Global.gameEmitter.on(GameEvent.CAMERA_ZOOM, (data) => {
      const duration = data.duration ?? 0;
      if (duration > 0) {
        this.cameras.main.zoomTo(data.zoom, duration, 'Sine.easeInOut');
      } else {
        this.cameras.main.setZoom(data.zoom);
      }
    }, this);

    var shadowBoltConfig = {
      key: 'shadowboltanim',
      frames: this.anims.generateFrameNumbers('shadowbolt', { start: 0, end: 5, first: 0 }),
      frameRate: 10
    }

    var burningConfig = {
      key: 'burninganim',
      frames: this.anims.generateFrameNumbers('burning', { start: 0, end: 14, first: 0 }),
      repeat: -1,
      frameRate: 10
    }

    this.anims.create(shadowBoltConfig);
    this.anims.create(burningConfig);

    this.onJumpComplete = this.onJumpComplete.bind(this);
    this.onMoveComplete = this.onMoveComplete.bind(this);
    this.onReturnComplete = this.onReturnComplete.bind(this);
    this.onDmgTextComplete = this.onDmgTextComplete.bind(this);

    Global.gameEmitter.on(NetworkEvent.PERCEPTION, this.renderInit, this);
    Global.gameEmitter.on(NetworkEvent.IMAGE_DEF, this.processImageDefMessage, this);
    Global.gameEmitter.on(NetworkEvent.DMG, this.processDmgMessage, this);
    Global.gameEmitter.on(NetworkEvent.SPOIL, this.processSpoilMessage, this);
    Global.gameEmitter.on(NetworkEvent.STEAL, this.processStealMessage, this);
    Global.gameEmitter.on(NetworkEvent.TORCH, this.processTorchMessage, this);
    Global.gameEmitter.on(NetworkEvent.SPEECH, this.processSpeech, this);
    Global.gameEmitter.on(NetworkEvent.SOUND, this.processSound, this);
    Global.gameEmitter.on(NetworkEvent.XP, this.processXp, this);
    Global.gameEmitter.on(NetworkEvent.GAINED_EFFECT, this.processGainedEffect, this);
    Global.gameEmitter.on(NetworkEvent.LOST_EFFECT, this.processLostEffect, this);
    Global.gameEmitter.on(NetworkEvent.REDUCED_EFFECT, this.processReducedEffect, this);
    Global.gameEmitter.on(NetworkEvent.INCREASED_EFFECT, this.processIncreasedEffect, this);
    Global.gameEmitter.on(NetworkEvent.INFO_ACTIVITY_UPDATE, this.processActivityUpdate, this);
    this.load.on('filecomplete', this.fileLoadComplete, this);
    this.load.on('complete', this.loadComplete, this);

    this.time.addEvent({ delay: 1000, callback: this.processFireAnimationState, callbackScope: this, loop: true });

  }

  processImageDefMessage(message) {
    console.log('image_def')

    // The server places `result` on `data` (not on the packet root) when the
    // tileset entry is missing — short-circuit cleanly so we don't try to
    // load a spritesheet with undefined frame dimensions.
    if (message.data?.result !== '404') {
      console.log(message.name);
      console.log(message.data);

      if (Array.isArray(message.data.images)) {
        console.log(message.data.images);

        //Check if already loaded
        if (!(message.name in Global.imageDefList)) {

          for (var i = 0; i < message.data.images.length; i++) {

            var multiImage: MultiImage = {
              key: message.name + i,
              imageName: message.data.images[i],
              width: message.data.frames[i][2],
              height: message.data.frames[i][3],
              regX: message.data.frames[i][5],
              regY: message.data.frames[i][6]
            }

            if (!this.multiImages.hasOwnProperty(message.name)) {
              this.multiImages[message.name] = new Array();
            }

            this.multiImages[message.name].push(multiImage);

            this.load.image(message.name + i, multiImage.imageName);
          }

          this.load.start();
        }
      } else {
        console.log(message.name);
        this.load.spritesheet(message.name, './static/art/' + message.name + '.png',
          {
            frameWidth: message.data.frames.width,
            frameHeight: message.data.frames.height
          })
        this.load.start();
      }

      Global.imageDefList[message.name] = message.data;
      Global.gameEmitter.emit(GameEvent.IMAGE_DEFINITION_READY, { imageName: message.name });
    }
  }

  renderInit(): void {
    console.log('renderInit');
    var objStates = Object.assign({}, Global.objectStates);

    for (var objId in objStates) {
      var objState = objStates[objId];
      objState.op = 'added';
    }

    this.drawObjects(objStates);

    Global.gameEmitter.on(NetworkEvent.CHANGES, this.setRender, this);
    Global.gameEmitter.on(NetworkEvent.NEW_PERCEPTION, this.setRender, this);
    Global.gameEmitter.on(NetworkEvent.OBJ_PERCEPTION, this.setRender, this);
    Global.gameEmitter.on(GameEvent.VISIBLE, this.drawAllObjects, this);

    this.time.addEvent({ delay: 200, callback: this.processRender, callbackScope: this, loop: true });
  }

  processRender(): void {
    if (this.renderToggle) {
      this.drawObjects(Global.objectStates)
      this.renderToggle = false;
    }
  }

  update(time: number): void {
    this.updateActionProgressBars(time);
    this.updateVillagerActivityBadges();
  }

  setRender(): void {
    console.log('ObjectScene setRender')
    this.renderToggle = true;
  }

  drawAllObjects(): void {
    console.log('########## drawAllObjects from Global.objectStates ##########');
    /*for (var objectId in Global.objectStates) {
      var objectState = Global.objectStates[objectId] as ObjectState;

      // Do not destroy structures that are not visible
      if (objectState.class == 'structure' && !Util.isVisible(objectState.x, objectState.y)) {
        continue;
      }

      var obj = this.objectList[objectId];
      if (obj) {
        obj.destroy();
      }
    }

    for (var objectId in Global.objectStates) {
      var objectState = Global.objectStates[objectId] as ObjectState;
      objectState.op = 'added';
    }*/

    this.setRender();
  }

  private getRenderedObject(objectId: string): RenderObject | null {
    var renderObject = this.objectList[objectId];

    if (renderObject && renderObject.scene != null) {
      return renderObject;
    }

    if (renderObject) {
      delete this.objectList[objectId];
    }

    return null;
  }

  private shouldRenderState(objectState: ObjectState): boolean {
    return isPresentMapObject(objectState);
  }

  private applyImagePresentation(
    image: GameImage,
    _imageName: string,
    presentationScale = 1,
  ): { insetX: number; insetY: number } {
    image.setScale(presentationScale);
    return {
      insetX: image.width * (1 - presentationScale) / 2,
      insetY: image.height * (1 - presentationScale) / 2,
    };
  }

  private isImageReady(imageName: string): boolean {
    if (!(imageName in Global.imageDefList)) {
      return false;
    }

    const imageType = Util.getImageType(imageName);

    if (imageType == CONTAINER) {
      if (!(imageName in this.multiImages)) {
        return false;
      }

      for (var i = 0; i < this.multiImages[imageName].length; i++) {
        if (!this.textures.exists(this.multiImages[imageName][i].key)) {
          return false;
        }
      }

      return true;
    }

    return this.textures.exists(imageName);
  }

  private queueImageDefTask(objectState: ObjectState): void {
    this.pendingImageTasks[objectState.id] = {
      objectId: objectState.id,
      image: objectState.image,
      renderKind: Global.imageDefList.hasOwnProperty(objectState.image) ? Util.getImageType(objectState.image) : undefined
    };

    if (!Global.imageDefList.hasOwnProperty(objectState.image)) {
      Global.network.sendImageDef(objectState.image);
    }
  }

  private clearPendingImageTask(objectId: string): void {
    delete this.pendingImageTasks[objectId];
  }

  private stopMoveTween(objectId: string): void {
    var tween = this.activeMoveTweens[objectId];

    if (tween) {
      tween.remove();
      delete this.activeMoveTweens[objectId];
    }

    var renderObject = this.getRenderedObject(objectId);
    if (renderObject) {
      this.tweens.killTweensOf(renderObject);
    }
  }

  private destroyRenderedObject(objectId: string): void {
    this.stopMoveTween(objectId);
    this.clearPendingImageTask(objectId);
    this.destroyActionProgressBar(objectId);
    this.destroyVillagerActivityBadge(objectId);

    this.clearStateTextTimer(objectId);

    if (this.fireAnimationSprites[objectId]) {
      this.fireAnimationSprites[objectId].destroy();
      delete this.fireAnimationSprites[objectId];
    }

    delete this.wallList[objectId];

    var renderObject = this.objectList[objectId];
    if (renderObject && renderObject.scene != null) {
      renderObject.destroy();
    }

    delete this.objectList[objectId];
  }

  private ensureRenderedObject(objectState: ObjectState): RenderObject | null {
    if (!this.shouldRenderState(objectState)) {
      this.destroyRenderedObject(objectState.id);
      return null;
    }

    var renderObject = this.getRenderedObject(objectState.id);

    if (!this.isImageReady(objectState.image)) {
      if (renderObject && (objectState.updateAttr == 'template' || objectState.updateAttr == 'image')) {
        this.destroyRenderedObject(objectState.id);
        renderObject = null;
      }

      this.queueImageDefTask(objectState);
      return null;
    }

    const imageType = Util.getImageType(objectState.image);

    if (renderObject) {
      if ((imageType == SPRITE && !(renderObject instanceof GameSprite)) ||
          (imageType == IMAGE && !(renderObject instanceof GameImage)) ||
          (imageType == CONTAINER && !(renderObject instanceof GameContainer))) {
        this.destroyRenderedObject(objectState.id);
        renderObject = null;
      } else if (imageType == SPRITE && (renderObject as GameSprite).imageName != objectState.image) {
        this.destroyRenderedObject(objectState.id);
        renderObject = null;
      } else if (imageType == CONTAINER && (renderObject as GameContainer).containerName != objectState.image) {
        this.destroyRenderedObject(objectState.id);
        renderObject = null;
      }
    }

    if (!renderObject) {
      if (imageType == SPRITE) {
        this.addSprite(objectState);
      } else if (imageType == IMAGE) {
        this.addImage(objectState);
      } else if (imageType == CONTAINER) {
        this.addContainer(objectState);
      }

      renderObject = this.getRenderedObject(objectState.id);
    }

    return renderObject;
  }

  private syncRenderedObject(objectState: ObjectState): void {
    if (!this.shouldRenderState(objectState)) {
      this.destroyRenderedObject(objectState.id);
      return;
    }

    var renderObject = this.ensureRenderedObject(objectState);
    if (!renderObject) {
      return;
    }

    const imageType = Util.getImageType(objectState.image);

    if (imageType == IMAGE) {
      this.updateImage(objectState);
    } else if (imageType == SPRITE) {
      this.updateSprite(objectState);
    } else if (imageType == CONTAINER) {
      this.updateContainer(objectState);
    }

    this.syncActionProgressBar(objectState, renderObject);
    this.syncVillagerActivityBadge(objectState, renderObject);
  }

  private destroyVillagerActivityBadge(objectId: string): void {
    var badge = this.villagerActivityBadges[objectId];

    if (badge?.icon.scene != null) {
      badge.icon.destroy();
    }
    if (badge?.outline.scene != null) {
      badge.outline.destroy();
    }

    delete this.villagerActivityBadges[objectId];
  }

  private syncVillagerActivityBadge(
    objectState: ObjectState,
    renderObject: RenderObject,
  ): void {
    const icon = villagerActivityIcon(objectState);
    if (!icon) {
      this.destroyVillagerActivityBadge(objectState.id);
      return;
    }

    const texture = VILLAGER_ACTIVITY_TEXTURE_PREFIX + icon;
    var badge = this.villagerActivityBadges[objectState.id];
    if (badge?.icon.scene == null || badge?.outline.scene == null) {
      if (badge?.icon.scene != null) {
        badge.icon.destroy();
      }
      if (badge?.outline.scene != null) {
        badge.outline.destroy();
      }
      delete this.villagerActivityBadges[objectState.id];
      badge = null;
    }

    if (!badge) {
      const outline = this.add.image(0, 0, texture);
      outline.setOrigin(0.5, 0.5);
      outline.setDisplaySize(
        VILLAGER_ACTIVITY_OUTLINE_SIZE,
        VILLAGER_ACTIVITY_OUTLINE_SIZE,
      );
      outline.setTintFill(VILLAGER_ACTIVITY_OUTLINE_COLOR);
      outline.setAlpha(VILLAGER_ACTIVITY_OUTLINE_ALPHA);
      outline.setDepth(VILLAGER_ACTIVITY_OUTLINE_DEPTH);

      const activityIcon = this.add.image(0, 0, texture);
      activityIcon.setOrigin(0.5, 0.5);
      activityIcon.setDisplaySize(
        VILLAGER_ACTIVITY_SIZE,
        VILLAGER_ACTIVITY_SIZE,
      );
      activityIcon.setDepth(VILLAGER_ACTIVITY_DEPTH);

      badge = { icon: activityIcon, outline };
      this.villagerActivityBadges[objectState.id] = badge;
    } else if (badge.icon.texture.key != texture) {
      badge.icon.setTexture(texture);
      badge.outline.setTexture(texture);
    }

    this.positionVillagerActivityBadge(objectState.id, badge, renderObject);
  }

  private positionVillagerActivityBadge(
    objectId: string,
    badge: VillagerActivityBadge,
    renderObject: RenderObject,
  ): void {
    const progressBar = this.actionProgressBars[objectId];
    const hasProgressBar = progressBar?.graphics.scene != null;
    const yOffset = hasProgressBar
      ? badgeCenterAboveProgressBar(
          ACTION_PROGRESS_Y_OFFSET,
          ACTION_PROGRESS_HEIGHT,
          VILLAGER_ACTIVITY_OUTLINE_SIZE,
          VILLAGER_ACTIVITY_PROGRESS_GAP,
        )
      : VILLAGER_ACTIVITY_Y_OFFSET;
    const x = renderObject.x + 36;
    const y = renderObject.y + yOffset;
    badge.icon.setPosition(x, y);
    badge.outline.setPosition(x, y);
  }

  private updateVillagerActivityBadges(): void {
    for (var objectId in this.villagerActivityBadges) {
      var objectState = Global.objectStates[objectId];
      var renderObject = this.getRenderedObject(objectId);
      const icon = villagerActivityIcon(objectState);

      if (!objectState || !renderObject || !icon) {
        this.destroyVillagerActivityBadge(objectId);
        continue;
      }

      const badge = this.villagerActivityBadges[objectId];
      const texture = VILLAGER_ACTIVITY_TEXTURE_PREFIX + icon;
      if (badge.icon.texture.key != texture) {
        badge.icon.setTexture(texture);
        badge.outline.setTexture(texture);
      }

      this.positionVillagerActivityBadge(objectId, badge, renderObject);
    }
  }

  private isActionProgressTarget(objectState: ObjectState): boolean {
    return (objectState.subclass == HERO || objectState.subclass == VILLAGER) &&
      ACTION_PROGRESS_STATES.has(objectState.state);
  }

  private getActionProgressDurationMs(state: string): number {
    return ACTION_PROGRESS_DURATIONS_MS[state] || ACTION_PROGRESS_DEFAULT_DURATION_MS;
  }

  private getStructureWorkProgress(structureId: string, structureState: ObjectState): ConstructionProgressSample | null {
    constructionProgressTimeline.updateFromSource({
      id: structureId,
      work_done: structureState.work_done,
      total_work: structureState.total_work,
      work_per_sec: structureState.work_per_sec,
      work_done_milliunits: structureState.work_done_milliunits,
      total_work_milliunits: structureState.total_work_milliunits,
      work_per_sec_milliunits: structureState.work_per_sec_milliunits,
      construction_action_id: structureState.construction_action_id,
      construction_updated_at_ms: structureState.construction_updated_at_ms,
    });

    return constructionProgressTimeline.sample(structureId);
  }

  private getConstructionProgressForActor(objectState: ObjectState): number | null {
    for (var structureId in Global.objectStates) {
      var structureState = Global.objectStates[structureId];

      if (!structureState || structureState.class != STRUCTURE ||
        (structureState.state != BUILDING && structureState.state != UPGRADING)) {
        continue;
      }

      if (structureState.x != objectState.x || structureState.y != objectState.y) {
        continue;
      }

      var progress = this.getStructureWorkProgress(structureId, structureState);

      if (!progress) {
        continue;
      }

      return progress.fraction;
    }

    return null;
  }

  private destroyActionProgressBar(objectId: string): void {
    var progressBar = this.actionProgressBars[objectId];

    if (!progressBar) {
      return;
    }

    if (progressBar.graphics.scene != null) {
      progressBar.graphics.destroy();
    }

    delete this.actionProgressBars[objectId];
  }

  private syncActionProgressBar(objectState: ObjectState, renderObject: RenderObject): void {
    if (!this.isActionProgressTarget(objectState)) {
      this.destroyActionProgressBar(objectState.id);
      return;
    }

    var progressBar = this.actionProgressBars[objectState.id];
    var now = this.time.now;
    var authoritative = anchorActionProgress(objectState, now);
    if (!authoritative && requiresAuthoritativeActionProgress(objectState.state)) {
      this.destroyActionProgressBar(objectState.id);
      return;
    }
    var durationMs = authoritative?.durationMs || this.getActionProgressDurationMs(objectState.state);

    if (!progressBar) {
      progressBar = {
        graphics: this.add.graphics(),
        state: objectState.state,
        startTime: authoritative?.startTimeMs ?? now,
        durationMs: durationMs,
        actionId: authoritative?.actionId,
        authoritativeElapsedMs: authoritative?.elapsedMs,
      };
      progressBar.graphics.setDepth(ACTION_PROGRESS_DEPTH);
      this.actionProgressBars[objectState.id] = progressBar;
    } else if (progressBar.state != objectState.state ||
               progressBar.actionId != authoritative?.actionId) {
      progressBar.state = objectState.state;
      progressBar.startTime = authoritative?.startTimeMs ?? now;
      progressBar.durationMs = durationMs;
      progressBar.actionId = authoritative?.actionId;
      progressBar.authoritativeElapsedMs = authoritative?.elapsedMs;
    } else if (authoritative &&
               (progressBar.durationMs != authoritative.durationMs ||
                progressBar.authoritativeElapsedMs != authoritative.elapsedMs)) {
      // A perception refresh or reconnect carries an updated server snapshot.
      progressBar.startTime = authoritative.startTimeMs;
      progressBar.durationMs = authoritative.durationMs;
      progressBar.authoritativeElapsedMs = authoritative.elapsedMs;
    }

    this.drawActionProgressBar(progressBar, objectState, renderObject, now);
  }

  private updateActionProgressBars(time: number): void {
    var now = typeof time == 'number' ? time : this.time.now;

    for (var objectId in this.actionProgressBars) {
      var objectState = Global.objectStates[objectId];
      var renderObject = this.getRenderedObject(objectId);

      if (!objectState || !renderObject || !this.isActionProgressTarget(objectState)) {
        this.destroyActionProgressBar(objectId);
        continue;
      }

      var progressBar = this.actionProgressBars[objectId];
      if (progressBar.state != objectState.state) {
        var authoritative = anchorActionProgress(objectState, now);
        progressBar.state = objectState.state;
        progressBar.startTime = authoritative?.startTimeMs ?? now;
        progressBar.durationMs = authoritative?.durationMs || this.getActionProgressDurationMs(objectState.state);
        progressBar.actionId = authoritative?.actionId;
        progressBar.authoritativeElapsedMs = authoritative?.elapsedMs;
      }

      this.drawActionProgressBar(progressBar, objectState, renderObject, now);
    }
  }

  private drawActionProgressBar(progressBar: ActionProgressBar, objectState: ObjectState, renderObject: RenderObject, time: number): void {
    var elapsed = Math.max(0, time - progressBar.startTime);
    var left = renderObject.x + 36 - ACTION_PROGRESS_WIDTH / 2;
    var top = renderObject.y + ACTION_PROGRESS_Y_OFFSET - ACTION_PROGRESS_HEIGHT / 2;

    progressBar.graphics.clear();
    progressBar.graphics.fillStyle(0x000000, 0.65);
    progressBar.graphics.fillRect(left, top, ACTION_PROGRESS_WIDTH, ACTION_PROGRESS_HEIGHT);
    progressBar.graphics.lineStyle(1, 0x111111, 0.9);
    progressBar.graphics.strokeRect(left, top, ACTION_PROGRESS_WIDTH, ACTION_PROGRESS_HEIGHT);

    var progress = objectState.state == BUILDING || objectState.state == UPGRADING
      ? this.getConstructionProgressForActor(objectState)
      : null;
    if (progress == null) {
      progress = Math.min(1, elapsed / progressBar.durationMs);
    }

    var fillWidth = Math.max(0, (ACTION_PROGRESS_WIDTH - 2) * progress);

    if (fillWidth > 0) {
      progressBar.graphics.fillStyle(ACTION_PROGRESS_FILL_COLOR, 1);
      progressBar.graphics.fillRect(left + 1, top + 1, fillWidth, ACTION_PROGRESS_HEIGHT - 2);
    }
  }

  private reconcileRenderedObjects(): void {
    for (var objectId in this.objectList) {
      var objectState = Global.objectStates[objectId];

      if (!objectState) {
        this.destroyRenderedObject(objectId);
        continue;
      }

      if (!this.shouldRenderState(objectState)) {
        this.destroyRenderedObject(objectId);
      }
    }
  }

  private flushPendingImageTasksForImage(imageName?: string): void {
    for (var objectId in this.pendingImageTasks) {
      var task = this.pendingImageTasks[objectId];

      if (imageName && task.image != imageName) {
        continue;
      }

      var objectState = Global.objectStates[objectId];

      if (!objectState || objectState.image != task.image || !this.shouldRenderState(objectState)) {
        delete this.pendingImageTasks[objectId];
        continue;
      }

      if (!this.isImageReady(task.image)) {
        continue;
      }

      delete this.pendingImageTasks[objectId];
      this.syncRenderedObject(objectState);
    }
  }

  private startMoveTween(objectId: string, renderObject: RenderObject, x: integer, y: integer): void {
    this.stopMoveTween(objectId);

    this.activeMoveTweens[objectId] = this.tweens.add({
      targets: renderObject,
      x: x,
      y: y,
      ease: 'Power1',
      duration: 500,
      onComplete: () => this.onMoveComplete(objectId)
    });
  }

  drawObjects(objectStates: Record<string, ObjectState>): void {
    console.log('***** drawObjects ******');
    console.log(JSON.stringify(Global.objectStates, null, 2));
    //Clear visibleTiles & shroud
    Global.visibleTiles = [];
    this.clearShroud();
    const positiveVisibleTiles = new Set<string>();

    for (var objectId in objectStates) {
      var objectState = objectStates[objectId] as ObjectState;
      console.log(objectState);

      if (isPerceivedMapObject(objectState)) {
        this.processVisibleTiles(objectState, positiveVisibleTiles);
      }

      if (objectState.op == 'added') {
        console.log('Object Added');
        this.syncRenderedObject(objectState);

        Global.objectStates[objectId].op = 'none';
        Global.objectStates[objectId].updateAttr = undefined;
        Global.objectStates[objectId].eventType = undefined;
      }
      else if (objectState.op == 'updated') {
        console.log('Object Updated');
        this.syncRenderedObject(objectState);
        Global.objectStates[objectId].op = 'none';
        Global.objectStates[objectId].updateAttr = undefined;
        Global.objectStates[objectId].eventType = undefined;
      } else if (objectState.op == 'deleted') {
        if (this.shouldRenderState(objectState)) {
          this.syncRenderedObject(objectState);
        } else {
          this.destroyRenderedObject(objectState.id);
        }
      }
    }

    console.log(Global.objectStates);

    this.reconcileRenderedObjects();

    // State/image updates should ignite or extinguish visible fire immediately;
    // the slower timer remains as a safety reconciliation pass.
    this.processFireAnimationState();

    //Call processWall here for loaded wall images
    this.processWallList();

    //Add Shroud tiles
    this.addShroud(positiveVisibleTiles);
  }

  processVisibleTiles(objectState: ObjectState, positiveVisibleTiles: Set<string>) {
    if (isVisibilitySource(objectState, Global.playerId)) {
      var visibleTiles = Util.range(objectState.x,
        objectState.y,
        objectState.vision);

      for (const tile of visibleTiles) {
        positiveVisibleTiles.add(`${tile.q},${tile.r}`);
      }
      Global.visibleTiles = Global.visibleTiles.concat(visibleTiles);
    } else if (objectState.player == Global.playerId && (objectState.vision ?? 0) <= 0) {

      if (objectState.subclass == HERO || objectState.subclass == VILLAGER) {
        // Add current tile to visible tiles
        Global.visibleTiles.push({
          q: objectState.x,
          r: objectState.y
        });
      }
    }
  }

  processWallList() {
    //Hide overlapping containers images
    for (var wallId in this.wallList) {
      var wallContainer = this.getRenderedObject(wallId) as GameContainer;

      if (!(wallContainer instanceof GameContainer)) {
        delete this.wallList[wallId];
        continue;
      }

      for (var childIndex = 0; childIndex < wallContainer.list.length; childIndex++) {
        (wallContainer.getAt(childIndex) as Phaser.GameObjects.Image)?.setVisible(true);
      }
    }

    for (var wallKey in this.wallList) {
      var wall = this.wallList[wallKey];
      var neighbours = Util.getNeighbours(wall.x, wall.y);

      for (var neighbourId in neighbours) {
        var neighbour = neighbours[neighbourId];

        for (var otherId in this.wallList) {
          var other = this.wallList[otherId];

          if ((neighbour.q == other.x) && (neighbour.r == other.y)) {
            var container = this.getRenderedObject(wall.id) as GameContainer;

            if (!(container instanceof GameContainer)) {
              continue;
            }

            if (neighbour.d == 'nw') {
              (container.getAt(2) as Phaser.GameObjects.Image)?.setVisible(false);
              (container.getAt(4) as Phaser.GameObjects.Image)?.setVisible(false);
            } else if (neighbour.d == 'ne') {
              (container.getAt(3) as Phaser.GameObjects.Image)?.setVisible(false);
              (container.getAt(5) as Phaser.GameObjects.Image)?.setVisible(false);
            } else if (neighbour.d == 'n') {
              (container.getAt(0) as Phaser.GameObjects.Image)?.setVisible(false);
              (container.getAt(1) as Phaser.GameObjects.Image)?.setVisible(false);
              (container.getAt(4) as Phaser.GameObjects.Image)?.setVisible(false);
              (container.getAt(5) as Phaser.GameObjects.Image)?.setVisible(false);
            } else if (neighbour.d == 's') {
              (container.getAt(8) as Phaser.GameObjects.Image)?.setVisible(false);
              (container.getAt(9) as Phaser.GameObjects.Image)?.setVisible(false);
            } else if (neighbour.d == 'sw') {
              (container.getAt(6) as Phaser.GameObjects.Image)?.setVisible(false);
            } else if (neighbour.d == 'se') {
              (container.getAt(7) as Phaser.GameObjects.Image)?.setVisible(false);
            }
          }
        }
      }
    }
  }

  getDirectionPermutations(arr) {
    if (arr.length === 1) {
      return [
        [arr[0]]
      ];
    }
    if (arr.length === 2) {
      return [
        [arr[0], arr[1]],
        [arr[1], arr[0]]
      ];
    }
    if (arr.length === 3) {
      return [
        [arr[0], arr[1], arr[2]],
        [arr[0], arr[2], arr[1]],
        [arr[1], arr[0], arr[2]],
        [arr[1], arr[2], arr[0]],
        [arr[2], arr[0], arr[1]],
        [arr[2], arr[1], arr[0]]
      ];
    }
    return [];
  }



  addShroud(positiveVisibleTiles: ReadonlySet<string>) {
    const existingShroudTiles = new Map(); // key: "q,r", value: Set of filenames used

    for (var index in Global.tileStates) {
      var tileState = Global.tileStates[index];

      if (Util.isVisible(tileState.hexX, tileState.hexY) == false) {
        var pixel = Util.hex_to_pixel(tileState.hexX, tileState.hexY);

        var shroud = new GameImage({
          scene: this,
          x: pixel.x,
          y: pixel.y,
          id: 'shroud' + pixel.x + pixel.y,
          imageName: 'shroud'
        });
        this.add.existing(shroud);
        this.shroudTiles.push(shroud);
      }
    }

    const HEX_DIRECTIONS = [
      { q: 1, r: 0, dir: "e" },
      { q: 1, r: -1, dir: "ne" },
      { q: 0, r: -1, dir: "n" },
      { q: -1, r: 0, dir: "w" },
      { q: -1, r: 1, dir: "sw" },
      { q: 0, r: 1, dir: "s" }
    ];

    const VALID_SHROUD_IMAGES = new Set([
      'shroud-n-ne-se',
      'shroud-ne-se-s',
      'shroud-ne-se',
      'shroud-nw-n-ne',
      'shroud-s-sw-nw',
      'shroud-s-sw',
      'shroud-se-s-sw',
      'shroud-se-s',
      'shroud-sw-nw-n',
      'shroud-sw-nw',
      'shroud-ne-n',
      'shroud-ne',
      'shroud-nw',
      'shroud-se',
      'shroud-sw',
      'shroud-s',
      'shroud-n',
      'shroud-se-sw',
      'shroud-ne-nw',
      'shroud-s-ne'
    ]);

    const visibleSet = new Set(
      Global.visibleTiles.map(tile => `${tile.q},${tile.r}`)
    );

    //console.log(Global.visibleTiles);

    for (const { q, r } of Global.visibleTiles) {
      //console.log(q, r);

      const missingDirs = [];
      const neighbors = Util.getNeighbours(q, r);

      //console.log(neighbors);
      for (const neighbor of neighbors) {
        const neighborKey = `${neighbor.q},${neighbor.r}`;
        if (!visibleSet.has(neighborKey)) {
          missingDirs.push(neighbor.d);
        }
      }

      //console.log(missingDirs);

      if (missingDirs.length >= 1 && missingDirs.length <= 3) {
        const permutations = this.getDirectionPermutations(missingDirs);
        //console.log(permutations);

        for (const perm of permutations) {
          const filename = `shroud-${perm.join('-')}`;
          const tileKey = `${q},${r}`;

          const usedFilenames = existingShroudTiles.get(tileKey) || new Set();
          //console.log(existingShroudTiles);

          if (VALID_SHROUD_IMAGES.has(filename) && !usedFilenames.has(filename)) {
            var pixel = Util.hex_to_pixel(q, r);

            //console.log("Adding shroud tile: " + filename + " at " + q + ", " + r);

            var shroud = new GameImage({
              scene: this,
              x: pixel.x,
              y: pixel.y,
              id: 'shroud' + pixel.x + pixel.y,
              imageName: filename
            });
            this.add.existing(shroud);
            this.shroudTiles.push(shroud);

            usedFilenames.add(filename);
            existingShroudTiles.set(tileKey, usedFilenames);
          }
        }
      }
    }

    for (var objectId in Global.objectStates) {
      var objectState = Global.objectStates[objectId];

      if (!isPerceivedMapObject(objectState)) {
        continue;
      }

      const coveredByPositiveVisibilitySource = positiveVisibleTiles.has(
        `${objectState.x},${objectState.y}`,
      );

      if (needsZeroVisionShroud(objectState, coveredByPositiveVisibilitySource)) {

        var pixel = Util.hex_to_pixel(objectState.x, objectState.y);

        // Check for exiting shroud-one-tile on the same pixel 
        var existingShroud = this.shroudTiles.find(shroud => shroud.imageName == 'shroud-one-tile' && shroud.x == pixel.x && shroud.y == pixel.y);
        if (existingShroud) {
          continue;
        }

        var shroud = new GameImage({
          scene: this,
          x: pixel.x,
          y: pixel.y,
          imageName: 'shroud-one-tile'
        });
        this.add.existing(shroud);
        this.shroudTiles.push(shroud);
      }
    }
  }



  clearShroud() {
    for (var i = 0; i < this.shroudTiles.length; i++) {
      var shroud = this.shroudTiles[i];

      if (shroud?.scene != null) {
        shroud.destroy();
      }
    }

    this.shroudTiles = [];
  }

  updateImage(objectState: ObjectState) {
    console.log('UpdateImage: ' + JSON.stringify(objectState));
    var image = this.getRenderedObject(objectState.id) as GameImage;

    if (!(image instanceof GameImage)) {
      return;
    }

    var pixel = Util.hex_to_pixel(objectState.x, objectState.y);

    console.log(image);

    if (usesFoundationGraphic(objectState)) {
      if (image.imageName != 'foundation') {
        image.setTexture('foundation');
        image.imageName = 'foundation';
      }
    } else if (objectState.image != image.imageName) {
      image.setTexture(objectState.image);
      image.imageName = objectState.image;
    }
    const presentationScale = isDroppedBagObject(objectState) ? DROPPED_BAG_RENDER_SCALE : 1;
    const inset = this.applyImagePresentation(
      image,
      image.imageName,
      presentationScale,
    );
    const renderX = pixel.x + inset.insetX;
    const renderY = pixel.y + inset.insetY;
    console.log(pixel);

    if (objectState.class == 'structure') {
      image.setDepth(1);
    } else if (objectState.class == 'unit') {
      image.setDepth(3);
    } else {
      image.setDepth(2);
    }

    //Move completed, add tween to new location
    if (image.x != renderX || image.y != renderY) {
      this.startMoveTween(objectState.id, image, renderX, renderY);
    }
  }

  updateSprite(objectState: ObjectState) {
    var sprite = this.getRenderedObject(objectState.id) as GameSprite;
    console.log('updateSprite');
    console.log(sprite);

    var pixel = Util.hex_to_pixel(objectState.x, objectState.y);

    // Guard against destroyed sprites still present in objectList (scene is undefined after destroy)
    const spriteReady = sprite != null && sprite.scene != null;

    if (spriteReady && objectState.state != DEAD && (sprite as any).__spDeadRenderLocked) {
      delete (sprite as any).__spDeadRenderLocked;
    }

    if (spriteReady && objectState.state == DEAD) {
      this.clearStateTextTimer(objectState.id);
      this.updateDeadRenderState(objectState.id, sprite);
      return;
    }

    if (objectState.state == 'moving') {
      this.syncStateTextTimer(objectState, sprite, 'moving', true);
      if (spriteReady) {
        sprite.play(objectState.image + '_moving');
        sprite.x = pixel.x;
        sprite.y = pixel.y;
      }
    } else {
      var animState;
      var anim;

      if (objectState.state == DEAD && objectState.prevstate != DEAD) {

        console.log("Playing Die animation from UpdateState");
        animState = 'die';
      } else {
        animState = objectState.state;
      }

      anim = objectState.image + '_' + animState;
      const animationExists = this.anims.exists(anim);
      this.syncStateTextTimer(objectState, sprite, animState, animationExists);
      console.log(this.anims);
      if (animationExists) {

        console.log("Playing animation: " + anim);
        console.log(anim);
        if (spriteReady) {
          console.log(typeof sprite);
          console.log(sprite)
          sprite.play(anim);
        } else {
          console.log("Error in animations for sprite for obj: " + objectState.id);
        }
      } else {
        console.log('Animation ' + anim + ' does not exist');
        if (objectState.state == DEAD && spriteReady) {
          sprite.setTexture('gravestone');
          sprite.setDepth(2);
        }
      }

      //Only follow if Hero
      if (objectState.subclass == HERO && objectState.player == Global.playerId) {
        var mapScene = this.scene.get('MapScene') as MapScene;
        mapScene.cameras.main.startFollow(sprite, true);
        mapScene.cameras.main.followOffset.x = -36;
        mapScene.cameras.main.followOffset.y = -36;

        var weatherScene = this.scene.get('WeatherScene') as WeatherScene;
        weatherScene.cameras.main.startFollow(sprite, true);
        weatherScene.cameras.main.followOffset.x = -36;
        weatherScene.cameras.main.followOffset.y = -36;

        this.cameras.main.startFollow(sprite, true);
        this.cameras.main.followOffset.x = -36;
        this.cameras.main.followOffset.y = -36;
      }

      if (spriteReady) {
        //Move completed, add tween to new location
        if (sprite.x != pixel.x || sprite.y != pixel.y) {
          this.startMoveTween(objectState.id, sprite, pixel.x, pixel.y);
        }
      }
    }
  }

  updateContainer(objectState: ObjectState) {
    var container = this.getRenderedObject(objectState.id) as GameContainer;

    if (!(container instanceof GameContainer)) {
      return;
    }

    var pixel = Util.hex_to_pixel(objectState.x, objectState.y);
    container.x = pixel.x;
    container.y = pixel.y;

    if (objectState.class == 'structure') {
      container.setDepth(1);
    } else if (objectState.class == 'unit') {
      container.setDepth(3);
    } else {
      container.setDepth(2);
    }

    if (usesFoundationGraphic(objectState)) {
      const currentImage = container.list.length == 1
        ? container.list[0]
        : null;
      const alreadyShowingFoundation = currentImage instanceof GameImage &&
        currentImage.imageName == 'foundation';

      if (!alreadyShowingFoundation) {
        container.removeAll(true);
        var foundationImage = new GameImage({
          scene: this,
          x: 0,
          y: 0,
          id: objectState.id,
          imageName: "foundation"
        });

        container.add(foundationImage);
      }
    } else if (objectState.state == 'none') {
      var multiImageList = this.multiImages[objectState.image];
      container.removeAll(true);

      for (var i = 0; i < multiImageList.length; i++) {
        var multiImage = multiImageList[i] as MultiImage;

        var image = new GameImage({
          scene: this,
          x: -1 * multiImage.regX,
          y: -1 * multiImage.regY,
          id: objectState.id,
          imageName: multiImage.key
        });
        container.add(image);
      }
    }

    if (objectState.subclass == WALL) {
      this.wallList[objectState.id] = objectState;
    } else {
      delete this.wallList[objectState.id];
    }
  }

  addSprite(objectState: ObjectState) {
    var pixel = Util.hex_to_pixel(objectState.x, objectState.y);
    var imageName = objectState.image;

    var sprite = new GameSprite({
      scene: this,
      x: pixel.x,
      y: pixel.y,
      id: objectState.id,
      imageName: imageName
    });

    sprite.setDepth(3);

    this.add.existing(sprite);

    var anim = objectState.image + '_' + objectState.state;

    if (this.anims.exists(anim)) {
      sprite.anims.play(anim);
    } else {
      if (objectState.state == DEAD) {
        sprite.setTexture('gravestone');
        sprite.setDepth(2);
      }

      console.log('No animation found, not playing');
    }

    this.objectList[objectState.id] = sprite;
    delete this.wallList[objectState.id];

    if (objectState.subclass == 'hero') {
      var mapScene = this.scene.get('MapScene') as MapScene;
      mapScene.cameras.main.centerOn(sprite.x + 36, sprite.y + 36);

      var weatherScene = this.scene.get('WeatherScene') as WeatherScene;
      weatherScene.cameras.main.centerOn(sprite.x + 36, sprite.y + 36);

      this.cameras.main.centerOn(sprite.x + 36, sprite.y + 36);

      sprite.setDepth(5);
    }
  }

  addLoadedSprites(imageName) {
    this.flushPendingImageTasksForImage(imageName);
  }

  createSpriteAnimation(imageName) {
    var animsData = Global.imageDefList[imageName].animations;

    for (var animName in animsData) {
      var anim = animsData[animName];
      var repeat = 0;
      var duration;
      var frames;

      if (Array.isArray(anim)) {
        if (anim.length > 1) {
          var start = anim[0];
          var end = anim[1];

          repeat = anim[2];
          duration = anim[3];

          frames = this.anims.generateFrameNumbers(imageName, { start: start, end: end });

        } else {
          duration = 10000;
          frames = this.anims.generateFrameNumbers(imageName, { start: anim[0], end: anim[0] });
        }
      }
      else if (Array.isArray(anim.frames)) {
        duration = anim.speed;
        repeat = anim.repeat;
        frames = this.anims.generateFrameNumbers(imageName, { frames: anim.frames });
      } else {
        console.log('Should never reach here')
      }

      var config = {
        key: imageName + '_' + animName,
        frames: frames,
        repeat: repeat,
        duration: duration
      };

      console.log(config);
      if (!this.anims.exists(config.key)) {
        this.anims.create(config);
      }
    }
  }

  addImage(objectState: ObjectState) {
    var pixel = Util.hex_to_pixel(objectState.x, objectState.y);
    var imageName = '';

    if (usesFoundationGraphic(objectState)) {
      imageName = 'foundation';
    } else {
      imageName = objectState.image;
    }

    var image = new GameImage({
      scene: this,
      x: pixel.x,
      y: pixel.y,
      id: objectState.id,
      imageName: imageName
    });

    const presentationScale = isDroppedBagObject(objectState) ? DROPPED_BAG_RENDER_SCALE : 1;
    const inset = this.applyImagePresentation(image, imageName, presentationScale);
    image.x += inset.insetX;
    image.y += inset.insetY;

    if (objectState.class == 'structure') {
      image.setDepth(1);
    } else if (objectState.class == 'unit') {
      image.setDepth(3);
    } else {
      image.setDepth(2);
    }

    this.add.existing(image);

    this.objectList[objectState.id] = image;
    delete this.wallList[objectState.id];
  }

  addLoadedImages(imageName) {
    this.flushPendingImageTasksForImage(imageName);
  }

  addContainer(objectState: ObjectState) {
    console.log('Adding container')
    var pixel = Util.hex_to_pixel(objectState.x, objectState.y);

    var container = new GameContainer({
      scene: this,
      x: pixel.x,
      y: pixel.y,
      id: objectState.id,
      containerName: objectState.image
    });

    if (objectState.class == 'structure') {
      container.setDepth(1);
    } else if (objectState.class == 'unit') {
      container.setDepth(3);
    } else {
      container.setDepth(2);
    }

    this.add.existing(container);

    this.objectList[objectState.id] = container;

    if (usesFoundationGraphic(objectState)) {
      var image = new GameImage({
        scene: this,
        x: 0,
        y: 0,
        id: objectState.id,
        imageName: "foundation"
      });

      container.add(image);
      console.log(container);
      delete this.wallList[objectState.id];
    } else {

      var multiImageList = this.multiImages[objectState.image];

      for (var i = 0; i < multiImageList.length; i++) {
        var multiImage = multiImageList[i] as MultiImage;

        var image = new GameImage({
          scene: this,
          x: -1 * multiImage.regX,
          y: -1 * multiImage.regY,
          id: objectState.id,
          imageName: multiImage.key
        });
        container.add(image);
      }

      if (objectState.subclass == WALL) {
        this.wallList[objectState.id] = objectState;
      } else {
        delete this.wallList[objectState.id];
      }
    }
  }

  addLoadedContainerImages(imageName) {
    this.flushPendingImageTasksForImage(imageName);
  }

  fileLoadComplete(key, type, raw) {
    console.log('Loaded file: ' + key + ', ' + type);
    var imageName = key;
    if (!(imageName in Global.imageDefList)) {
      return;
    }

    var imageType = Util.getImageType(imageName);

    if (imageType == SPRITE) {
      this.createSpriteAnimation(imageName);
      this.addLoadedSprites(imageName);
    } else if (imageType == IMAGE) {
      this.addLoadedImages(imageName);
    }

  }

  loadComplete() {
    console.log('loadComplete')
    this.flushPendingImageTasksForImage();

    //Call process wall for images finished loading
    this.processWallList();

    Global.gameEmitter.emit(GameEvent.LOADING_FINISHED, {});
  }

  private forceDeadFinalFrame(targetId: string, target: RenderObject): void {
    var targetState = Global.objectStates[targetId];

    if (!targetState || targetState.state != DEAD) {
      return;
    }

    if (targetState.class == UNIT) {
      if (target instanceof GameSprite) {
        var deadAnim = target.imageName + '_dead';

        if (this.anims.exists(deadAnim)) {
          target.anims.chain();
          target.play(deadAnim);
        } else {
          target.anims.chain();
          target.anims.stop();
          target.setTexture('gravestone');
          target.setDepth(2);
        }
      } else if (target instanceof GameImage) {
        target.setTexture('gravestone');
        target.imageName = 'gravestone';
      }
    } else if (targetState.class == STRUCTURE) {
      if (target instanceof GameContainer) {
        target.removeAll(true);

        var image = new GameImage({
          scene: this,
          x: 0,
          y: 0,
          id: targetId,
          imageName: "foundation"
        });

        target.add(image);
      } else if (target instanceof GameImage) {
        target.setTexture('foundation');
        target.imageName = 'foundation';
      }
    }
  }

  private updateDeadRenderState(targetId: string, target: RenderObject): void {
    var targetState = Global.objectStates[targetId];

    if (!targetState) {
      return;
    }

    if (targetState.class == UNIT) {
      if (target instanceof GameSprite) {
        if ((target as any).__spDeadRenderLocked) {
          return;
        }

        (target as any).__spDeadRenderLocked = true;
        var anim = target.imageName + '_die';

        if (this.anims.exists(anim)) {
          console.log("Playing Die animation from Damage");
          target.once(Phaser.Animations.Events.ANIMATION_COMPLETE, () => {
            this.forceDeadFinalFrame(targetId, target);
          });
          target.play(anim);
        } else {
          this.forceDeadFinalFrame(targetId, target);
        }
      } else if (target instanceof GameImage) {
        this.forceDeadFinalFrame(targetId, target);
      }
    } else if (targetState.class == STRUCTURE) {
      this.forceDeadFinalFrame(targetId, target);
    }
  }

  private getDamageTextColor(targetId): string {
    var targetState = Global.objectStates[targetId];

    if (targetState && targetState.class == STRUCTURE) {
      return STRUCTURE_DAMAGE_TEXT_COLOR;
    }

    return UNIT_DAMAGE_TEXT_COLOR;
  }

  private shakeForFinisher(): void {
    const now = Date.now();
    if (now - this.lastFinisherShakeAt < 250) {
      return;
    }
    this.lastFinisherShakeAt = now;
    this.cameras.main.shake(180, 0.008);
    const mapScene = this.scene.get('MapScene') as MapScene;
    mapScene?.cameras?.main?.shake(180, 0.008);
  }

  private showComboAnnouncement(
    sourceId: number,
    source: RenderObject,
    comboText?: string,
  ): void {
    if (
      !comboText
      || !shouldAnnounceCombo(
        this.comboAnnouncementTimes,
        sourceId,
        comboText,
        Date.now(),
      )
    ) {
      return;
    }

    this.shakeForFinisher();
    const text = this.add.text(source.x + 36, source.y - 5, comboText, {
      fontFamily: 'Verdana',
      fontSize: 30,
      fontStyle: 'bold',
      align: 'center',
      color: '#FFD45A',
      stroke: '#000000',
      strokeThickness: 6,
    });
    text.setDepth(10);
    text.setOrigin(0.5, 0.5);

    this.tweens.add({
      targets: text,
      y: source.y - 50,
      alpha: 0,
      ease: 'Power1',
      duration: 5000,
      onComplete: this.onDmgTextComplete,
    }).play();
  }

  private showTargetDamage(
    targetId: number,
    target: RenderObject,
    targetText: string,
    compact = false,
  ): void {
    const text = this.add.text(target.x + 36, target.y - 5, targetText, {
      fontFamily: 'Verdana',
      fontSize: compact ? 20 : 22,
      fontStyle: 'normal',
      align: 'center',
      color: this.getDamageTextColor(targetId),
      stroke: '#000000',
      strokeThickness: compact ? 3 : 4,
    });
    text.setDepth(10);
    text.setOrigin(0.5, 0.5);

    this.tweens.add({
      targets: text,
      y: target.y - 50,
      alpha: 0,
      ease: 'Power1',
      duration: 5000,
      onComplete: this.onDmgTextComplete,
    }).play();
  }

  private playSpriteAction(source: RenderObject, action: string): void {
    if (!(source instanceof GameSprite)) {
      return;
    }

    const actionAnimation = source.imageName + '_' + action;
    if (!this.anims.exists(actionAnimation)) {
      return;
    }

    source.play(actionAnimation);
    const idleAnimation = source.imageName + '_none';
    if (this.anims.exists(idleAnimation)) {
      source.anims.chain(idleAnimation);
    }
  }

  processDmgMessage(message) {
    console.log('Dmg Message: ' + message.source_id + ' -> ' + message.target_id);
    const floatingText = combatFloatingTextPresentation(message);
    if (message.source_id in Global.objectStates && message.target_id in Global.objectStates) {
      if (message.source_id in this.objectList &&
        message.target_id in this.objectList) {
        var source = this.objectList[message.source_id] as GameSprite;
        var target = this.objectList[message.target_id];

        console.log('Source: ' + source);

        if (source == null || target == null)
          return;

        var sourceIsDead = Global.objectStates[message.source_id].state == DEAD;

        // What is this for? Is this in the wrong spot
        if (Global.objectStates[message.source_id].subclass == HERO) {

          var mapScene = this.scene.get('MapScene') as MapScene;
          mapScene.cameras.main.stopFollow();
          this.cameras.main.stopFollow();
        }

        //TODO Check subclass 
        if (message.state == DEAD) {
          this.updateDeadRenderState(message.target_id, target);
        }

        if (!sourceIsDead && message.attack_type == 'Shadow Bolt') {
          this.playSpriteAction(source, 'cast');

          var shadowBolt = this.add.sprite(
            source.x + 36,
            source.y + 36,
            'shadowbolt',
          );
          shadowBolt.anims.play('shadowboltanim');

          var diffX = (target.x - source.x) * 0.5;
          var diffY = (target.y - source.y) * 0.5;

          var destX = target.x + 36;
          var destY = target.y + 36;

          var tween = this.tweens.add({
            targets: shadowBolt,
            x: destX,
            y: destY,
            ease: 'Power2',
            duration: 1000,
          });

        } else if (!sourceIsDead) {
          console.log('Play attack');
          this.playSpriteAction(source, 'attack');

          var diffX = (target.x - source.x) * 0.5;
          var diffY = (target.y - source.y) * 0.5;

          var destX = source.x + diffX;
          var destY = source.y + diffY;

          var tween = this.tweens.add({
            targets: source,
            x: destX,
            y: destY,
            ease: 'Power2',
            duration: 750,
            onComplete: this.onJumpComplete,
            callbackScope: this
          });

          tween.play();
        }

        this.showComboAnnouncement(message.source_id, source, floatingText.sourceText);
        this.showTargetDamage(message.target_id, target, floatingText.targetText);
      }
    } else if (message.target_id in Global.objectStates) {
      var targetObjectState = Global.objectStates[message.target_id];
      var target = this.objectList[message.target_id];

      if (target == null) {
        return;
      }

      var neighbours = Util.getNeighbours(targetObjectState.x, targetObjectState.y);

      // Randomly select a neighbour
      var randomNeighbour = neighbours[Math.floor(Math.random() * neighbours.length)];

      // Convert random neighbour to pixel coordinates
      var randomNeighbourPixel = Util.hex_to_pixel(randomNeighbour.q, randomNeighbour.r);

      // Add unknown unit sprite, not sure why it is offset by 36 compared to the target, must be origin 
      var unknownUnit = this.add.sprite(
        randomNeighbourPixel.x + 36,
        randomNeighbourPixel.y + 36,
        'unknownunit',
      );
      unknownUnit.setDepth(10);

      /*var diffX = (target.x - unknownUnit.x) * 0.5;
      var diffY = (target.y - unknownUnit.y) * 0.5;

      var destX = target.x + 36;
      var destY = target.y + 36;*/

      if (message.state == DEAD) {
        this.updateDeadRenderState(message.target_id, target);
      }

      var tween = this.tweens.add({
        targets: unknownUnit,
        x: target.x + 36,
        y: target.y + 36,
        ease: 'Power2',
        duration: 750,
        onComplete: this.onJumpCompleteUnknownUnit
      });

      tween.play();

      this.showTargetDamage(message.target_id, target, floatingText.targetText, true);
    }
  }

  onJumpComplete(tween, targets) {
    var objectState = Global.objectStates[targets[0].id];

    // Possible that object has been removed, so check if it exists
    if (!objectState) {
      return;
    }

    if (objectState.state == DEAD) {
      this.forceDeadFinalFrame(targets[0].id, targets[0]);
      return;
    }

    var origin = Util.hex_to_pixel(objectState.x, objectState.y);

    var returnTween = this.tweens.add({
      targets: targets[0],
      x: origin.x,
      y: origin.y,
      ease: 'Power2',
      duration: 200,
      onComplete: this.onReturnComplete,
      callbackScope: this
    });

    returnTween.play();

  }

  onJumpCompleteUnknownUnit(tween, targets) {
    targets[0].destroy();
  }

  onReturnComplete(tween, targets) {
    console.log('onReturnComplete');
    var objectState = Global.objectStates[targets[0].id];
    if (objectState && objectState.state == DEAD) {
      this.forceDeadFinalFrame(targets[0].id, targets[0]);
    }
  }

  onMoveComplete(objectId: string) {
    delete this.activeMoveTweens[objectId];

    var objectState = Global.objectStates[objectId];
    console.log(objectState);

    if (!objectState) {
      this.destroyRenderedObject(objectId);
      return;
    }

    if (objectState.subclass == HERO) {
      var mapScene = this.scene.get('MapScene') as MapScene;
      mapScene.cameras.main.stopFollow();

      var weatherScene = this.scene.get('WeatherScene') as WeatherScene;
      weatherScene.cameras.main.stopFollow();

      this.cameras.main.stopFollow();
      this.reconcileRenderedObjects();
      return;
    }

    if (objectState.player != Global.playerId && !Util.isVisible(objectState.x, objectState.y)) {
      this.destroyRenderedObject(objectId);
    }
  }

  onDmgTextComplete(tween, targets) {
    targets[0].destroy();
  }

  processSpoilMessage(message) {
    console.log('Spoil Message: ' + message.source_id + ' -> ' + message.target_id);
    if (message.source_id in this.objectList &&
      message.target_id in this.objectList) {
      var source = this.objectList[message.source_id] as GameSprite;
      var target = this.objectList[message.target_id];

      console.log('Source: ' + source);

      if (source == null || target == null)
        return;


      console.log('Play attack');
      this.playSpriteAction(source, 'attack');

      var diffX = (target.x - source.x) * 0.5;
      var diffY = (target.y - source.y) * 0.5;

      var destX = source.x + diffX;
      var destY = source.y + diffY;

      var tween = this.tweens.add({
        targets: source,
        x: destX,
        y: destY,
        ease: 'Power2',
        duration: 750,
        onComplete: this.onJumpComplete,
        callbackScope: this
      });

      tween.play();
    }

    var dmgMsg = message.itemquantity + ' food';

    var dmgText = this.add.text(target.x + 36, target.y - 5, dmgMsg, { fontFamily: 'Verdana', fontSize: 22, color: '#FFA500', stroke: '#000000', strokeThickness: 4 });
    dmgText.setDepth(10);
    dmgText.setOrigin(0.5, 0.5);

    var textTween = this.tweens.add({
      targets: dmgText,
      y: target.y - 50,
      alpha: 0,
      ease: 'Power1',
      duration: 5000,
      onComplete: this.onSpoilTextComplete
    });

    textTween.play();
  }

  onSpoilTextComplete(tween, targets) {
    targets[0].destroy();
  }

  processStealMessage(message) {
    console.log('Steal Message: ' + message.source_id + ' -> ' + message.target_id);
    if (message.source_id in this.objectList &&
      message.target_id in this.objectList) {
      var source = this.objectList[message.source_id] as GameSprite;
      var target = this.objectList[message.target_id];

      console.log('Source: ' + source);

      if (source == null || target == null)
        return;


      console.log('Play attack');
      this.playSpriteAction(source, 'attack');

      var diffX = (target.x - source.x) * 0.5;
      var diffY = (target.y - source.y) * 0.5;

      var destX = source.x + diffX;
      var destY = source.y + diffY;

      var tween = this.tweens.add({
        targets: source,
        x: destX,
        y: destY,
        ease: 'Power2',
        duration: 750,
        onComplete: this.onJumpComplete,
        callbackScope: this
      });

      tween.play();
    }

    var dmgMsg = 'Steal';

    var dmgText = this.add.text(target.x + 36, target.y - 5, dmgMsg, { fontFamily: 'Verdana', fontSize: 22, color: '#FFCC33', stroke: '#000000', strokeThickness: 4 });
    dmgText.setDepth(10);
    dmgText.setOrigin(0.5, 0.5);

    var textTween = this.tweens.add({
      targets: dmgText,
      y: target.y - 50,
      alpha: 0,
      ease: 'Power1',
      duration: 5000,
      onComplete: this.onStealTextComplete
    });

    textTween.play();
  }

  onStealTextComplete(tween, targets) {
    targets[0].destroy();
  }

  processTorchMessage(message) {
    console.log('Torch Message: ' + message.source_id + ' -> ' + message.target_id);
    if (message.source_id in this.objectList &&
      message.target_id in this.objectList) {
      var source = this.objectList[message.source_id] as GameSprite;
      var target = this.objectList[message.target_id];

      console.log('Source: ' + source);

      if (source == null || target == null)
        return;


      console.log('Play attack');
      this.playSpriteAction(source, 'attack');

      var diffX = (target.x - source.x) * 0.5;
      var diffY = (target.y - source.y) * 0.5;

      var destX = source.x + diffX;
      var destY = source.y + diffY;

      var tween = this.tweens.add({
        targets: source,
        x: destX,
        y: destY,
        ease: 'Power2',
        duration: 750,
        onComplete: this.onJumpComplete,
        callbackScope: this
      });

      tween.play();
    }

    var torchMsg = 'Torch';

    var torchText = this.add.text(target.x + 36, target.y - 5, torchMsg, { fontFamily: 'Verdana', fontSize: 20, color: '#FFCC33' });
    torchText.setDepth(10);
    torchText.setOrigin(0.5, 0.5);

    var textTween = this.tweens.add({
      targets: torchText,
      y: target.y - 50,
      alpha: 0,
      ease: 'Power1',
      duration: 5000,
      onComplete: this.onTorchTextComplete
    });

    textTween.play();
  }

  onTorchTextComplete(tween, targets) {
    targets[0].destroy();
  }

  // Floating "!" alert above an NPC/villager — same animation/timing for all colors.
  showAlertFloater(sourceId, color: string) {
    var npcSprite = this.objectList[sourceId];
    if (!npcSprite) return;
    var alertText = this.add.text(npcSprite.x + 36, npcSprite.y - 5, '!', {
      fontFamily: 'Verdana',
      fontSize: 21,
      fontStyle: 'bold',
      color: color,
      stroke: '#000000',
      strokeThickness: 4,
    });
    alertText.setOrigin(0.5, 0.5);
    alertText.setDepth(25);
    this.tweens.add({
      targets: alertText,
      y: npcSprite.y - 50,
      alpha: 0,
      ease: 'Power1',
      delay: 400,
      duration: 700,
      onComplete: (_tween, targets) => { targets[0].destroy(); },
    });
  }

  processActivityUpdate(message) {
    var prev = this.lastVillagerActivity[message.id];
    this.lastVillagerActivity[message.id] = message.activity;

    var objectState = Global.objectStates[message.id];
    if (objectState) {
      objectState.activity = message.activity;

      var renderObject = this.getRenderedObject(message.id);
      if (objectState.subclass === VILLAGER && renderObject) {
        this.syncVillagerActivityBadge(objectState, renderObject);
      }

      if (
        (objectState.subclass === HERO || objectState.subclass === VILLAGER)
        && (objectState.state === GATHERING || objectState.state === 'refining')
        && renderObject instanceof GameSprite
      ) {
        const animationExists = this.anims.exists(
          objectState.image + '_' + objectState.state,
        );
        this.syncStateTextTimer(
          objectState,
          renderObject,
          objectState.state,
          animationExists,
        );
      }
    }

    // Yellow "!" floater on transition into Fleeing
    if (message.activity === 'Fleeing' && prev !== 'Fleeing') {
      this.showAlertFloater(message.id, '#FFD700');
    }
  }

  processSpeech(message) {
    var objectState = Global.objectStates[message.source];

    // Possible that object has been removed or not visible, so check if it exists
    if (!objectState) {
      return;
    }

    // Special case: "!" alert — floats upward from same position as damage numbers
    if (message.speech === '!') {
      this.showAlertFloater(message.source, '#FF0000');
      return;
    }

    // Prose speech bubbles are rendered by the React HTML overlay
    // (SpeechBubbleLayer), which listens to the same NetworkEvent.SPEECH. Drawing
    // them in HTML rather than in the canvas keeps the text a constant, readable
    // size regardless of the camera zoom.
  }

  onSpeechComplete(tween, targets) {
    targets[0].destroy();
  }

  processSound(message) {
    console.log('Sound Message: ' + JSON.stringify(message));
    var source = Util.hex_to_pixel(message.x, message.y);
    var graphics = this.add.graphics()
    var container = this.add.container(
      source.x + 36 - 60,
      source.y - 20,
    );

    if (message.sound.length < 40) {
      var soundText = this.add.text(60, 20, message.sound, { fontFamily: 'Alegreya', fontSize: 14, color: '#FFFFFF' });
      soundText.setWordWrapWidth(120);
      soundText.setOrigin(0.5, 0.5);
      soundText.setAlign('center');

      container.add(graphics);
      container.add(soundText);
      container.setDepth(20);

      graphics.fillStyle(0x000000, 0.50);
      graphics.fillRoundedRect(0,
        0,
        120,
        40,
        5);

      if (message.sound.length < 5) {
        graphics.setVisible(false);
      }

      var textTween = this.tweens.add({
        targets: container,
        alpha: 0,
        ease: 'Power1',
        delay: 5000,
        duration: 5000,
        onComplete: this.onSpeechComplete
      });

      textTween.play();
    } else if (message.sound.length < 60) {
      var soundText = this.add.text(60, 30, message.sound, { fontFamily: 'Alegreya', fontSize: 14, color: '#FFFFFF' });
      soundText.setWordWrapWidth(120);
      soundText.setOrigin(0.5, 0.5);
      soundText.setAlign('center');

      container.add(graphics);
      container.add(soundText);
      container.setDepth(20);

      graphics.fillStyle(0x000000, 0.50);
      graphics.fillRoundedRect(0,
        0,
        120,
        60,
        5);

      if (message.sound.length < 5) {
        graphics.setVisible(false);
      }

      var textTween = this.tweens.add({
        targets: container,
        alpha: 0,
        ease: 'Power1',
        delay: 5000,
        duration: 5000,
        onComplete: this.onSpeechComplete
      });

      textTween.play();
    } else {
      var soundText = this.add.text(60, 40, message.sound, { fontFamily: 'Alegreya', fontSize: 14, color: '#FFFFFF' });
      soundText.setWordWrapWidth(120);
      soundText.setOrigin(0.5, 0.5);
      soundText.setAlign('center');

      container.add(graphics);
      container.add(soundText);
      container.setDepth(20);

      graphics.fillStyle(0x000000, 0.50);
      graphics.fillRoundedRect(0,
        0,
        120,
        75,
        5);

      if (message.sound.length < 5) {
        graphics.setVisible(false);
      }

      var textTween = this.tweens.add({
        targets: container,
        alpha: 0,
        ease: 'Power1',
        delay: 3000,
        duration: 3000,
        onComplete: this.onSpeechComplete
      });

      textTween.play();
    }
  }

  onSoundComplete(tween, targets) {
    targets[0].destroy();
  }

  processXp(message) {
    var objectState = Global.objectStates[message.id];
    var source = Util.hex_to_pixel(objectState.x, objectState.y);

    for (var i = 0; i < message.xp_list.length; i++) {
      var value = '+' + message.xp_list[i].xp + ' ' + message.xp_list[i].skill + ' XP';

      var xpText = this.add.text(source.x + 36, source.y - 5 - (i * 15), value, { fontFamily: 'Verdana', fontSize: 14, color: '#FFFFFF' });
      xpText.setDepth(10);
      xpText.setOrigin(0.5, 0.5);

      var textTween = this.tweens.add({
        targets: xpText,
        alpha: 0,
        ease: 'Power1',
        duration: 7000,
        onComplete: this.onXpTextComplete
      });

      textTween.play();
    }
  }

  onXpTextComplete(tween, targets) {
    targets[0].destroy();
  }

  processGainedEffect(message) {
    var source = Util.hex_to_pixel(message.x, message.y);

    var textValue = 'Gained ' + message.effect;
    var effectTextDelay = 500;

    // If drawing of map has not be completed yet, delay the effect text
    if (!Global.drawMapCompleted) {
      effectTextDelay = 5000;
    }

    this.time.addEvent({
      delay: effectTextDelay,
      callback: () => {
        var effectText = this.add.text(source.x + 36, source.y - 5 + Global.effectTextOffsetY, textValue, { fontFamily: 'Verdana', fontSize: 14, color: '#00ff00', stroke: '#000', strokeThickness: 2 });
        effectText.setDepth(10);
        effectText.setOrigin(0.5, 0.5);

        // To allow stacking of effect messages
        Global.effectTextOffsetY = 25;

        var textTween = this.tweens.add({
          targets: effectText,
          alpha: 0,
          ease: 'Power1',
          duration: 5000,
          onComplete: this.onEffectTextComplete
        });

        textTween.play();
      },
      loop: false
    });
  }

  processLostEffect(message) {
    var source = Util.hex_to_pixel(message.x, message.y);

    var textValue = 'Lost ' + message.effect;

    this.time.addEvent({
      delay: 500,
      callback: () => {
        var effectText = this.add.text(source.x + 36, source.y - 5 + Global.effectTextOffsetY, textValue, { fontFamily: 'Verdana', fontSize: 14, color: '#FF0000', stroke: '#000', strokeThickness: 2 });
        effectText.setDepth(10);
        effectText.setOrigin(0.5, 0.5);

        // To allow stacking of effect messages
        Global.effectTextOffsetY = 25;

        var textTween = this.tweens.add({
          targets: effectText,
          alpha: 0,
          ease: 'Power1',
          duration: 5000,
          onComplete: this.onEffectTextComplete
        });

        textTween.play();
      },
      loop: false
    });
  }

  processReducedEffect(message) {
    var source = Util.hex_to_pixel(message.x, message.y);

    var textValue = message.label + ' ' + message.effect;

    this.time.addEvent({
      delay: 500,
      callback: () => {
        var effectText = this.add.text(source.x + 36, source.y - 5 + Global.effectTextOffsetY, textValue, { fontFamily: 'Verdana', fontSize: 14, color: '#FF0000', stroke: '#000', strokeThickness: 2 });
        effectText.setDepth(10);
        effectText.setOrigin(0.5, 0.5);

        // To allow stacking of effect messages
        Global.effectTextOffsetY = 25;

        var textTween = this.tweens.add({
          targets: effectText,
          alpha: 0,
          ease: 'Power1',
          duration: 5000,
          onComplete: this.onEffectTextComplete
        });

        textTween.play();
      },
      loop: false
    });
  }

  processIncreasedEffect(message) {
    var source = Util.hex_to_pixel(message.x, message.y);

    var textValue = message.label + ' ' + message.effect;

    this.time.addEvent({
      delay: 500,
      callback: () => {
        var effectText = this.add.text(source.x + 36, source.y - 5 + Global.effectTextOffsetY, textValue, { fontFamily: 'Verdana', fontSize: 14, color: '#00ff00', stroke: '#000', strokeThickness: 2 });
        effectText.setDepth(10);
        effectText.setOrigin(0.5, 0.5);

        // To allow stacking of effect messages
        Global.effectTextOffsetY = 25;

        var textTween = this.tweens.add({
          targets: effectText,
          alpha: 0,
          ease: 'Power1',
          duration: 5000,
          onComplete: this.onEffectTextComplete
        });

        textTween.play();
      },
      loop: false
    });
  }

  onEffectTextComplete(tween, targets) {
    targets[0].destroy();

    // Reset offset
    Global.effectTextOffsetY = 0;
  }

  processTextState(sprite, state, activity?: string) {
    if (sprite == null || sprite.scene == null)
      return;

    var value = objectStateText(state, activity);

    var stateText = this.add.text(sprite.x + 36, sprite.y - 5, value, { fontFamily: 'Verdana', fontSize: 14, color: '#00d2ff' });
    stateText.setDepth(10);
    stateText.setOrigin(0.5, 0.5);

    var textTween = this.tweens.add({
      targets: stateText,
      alpha: 0,
      ease: 'Power1',
      duration: 3000,
      onComplete: this.onTextStateComplete
    });

    textTween.play();
  }

  onTextStateComplete(tween, targets) {
    targets[0].destroy();
  }

  private clearStateTextTimer(objectId: string): void {
    const stateTimer = this.stateTimerList[objectId];
    if (!stateTimer) {
      return;
    }

    clearInterval(stateTimer.timer);
    delete this.stateTimerList[objectId];
  }

  private syncStateTextTimer(
    objectState: ObjectState,
    sprite: GameSprite,
    state: string,
    animationExists: boolean,
  ): void {
    const activity = objectState.subclass === HERO || objectState.subclass === VILLAGER
      ? objectState.activity
      : undefined;
    const value = repeatingObjectStateText(state, animationExists, activity);
    const existing = this.stateTimerList[objectState.id];

    if (existing && (
      existing.state !== state
      || existing.value !== value
      || value === null
    )) {
      this.clearStateTextTimer(objectState.id);
    }

    if (value === null || sprite == null || sprite.scene == null) {
      return;
    }

    if (this.stateTimerList[objectState.id]) {
      return;
    }

    this.processTextState(sprite, state, activity);

    const timer = setInterval(() => {
      const liveState = Global.objectStates[objectState.id];
      const liveSprite = this.getRenderedObject(objectState.id);
      const liveActivity = liveState?.subclass === HERO || liveState?.subclass === VILLAGER
        ? liveState.activity
        : undefined;
      const liveValue = liveState
        ? repeatingObjectStateText(state, animationExists, liveActivity)
        : null;
      if (
        !liveState
        || liveState.state !== state
        || liveValue !== value
        || !(liveSprite instanceof GameSprite)
      ) {
        this.clearStateTextTimer(objectState.id);
        return;
      }

      this.processTextState(liveSprite, state, liveActivity);
    }, 3000);

    this.stateTimerList[objectState.id] = { state, value, timer };
  }

  replaceImageDefTask(objectState) {
    this.queueImageDefTask(objectState);
  }

  processFireAnimationState() {
    const litCampfireAnimationEnabled = isDesktop();

    for (var burningId in this.fireAnimationSprites) {
      var burningState = Global.objectStates[burningId];
      var burningPresentation = fireAnimationPresentation(
        burningState,
        litCampfireAnimationEnabled,
      );
      var baseRenderMissing = burningPresentation != null
        && burningPresentation.kind != 'burning-object'
        && !this.getRenderedObject(burningId);

      if (!burningPresentation || baseRenderMissing || !this.shouldRenderState(burningState)) {
        this.fireAnimationSprites[burningId].destroy();
        delete this.fireAnimationSprites[burningId];
      }
    }

    for (var objectId in Global.objectStates) {
      var objectState = Global.objectStates[objectId] as ObjectState;
      var presentation = fireAnimationPresentation(
        objectState,
        litCampfireAnimationEnabled,
      );

      if (!presentation || !this.shouldRenderState(objectState)) {
        continue;
      }

      if (presentation.kind != 'burning-object' && !this.getRenderedObject(objectId)) {
        continue;
      }

      var origin = Util.hex_to_pixel(objectState.x, objectState.y);
      var burningSprite = this.fireAnimationSprites[objectId];

      if (!burningSprite || burningSprite.scene == null) {
        burningSprite = this.add.sprite(
          origin.x + presentation.offsetX,
          origin.y + presentation.offsetY,
          'burning',
        );
        burningSprite.play({
          key: 'burninganim',
          startFrame: fireAnimationStartFrame(objectId),
        });
        this.fireAnimationSprites[objectId] = burningSprite;
      }

      burningSprite.setPosition(
        origin.x + presentation.offsetX,
        origin.y + presentation.offsetY,
      );
      burningSprite.setDepth(presentation.depth);
      burningSprite.setScale(presentation.scale);
      burningSprite.setAlpha(presentation.alpha);
      burningSprite.setBlendMode(
        presentation.additiveBlend ? Phaser.BlendModes.ADD : Phaser.BlendModes.NORMAL,
      );
    }
  }
}
