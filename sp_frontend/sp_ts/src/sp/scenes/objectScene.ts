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
import { HERO, DEAD, SPRITE, CONTAINER, IMAGE, FOUNDED, WALL, UNIT, STRUCTURE, VILLAGER, HARVESTING, CRAFTING, GATHERING } from '../config';
import { GameImage } from '../objects/gameImage';
import { GameContainer } from '../objects/gameContainer';


export class ObjectScene extends Phaser.Scene {

  private renderToggle = false;

  public objectList = {};

  private imageDefTasks = [];
  private containerTasks = [];

  private shroudTiles = [];

  private wallList: Array<ObjectState> = [];

  private multiImages: Record<string, Array<MultiImage>> = {};

  private stateTimerList = {};
  private burningSprites: Record<string, Phaser.GameObjects.Sprite> = {};

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

    this.load.on('filecomplete', this.fileLoadComplete, this);
    this.load.on('complete', this.loadComplete, this);

    this.time.addEvent({ delay: 1000, callback: this.processBurningState, callbackScope: this, loop: true });

  }

  processImageDefMessage(message) {
    console.log('image_def')

    if (message.result != '404') {
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
            this.load.start();
          }

          this.containerTasks.push(message.name);
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

  drawObjects(objectStates: Record<string, ObjectState>): void {
    console.log('***** drawObjects ******');
    console.log(JSON.stringify(Global.objectStates, null, 2));
    //Clear visibleTiles & shroud
    Global.visibleTiles = [];
    this.clearShroud();

    for (var objectId in objectStates) {
      var objectState = objectStates[objectId] as ObjectState;
      console.log(objectState);

      if (objectState.op == 'added') {
        console.log('Object Added');

        if (Global.imageDefList.hasOwnProperty(objectState.image)) {
          const imageType = Util.getImageType(objectState.image);

          if (imageType == SPRITE) {
            this.addSprite(objectState);
          } else if (imageType == IMAGE) {
            this.addImage(objectState);
          } else if (imageType == CONTAINER) {
            this.addContainer(objectState);
          }
        } else {
          Global.network.sendImageDef(objectState.image);

          this.imageDefTasks.push(objectState);
        }

        Global.objectStates[objectId].op = 'none';
      }
      else if (objectState.op == 'updated') {
        console.log('Object Updated');
        console.log(Global.imageDefList.hasOwnProperty(objectState.image));
        if (Global.imageDefList.hasOwnProperty(objectState.image)) {
          const imageType = Util.getImageType(objectState.image);
          console.log(imageType);
          if (imageType == IMAGE) {
            this.updateImage(objectState);
          } else if (imageType == SPRITE) {
            this.updateSprite(objectState);
          } else if (imageType == CONTAINER) {
            this.updateContainer(objectState);
          }
        } else {
          Global.network.sendImageDef(objectState.image);

          if (objectState.updateAttr == 'state') {
            this.imageDefTasks.push(objectState);
          } else if (objectState.updateAttr == 'template') {
            //Remove old image template
            var obj = this.objectList[objectState.id];
            obj.destroy();

            //Replace old imageDef task with new one
            this.replaceImageDefTask(objectState);
          } else if (objectState.updateAttr == 'image') {
            //Remove old image template
            var obj = this.objectList[objectState.id];
            obj.destroy();

            //Replace old imageDef task with new one
            this.replaceImageDefTask(objectState);
          }
        }
        Global.objectStates[objectId].op = 'none';
      } else if (objectState.op == 'deleted') {
        //console.log('Object deleted');

        // Do not destroy structures that are not visible
        if (objectState.class == 'structure' && !Util.isVisible(objectState.x, objectState.y)) {
          continue;
        }

        var obj = this.objectList[objectState.id];

        if (obj instanceof GameContainer) {
          //console.log('Removing contents of container');
          var f = obj.getAt(0) as Phaser.GameObjects.Image;
          console.log(f);
          f.destroy();
        }

        obj.destroy();

        //Remove from Global States
        delete Global.objectStates[objectId];
      }

      this.processVisibleTiles(objectState);
    }

    console.log(Global.objectStates);

    //Call processWall here for loaded wall images
    this.processWallList();

    //Add Shroud tiles
    this.addShroud();
  }

  processVisibleTiles(objectState: ObjectState) {
    if (objectState.player == Global.playerId) {
      if (objectState.vision > 0) {
        var visibleTiles = Util.range(objectState.x,
          objectState.y,
          objectState.vision);

        Global.visibleTiles = Global.visibleTiles.concat(visibleTiles);
      } else {

        if (objectState.subclass == HERO || objectState.subclass == VILLAGER) {
          // Add current tile to visible tiles
          Global.visibleTiles.push({
            q: objectState.x,
            r: objectState.y
          });
        }
      }
    }
  }

  processWallList() {
    //Hide overlapping containers images
    for (var wallKey in this.wallList) {
      var wall = this.wallList[wallKey];
      var neighbours = Util.getNeighbours(wall.x, wall.y);

      for (var neighbourId in neighbours) {
        var neighbour = neighbours[neighbourId];

        for (var otherId in this.wallList) {
          var other = this.wallList[otherId];

          if ((neighbour.q == other.x) && (neighbour.r == other.y)) {
            var container = this.objectList[wall.id] as GameContainer;

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



  addShroud() {
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

      if (objectState.vision == 0) {

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

      shroud.destroy();
    }

    this.shroudTiles = [];
  }

  updateImage(objectState: ObjectState) {
    console.log('UpdateImage: ' + JSON.stringify(objectState));
    var image = this.objectList[objectState.id] as GameImage;
    var pixel = Util.hex_to_pixel(objectState.x, objectState.y);

    console.log(image);

    //Structure construction complete
    if (objectState.state == 'none') {
      if (objectState.image != image.imageName) {
        image.setTexture(objectState.image);
        image.imageName = objectState.image;
      }
    }
    console.log(pixel);

    if (objectState.class == 'structure') {
      image.setDepth(1);
    } else if (objectState.class == 'unit') {
      image.setDepth(3);
    } else {
      image.setDepth(2);
    }

    //Move completed, add tween to new location
    if (image.x != pixel.x || image.y != pixel.y) {
      var tween = this.tweens.add({
        targets: image,
        x: pixel.x,
        y: pixel.y,
        ease: 'Power1',
        duration: 500,
        onComplete: this.onMoveComplete
      });

      tween.play();
    }
  }

  updateSprite(objectState: ObjectState) {
    var sprite = this.objectList[objectState.id] as GameSprite;
    console.log('updateSprite');
    console.log(sprite);

    var pixel = Util.hex_to_pixel(objectState.x, objectState.y);

    // Guard against destroyed sprites still present in objectList (scene is undefined after destroy)
    const spriteReady = sprite != null && sprite.scene != null;

    if (objectState.state == 'moving') {
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

      if (objectState.prevstate != objectState.state) {
        if (objectState.id in this.stateTimerList) {
          //Clear timer and remove from list
          clearInterval(this.stateTimerList[objectState.id]);
          delete this.stateTimerList[objectState.id];
        }
      }
      console.log(this.anims);
      if (this.anims.exists(anim)) {

        console.log("Playing animation: " + anim);
        console.log(anim);
        if (spriteReady) {
          console.log(typeof sprite);
          console.log(sprite)
          sprite.play(anim);

          if (objectState.state == CRAFTING || objectState.state == GATHERING || objectState.state == HARVESTING) {
            if (!(objectState.id in this.stateTimerList)) {
              this.processTextState(sprite, animState);

              //TODO reconsider if sprite isn't available yet
              // Repeat this every 3 seconds
              var timer = setInterval(() => {
                this.processTextState(sprite, animState)
              }, 3000);

              this.stateTimerList[objectState.id] = timer;
            }
          }
        } else {
          console.log("Error in animations for sprite for obj: " + objectState.id);
        }
      } else {
        console.log('Animation ' + anim + ' does not exist');
        if (objectState.state == DEAD && spriteReady) {
          sprite.setTexture('gravestone');
          sprite.setDepth(2);
        }
        else if (spriteReady && !(objectState.id in this.stateTimerList)) {
          this.processTextState(sprite, animState);

          //TODO reconsider if sprite isn't available yet
          // Repeat this every 3 seconds 
          var timer = setInterval(() => {
            this.processTextState(sprite, animState)
          }, 3000);

          this.stateTimerList[objectState.id] = timer;
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

      if (typeof sprite !== 'undefined') {
        //Move completed, add tween to new location
        if (sprite.x != pixel.x || sprite.y != pixel.y) {
          var tween = this.tweens.add({
            targets: sprite,
            x: pixel.x,
            y: pixel.y,
            ease: 'Power1',
            duration: 500,
            onComplete: this.onMoveComplete
          });

          tween.play();
        }
      }
    }
  }

  updateContainer(objectState: ObjectState) {

    //Structure construction complete
    if (objectState.state == 'none') {
      var multiImageList = this.multiImages[objectState.image];
      var container = this.objectList[objectState.id] as GameContainer;
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

      if (objectState.subclass == WALL) {
        this.wallList.push(objectState);
      }
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
    var spritesToAdd = this.imageDefTasks.filter(obj => obj.image === imageName);

    for (var i = 0; i < spritesToAdd.length; i++) {
      var spriteObj = spritesToAdd[i];
      this.addSprite(spriteObj);
    }
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
      this.anims.create(config);
    }
  }

  addImage(objectState: ObjectState) {
    var pixel = Util.hex_to_pixel(objectState.x, objectState.y);
    var imageName = '';

    if (objectState.state == FOUNDED) {
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

    if (objectState.class == 'structure') {
      image.setDepth(1);
    } else if (objectState.class == 'unit') {
      image.setDepth(3);
    } else {
      image.setDepth(2);
    }

    this.add.existing(image);

    this.objectList[objectState.id] = image;
  }

  addLoadedImages(imageName) {
    var imagesToAdd = this.imageDefTasks.filter(obj => obj.image === imageName);

    for (var i = 0; i < imagesToAdd.length; i++) {
      var imageObj = imagesToAdd[i];
      this.addImage(imageObj);
    }
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

    if (objectState.state == FOUNDED) {
      var image = new GameImage({
        scene: this,
        x: 0,
        y: 0,
        id: objectState.id,
        imageName: "foundation"
      });

      container.add(image);
      console.log(container);
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
        this.wallList.push(objectState);
      }
    }
  }

  addLoadedContainerImages(imageName) {
    var containersToAdd = this.imageDefTasks.filter(obj => obj.image === imageName);

    for (var i = 0; i < containersToAdd.length; i++) {
      var containerObj = containersToAdd[i];
      this.addContainer(containerObj);
    }
  }

  fileLoadComplete(key, type, raw) {
    console.log('Loaded file: ' + key + ', ' + type);
    var imageName = key;
    var imageType = Util.getImageType(imageName);

    if (imageType == SPRITE) {
      this.createSpriteAnimation(imageName);
      this.addLoadedSprites(imageName);
    } else if (imageType == IMAGE) {
      this.addLoadedImages(imageName);
    } else if (imageType == CONTAINER) {
      //this.addLoadedContainers(imageName);
    }

  }

  loadComplete() {
    console.log('loadComplete')
    for (var i = 0; i < this.containerTasks.length; i++) {
      var containerName = this.containerTasks[i];

      var containersToAdd = this.imageDefTasks.filter(obj => obj.image === containerName);

      for (var j = 0; j < containersToAdd.length; j++) {
        var objState = containersToAdd[j] as ObjectState;
        this.addContainer(objState);
      }
    }

    //Clear container Tasks
    this.containerTasks = [];

    //Call process wall for images finished loading
    this.processWallList();

    Global.gameEmitter.emit(GameEvent.LOADING_FINISHED, {});
  }

  processDmgMessage(message) {
    console.log('Dmg Message: ' + message.source_id + ' -> ' + message.target_id);
    if (message.source_id in Global.objectStates && message.target_id in Global.objectStates) {
      if (message.source_id in this.objectList &&
        message.target_id in this.objectList) {
        var source = this.objectList[message.source_id] as GameSprite;
        var target = this.objectList[message.target_id];

        console.log('Source: ' + source);

        if (source == null || target == null)
          return;

        // What is this for? Is this in the wrong spot
        if (Global.objectStates[message.source_id].subclass == HERO) {

          var mapScene = this.scene.get('MapScene') as MapScene;
          mapScene.cameras.main.stopFollow();
          this.cameras.main.stopFollow();
        }

        //TODO Check subclass 
        if (message.state == DEAD) {
          var anim = target.imageName + '_die';

          //Set object state to dead because an update is not sent to save on messages
          //Global.objectStates[message.target_id].state = DEAD;


          if (Global.objectStates[message.target_id].class == UNIT) {

            if (this.anims.exists(anim)) {
              console.log("Playing Die animation from Damage");
              target.play(target.imageName + '_die');
            } else {
              target.setTexture('gravestone');
            }
          } else if (Global.objectStates[message.target_id].class == STRUCTURE) {
            target.removeAll(true);

            var image = new GameImage({
              scene: this,
              x: 0,
              y: 0,
              id: message.target_id,
              imageName: "foundation"
            });

            target.add(image);
          }
        }

        if (message.attack_type == 'Shadow Bolt') {
          source.play(source.imageName + '_cast');
          source.anims.chain(source.imageName + '_none');

          var shadowBolt = this.add.sprite(source.x + 36, source.y + 36, 'shadowbolt');
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

        } else {
          console.log('Play attack');
          source.play(source.imageName + '_attack');
          source.anims.chain(source.imageName + '_none');

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
            onComplete: this.onJumpComplete
          });

          tween.play();
        }

        var dmgMsg = ''
        if ('combo' in message) {
          dmgMsg = message.combo + ' ' + message.dmg + '!';
        } else {
          dmgMsg = message.dmg;
        }

        var dmgText = this.add.text(target.x + 36, target.y - 5, dmgMsg, { fontFamily: 'Verdana', fontSize: 22, color: '#FF0000', stroke: '#000000', strokeThickness: 4 });
        dmgText.setDepth(10);
        dmgText.setOrigin(0.5, 0.5);

        var textTween = this.tweens.add({
          targets: dmgText,
          y: target.y - 50,
          alpha: 0,
          ease: 'Power1',
          duration: 5000,
          onComplete: this.onDmgTextComplete
        });

        textTween.play();
      }
    } else if (message.target_id in Global.objectStates) {
      var targetObjectState = Global.objectStates[message.target_id];
      var target = this.objectList[message.target_id];

      var neighbours = Util.getNeighbours(targetObjectState.x, targetObjectState.y);

      // Randomly select a neighbour
      var randomNeighbour = neighbours[Math.floor(Math.random() * neighbours.length)];

      // Convert random neighbour to pixel coordinates
      var randomNeighbourPixel = Util.hex_to_pixel(randomNeighbour.q, randomNeighbour.r);

      // Add unknown unit sprite, not sure why it is offset by 36 compared to the target, must be origin 
      var unknownUnit = this.add.sprite(randomNeighbourPixel.x + 36, randomNeighbourPixel.y + 36, 'unknownunit');
      unknownUnit.setDepth(10);

      /*var diffX = (target.x - unknownUnit.x) * 0.5;
      var diffY = (target.y - unknownUnit.y) * 0.5;

      var destX = target.x + 36;
      var destY = target.y + 36;*/

      if (message.state == DEAD) {
        var anim = target.imageName + '_die';

        //Set object state to dead because an update is not sent to save on messages
        //Global.objectStates[message.target_id].state = DEAD;


        if (Global.objectStates[message.target_id].class == UNIT) {

          if (this.anims.exists(anim)) {
            console.log("Playing Die animation from Damage");
            target.play(target.imageName + '_die');
          } else {
            target.setTexture('gravestone');
          }
        } else if (Global.objectStates[message.target_id].class == STRUCTURE) {
          target.removeAll(true);

          var image = new GameImage({
            scene: this,
            x: 0,
            y: 0,
            id: message.target_id,
            imageName: "foundation"
          });

          target.add(image);
        }
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

      var dmgMsg = ''
      if ('combo' in message) {
        dmgMsg = message.combo + ' ' + message.dmg + '!';
      } else {
        dmgMsg = message.dmg;
      }

      var dmgText = this.add.text(target.x + 36, target.y - 5, dmgMsg, { fontFamily: 'Verdana', fontSize: 20, color: '#FF0000' });
      dmgText.setDepth(10);
      dmgText.setOrigin(0.5, 0.5);

      var textTween = this.tweens.add({
        targets: dmgText,
        y: target.y - 50,
        alpha: 0,
        ease: 'Power1',
        duration: 5000,
        onComplete: this.onDmgTextComplete
      });

      textTween.play();
    }
  }

  onJumpComplete(tween, targets) {
    var objectState = Global.objectStates[targets[0].id];

    // Possible that object has been removed, so check if it exists
    if (!objectState) {
      return;
    }

    var origin = Util.hex_to_pixel(objectState.x, objectState.y);

    var returnTween = this.tweens.add({
      targets: targets[0],
      x: origin.x,
      y: origin.y,
      ease: 'Power2',
      duration: 200,
      onComplete: this.onReturnComplete
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
      targets[0].anims.stop();
      console.log('Setting to gravestone');
      targets[0].setTexture('gravestone');
    }
  }

  onMoveComplete(tween, targets) {
    var sprite = targets[0]
    var objectState = Global.objectStates[sprite.id];
    console.log(objectState);

    if (objectState) {
      if (objectState.subclass == HERO) {

        var mapScene = this.scene.get('MapScene') as MapScene;
        mapScene.cameras.main.stopFollow();

        var weatherScene = this.scene.get('WeatherScene') as WeatherScene;
        weatherScene.cameras.main.stopFollow();

        this.cameras.main.stopFollow();

        for (var targetId in Global.objectStates) {
          var otherState = Global.objectStates[targetId];

          if (Util.isVisible(otherState.x, otherState.y) == false) {
            // Do not destroy structures or points of interest that are not visible
            if (otherState.class == 'structure' || otherState.class == 'poi') {
              continue;
            }

            var otherSprite = this.objectList[targetId];
            otherSprite.destroy();

            // Added Nov 2023, the objectStates has to be deleted otherwise the next time the object return 
            // it will not be displayed
            delete Global.objectStates[targetId];
          }

        }
      } else if (objectState.player != Global.playerId) {

        if (Util.isVisible(objectState.x, objectState.y) == false) {
          sprite.destroy();

          // Added Nov 2023, the objectStates has to be deleted otherwise the next time the object return 
          // it will not be displayed
          delete Global.objectStates[sprite.id];
        }
      }
    } else {
      // ObjectState has already been deleted, sprite should be destroyed;  
      sprite.destroy();
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
      source.play(source.imageName + '_attack');
      source.anims.chain(source.imageName + '_none');

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
        onComplete: this.onJumpComplete
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
      source.play(source.imageName + '_attack');
      source.anims.chain(source.imageName + '_none');

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
        onComplete: this.onJumpComplete
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
      source.play(source.imageName + '_attack');
      source.anims.chain(source.imageName + '_none');

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
        onComplete: this.onJumpComplete
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

  processSpeech(message) {
    var objectState = Global.objectStates[message.source];

    // Possible that object has been removed or not visible, so check if it exists
    if (!objectState) {
      return;
    }

    // Special case: "!" alert — floats upward from same position as damage numbers
    if (message.speech === '!') {
      var npcSprite = this.objectList[message.source];
      if (!npcSprite) return;
      var alertText = this.add.text(npcSprite.x + 36, npcSprite.y - 5, '!', {
        fontFamily: 'Verdana',
        fontSize: 21,
        fontStyle: 'bold',
        color: '#FFD700',
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
      return;
    }

    var source = Util.hex_to_pixel(objectState.x, objectState.y);
    var graphics = this.add.graphics()
    var container = this.add.container(source.x - 24, source.y - 20);

    if (message.speech.length < 40) {
      var speechText = this.add.text(60, 20, message.speech, { fontFamily: 'Alegreya', fontSize: 14, color: '#FFFFFF' });
      speechText.setWordWrapWidth(120);
      speechText.setOrigin(0.5, 0.5);
      speechText.setAlign('center');

      container.add(graphics);
      container.add(speechText);
      container.setDepth(20);

      graphics.fillStyle(0x000000, 0.50);
      graphics.fillRoundedRect(0,
        0,
        120,
        40,
        5);

      if (message.speech.length < 5) {
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
    } else if (message.speech.length < 60) {
      var speechText = this.add.text(60, 30, message.speech, { fontFamily: 'Alegreya', fontSize: 14, color: '#FFFFFF' });
      speechText.setWordWrapWidth(120);
      speechText.setOrigin(0.5, 0.5);
      speechText.setAlign('center');

      container.add(graphics);
      container.add(speechText);
      container.setDepth(20);

      graphics.fillStyle(0x000000, 0.50);
      graphics.fillRoundedRect(0,
        0,
        120,
        60,
        5);

      if (message.speech.length < 5) {
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
      var speechText = this.add.text(60, 40, message.speech, { fontFamily: 'Alegreya', fontSize: 14, color: '#FFFFFF' });
      speechText.setWordWrapWidth(120);
      speechText.setOrigin(0.5, 0.5);
      speechText.setAlign('center');

      container.add(graphics);
      container.add(speechText);
      container.setDepth(20);

      graphics.fillStyle(0x000000, 0.50);
      graphics.fillRoundedRect(0,
        0,
        120,
        75,
        5);

      if (message.speech.length < 5) {
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

  onSpeechComplete(tween, targets) {
    targets[0].destroy();
  }

  processSound(message) {
    console.log('Sound Message: ' + JSON.stringify(message));
    var source = Util.hex_to_pixel(message.x, message.y);
    var graphics = this.add.graphics()
    var container = this.add.container(source.x - 24, source.y - 20);

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

  processTextState(sprite, state) {
    if (sprite == null)
      return;

    var value = '';

    if (state == 'sleeping') {
      value = '...zzzZZZ';
    } else {
      value = '* ' + state + ' *';
    }

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

  replaceImageDefTask(objectState) {
    //imageDefTasks could be replaced with a map to increase performance
    for (var i = 0; i < this.imageDefTasks.length; i++) {
      if (objectState.id == this.imageDefTasks[i].id) {
        this.imageDefTasks[i] = objectState;
        break;
      }
    }
  }

  processBurningState() {
    // Loop through Global.objectStates dictionary
    for (var objectId in Global.objectStates) {
      var objectState = Global.objectStates[objectId] as ObjectState;

      if (objectState.state == 'burning') {
        // If the object is already in the burningSprites dictionary, skip
        if (this.burningSprites[objectId]) {
          continue;
        }

        var origin = Util.hex_to_pixel(objectState.x, objectState.y);

        var burningSprite = this.add.sprite(origin.x + 36, origin.y + 36, 'burning');
        burningSprite.setDepth(10);
        burningSprite.play('burninganim');

        this.burningSprites[objectId] = burningSprite;
      } else {

        // Loop through the burningSprites dictionary and remove the burning animation if the object is not burning
        for (var keySprite in this.burningSprites) {
          if (objectId === keySprite) {
            var burningSprite = this.burningSprites[keySprite];
            burningSprite.destroy();
            delete this.burningSprites[keySprite];
          }
        }
      }
    }
  }
}