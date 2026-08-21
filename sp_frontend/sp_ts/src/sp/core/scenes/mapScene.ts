/**
 * @author       Peter Holko
 * @copyright    2018 - 2019 Peter Holko
 */

import { Util } from '../util';
import { Global } from '../global';
import { Tile } from '../objects/tile';
import { GameSprite } from '../objects/gameSprite';
import { GameEvent } from '../gameEvent';
import { NetworkEvent } from '../networkEvent';
import { ObjectState } from '../objectState';
import { TileState } from '../tileState';
import { desktopCameraZoom } from '../config';
import { MAP_RENDER_EVENTS } from '../mapRenderEvents';
import {
  groupScoutedResourceCategories,
  resourceLayerToggleAction,
  SCOUT_CATEGORY_ICON_KEYS,
  SCOUT_CATEGORY_ICON_SIZE,
  scoutCategoryIconOffset,
} from '../scoutResourceCategories';

export class MapScene extends Phaser.Scene {

  private forests = [18, 19, 20, 21, 22, 24, 25, 26, 27, 28, 29, 30, 31];
  private renderToggle = false;

  private base : Phaser.GameObjects.Container;
  private trans : Phaser.GameObjects.Container;
  private extra : Phaser.GameObjects.Container;
  private void : Phaser.GameObjects.Container;
  private select : Phaser.GameObjects.Container;
  private resources : Phaser.GameObjects.Container;

  private selectHex : Phaser.GameObjects.Image;

  public mapTiles = {};

  public centerOn(x, y) : void {
    this.cameras.main.centerOn(x, y);
  }

  constructor() {
    super({
      key: "MapScene"
    });
  }

  preload(): void {
    console.log('Preload');

    //this.load.image('selecthex', './static/art/hover-hex.png');

    this.load.image('snow-n', './static/art/tileset/frozen/snow-n.png');
    this.load.image('snow-nw', './static/art/tileset/frozen/snow-nw.png');
    this.load.image('snow-sw', './static/art/tileset/frozen/snow-sw.png');
    this.load.image('snow-s', './static/art/tileset/frozen/snow-s.png');
    this.load.image('snow-se', './static/art/tileset/frozen/snow-se.png');
    this.load.image('snow-ne', './static/art/tileset/frozen/snow-ne.png');

    this.load.image('bankice-n', './static/art/tileset/flat/bank-to-ice-n.png');
    this.load.image('bankice-nw', './static/art/tileset/flat/bank-to-ice-nw.png');
    this.load.image('bankice-sw', './static/art/tileset/flat/bank-to-ice-sw.png');
    this.load.image('bankice-s', './static/art/tileset/flat/bank-to-ice-s.png');
    this.load.image('bankice-se', './static/art/tileset/flat/bank-to-ice-se.png');
    this.load.image('bankice-ne', './static/art/tileset/flat/bank-to-ice-ne.png');

    this.load.image('desert-n', './static/art/tileset/sand/desert-n.png');
    this.load.image('desert-nw', './static/art/tileset/sand/desert-nw.png');
    this.load.image('desert-sw', './static/art/tileset/sand/desert-sw.png');
    this.load.image('desert-s', './static/art/tileset/sand/desert-s.png');
    this.load.image('desert-se', './static/art/tileset/sand/desert-se.png');
    this.load.image('desert-ne', './static/art/tileset/sand/desert-ne.png');

    this.load.image('plains-n', './static/art/tileset/grass/dry-abrupt-n.png');
    this.load.image('plains-nw', './static/art/tileset/grass/dry-abrupt-nw.png');
    this.load.image('plains-sw', './static/art/tileset/grass/dry-abrupt-sw.png');
    this.load.image('plains-s', './static/art/tileset/grass/dry-abrupt-s.png');
    this.load.image('plains-se', './static/art/tileset/grass/dry-abrupt-se.png');
    this.load.image('plains-ne', './static/art/tileset/grass/dry-abrupt-ne.png');

    this.load.image('grass-n', './static/art/tileset/grass/green-abrupt-n.png');
    this.load.image('grass-nw', './static/art/tileset/grass/green-abrupt-nw.png');
    this.load.image('grass-sw', './static/art/tileset/grass/green-abrupt-sw.png');
    this.load.image('grass-s', './static/art/tileset/grass/green-abrupt-s.png');
    this.load.image('grass-se', './static/art/tileset/grass/green-abrupt-se.png');
    this.load.image('grass-ne', './static/art/tileset/grass/green-abrupt-ne.png');

    this.load.image('hillssnow-n', './static/art/tileset/hills/snow-n.png');
    this.load.image('hillssnow-nw', './static/art/tileset/hills/snow-nw.png');
    this.load.image('hillssnow-sw', './static/art/tileset/hills/snow-sw.png');
    this.load.image('hillssnow-s', './static/art/tileset/hills/snow-s.png');
    this.load.image('hillssnow-se', './static/art/tileset/hills/snow-se.png');
    this.load.image('hillssnow-ne', './static/art/tileset/hills/snow-ne.png');

    this.load.image('hillsgrass-n', './static/art/tileset/hills/regular-n.png');
    this.load.image('hillsgrass-nw', './static/art/tileset/hills/regular-nw.png');
    this.load.image('hillsgrass-sw', './static/art/tileset/hills/regular-sw.png');
    this.load.image('hillsgrass-s', './static/art/tileset/hills/regular-s.png');
    this.load.image('hillsgrass-se', './static/art/tileset/hills/regular-se.png');
    this.load.image('hillsgrass-ne', './static/art/tileset/hills/regular-ne.png');

    this.load.image('void-n', './static/art/tileset/void/void-n.png');
    this.load.image('void-nw', './static/art/tileset/void/void-nw.png');
    this.load.image('void-sw', './static/art/tileset/void/void-sw.png');
    this.load.image('void-s', './static/art/tileset/void/void-s.png');
    this.load.image('void-se', './static/art/tileset/void/void-se.png');
    this.load.image('void-ne', './static/art/tileset/void/void-ne.png');
    this.load.image('void', './static/art/tileset/void/void.png');
    //this.load.image('rain', './static/art/rain3.png');
    //this.load.image('snow', './static/art/snow.png');
    //this.load.image('alphamask', './static/art/alphamask.png');


    this.load.image('scout-resource-ore', './static/art/ui/resource_categories/ore.png');
    this.load.image('scout-resource-stone', './static/art/ui/resource_categories/stone.png');
    this.load.image('scout-resource-timber', './static/art/ui/resource_categories/timber.png');
    this.load.image('scout-resource-forage', './static/art/ui/resource_categories/forage.png');
    this.load.image('scout-resource-water', './static/art/ui/resource_categories/water.png');
    this.load.image('scout-resource-fish', './static/art/ui/resource_categories/fish.png');
    this.load.image('scout-resource-game', './static/art/ui/resource_categories/game.png');

    this.load.json('tileset', './static/tileset.json');
    this.load.on('filecomplete-json-tileset', this.tilesetComplete, this)

    this.load.on('complete', this.loadingComplete, this);
  }

  tilesetComplete(k, file): void {
    console.log('Tileset Complete');
    let tilesetJSON = this.cache.json.get('tileset');
    console.log(tilesetJSON);
  
    for(var key in tilesetJSON) {
      var tileData = tilesetJSON[key];
      Global.tileset[tileData.tile] = tileData;

      this.load.image('tileset' + tileData.tile, './static/art/' + tileData.image);
    }
  }

  loadingComplete() {
    console.log('Loading complete');
    for (const event of MAP_RENDER_EVENTS) {
      Global.gameEmitter.on(event, this.setRender, this);
    }
    Global.gameEmitter.on(NetworkEvent.NEARBY_RESOURCES, this.processNearbyResources, this);
    Global.gameEmitter.on(GameEvent.RESOURCE_LAYER_CLICK, this.toggleResourceLayer, this);
    Global.gameEmitter.on(GameEvent.SELECTED_OBJ_MOVED, this.selectedObjMoved, this);
    
    
    this.time.addEvent({ delay: 1000, callback: this.processRender, callbackScope: this, loop: true });
    //this.setRender();
  }

  create(): void {
    console.log('Map Scene Create');

    this.cameras.main.setZoom(desktopCameraZoom());
    Global.gameEmitter.on(GameEvent.CAMERA_ZOOM, (data) => {
      const duration = data.duration ?? 0;
      if (duration > 0) {
        this.cameras.main.zoomTo(data.zoom, duration, 'Sine.easeInOut');
      } else {
        this.cameras.main.setZoom(data.zoom);
      }
    }, this);

    this.base = this.add.container(0, 0);
    this.trans = this.add.container(0, 0);
    this.extra = this.add.container(0, 0);
    this.void = this.add.container(0, 0);
    this.resources = this.add.container(0, 0);
    this.resources.setDepth(1);
    
    this.select = this.add.container(0, 0);
    this.select.setDepth(10); // This doesn't work to bring the select hex over objs

    this.selectHex = new Phaser.GameObjects.Image(this, 0, 0, 'selecthex');
    this.selectHex.setOrigin(0);
    this.select.add(this.selectHex);

    var _this = this;

    this.input.on('gameobjectdown', function(pointer, gameObject) {
      if(pointer.downElement instanceof HTMLCanvasElement) {
        _this.selectHex.x = gameObject.x;
        _this.selectHex.y = gameObject.y;

        console.log('Tile Click: ' + gameObject.x + ',' + gameObject.y);

        Global.gameEmitter.emit(GameEvent.TILE_CLICK, gameObject);
      }
    });
  }

  update() : void {
  }

  processRender() : void {
    //console.log('processRender' + this.renderToggle);
    if(this.renderToggle) {
      this.drawMap();
      this.renderToggle = false;
    }
  }

  processNearbyResources(message) : void {
    const groups = groupScoutedResourceCategories(
      Array.isArray(message?.data) ? message.data : [],
    );

    this.resources.removeAll(true);

    for (const group of groups) {
      const pixel = Util.hex_to_pixel(group.x, group.y);
      group.categories.forEach((category, index) => {
        const offset = scoutCategoryIconOffset(index, group.categories.length);
        const icon = this.add.image(
          pixel.x + 36 + offset.x,
          pixel.y + 12 + offset.y,
          SCOUT_CATEGORY_ICON_KEYS[category],
        );
        icon
          .setOrigin(0.5)
          .setDisplaySize(SCOUT_CATEGORY_ICON_SIZE, SCOUT_CATEGORY_ICON_SIZE);
        this.resources.add(icon);
      });
    }

    const hasScoutedResources = this.resources.length > 0;
    this.resources.visible = hasScoutedResources;
    Global.resourceLayerVisible = hasScoutedResources;
  }

  toggleResourceLayer() : void {
    const action = resourceLayerToggleAction(
      Global.resourceLayerVisible,
      this.resources.length > 0,
    );

    if (action === 'request') {
      Global.network.sendNearbyResources();
      return;
    }

    const visible = action === 'show';
    this.resources.visible = visible;
    Global.resourceLayerVisible = visible;
  }

  // This event handler is required to move the tile select asset
  selectedObjMoved(objMovedEvent) : void {
    console.log(objMovedEvent);
    var pixel = Util.hex_to_pixel(objMovedEvent.hexX, objMovedEvent.hexY);
    console.log('selectedObjMoved - pixel: ' + pixel.x + ',' + pixel.y);
    this.selectHex.x = pixel.x;
    this.selectHex.y = pixel.y;

    var all_tiles = this.base.getAll();
    var new_selected_tile;

    for(var i = 0; i < all_tiles.length; i++) {
      var tile = all_tiles[i] as Tile;

      if(tile.hexX == objMovedEvent.hexX && tile.hexY == objMovedEvent.hexY) {
        new_selected_tile = tile;

        Global.gameEmitter.emit(GameEvent.TILE_CLICK, new_selected_tile);
      }
    }
  }

  setRender() : void {
    this.renderToggle = true
  }

  drawMap() : void {
    console.log('drawMap');
    this.base.removeAll(true);
    this.trans.removeAll(true);
    this.extra.removeAll(true);
    this.void.removeAll(true);

    Global.voidTiles = [];
    var tileArray = [];
        
    for(var index in Global.tileStates) {
      tileArray.push(Global.tileStates[index]);
    }

    tileArray.sort(function(a,b) {return (a.y > b.y) ? 1 : ((b.y > a.y) ? -1 : 0);} );     

    //Base layer
    for(var i = 0; i < tileArray.length; i++) {
      var tileState = tileArray[i];

      for(var j = 0; j < tileState.tiles.length; j++) {
        var tileTypeId = tileState.tiles[j];
        var imageName = 'tileset'  + tileTypeId;

        if(tileTypeId < 18) {
          var offsetX = Global.tileset[tileTypeId].offsetx;
          var offsetY = Global.tileset[tileTypeId].offsety;
          this.addToBase(tileState, 'tileset' + tileTypeId, offsetX, offsetY);
        } else if(this.forests.indexOf(tileTypeId) == -1) {
          //Default to grass as base tile
          this.addToBase(tileState, 'tileset1', 0, 0);
        }
      }
    }

    //Trans Layer
    for(var i = 0; i < tileArray.length; i++) {
      var tileState = tileArray[i];
      var tileType = tileState.tiles[tileState.tiles.length - 1];

      if(tileType == 3 || tileType == 4 || tileType == 5 || tileType == 17) { //Water
        var neighbours = Util.getNeighbours(tileState.hexX, tileState.hexY);

        for(var neighbourId in neighbours) {
          var neighbour = neighbours[neighbourId];
          var otherTile = Util.getTileAt(neighbour.q, neighbour.r);

          if(otherTile == false)
            continue;

          // Last tile type draw is assumed to be the tile type
          var otherTileType = otherTile.tiles[otherTile.tiles.length - 1];

          if(otherTileType != 3 && otherTileType != 4 && otherTileType != 5 && otherTileType != 17) {
            if(otherTileType == 2 || otherTileType == 16 || otherTileType == 33 || 
               otherTileType == 37 || otherTileType == 38) {
                this.addToTrans(tileState, 'snow-' + neighbour.d, 0, 0);
            } else if(otherTileType == 6 || otherTileType == 7 || otherTileType == 8 || otherTileType == 9) {
                this.addToTrans(tileState, 'bankice-' + neighbour.d, 0, 0);
                this.addToTrans(tileState, 'plains-' + neighbour.d, 0, 0);
            } else if(otherTileType == 10 || otherTileType == 12) {
                this.addToTrans(tileState, 'desert-' + neighbour.d, 0, 0);
            } else {
                this.addToTrans(tileState, 'bankice-' + neighbour.d, 0, 0);
                this.addToTrans(tileState, 'grass-' + neighbour.d, 0, 0);
            }
          }  
        }
      } else if(tileType == 2 || tileType == 16) {
        var neighbours = Util.getNeighbours(tileState.hexX, tileState.hexY);

        for(var neighbourId in neighbours) {
          var neighbour = neighbours[neighbourId];
          var otherTile = Util.getTileAt(neighbour.q, neighbour.r);

          if(otherTile == false)
            continue;

          var otherTileType = otherTile.tiles[otherTile.tiles.length - 1];

          if(otherTileType == 6 || otherTileType == 7 || otherTileType == 8 || otherTileType == 9) {
            this.addToTrans(tileState, 'plains-' + neighbour.d, 0, 0);
          } else if(otherTileType == 1) {
            this.addToTrans(tileState, 'grass-' + neighbour.d, 0, 0);
          } else if(otherTileType == 17) {
            this.addToTrans(tileState, 'hillssnow-' + neighbour.d, 0, 0);
          }
          
        }
      } else if(tileType == 1) {
        var neighbours = Util.getNeighbours(tileState.hexX, tileState.hexY);

        for(var neighbourId in neighbours) {
          var neighbour = neighbours[neighbourId];
          var otherTile = Util.getTileAt(neighbour.q, neighbour.r);

          if(otherTile == false)
            continue;

          var otherTileType = otherTile.tiles[otherTile.tiles.length - 1];

          if(otherTileType == 13) {
            this.addToTrans(tileState, 'hillsgrass-' + neighbour.d, 0, 0);
          }
        }
      }
    }

    //Extra layer
    for(var i = 0; i < tileArray.length; i++) {
      var tileState = tileArray[i];

      for(var j = 0; j < tileState.tiles.length; j++) {
        var tileTypeId = tileState.tiles[j];
        var imageName = 'tileset'  + tileTypeId;

        //tileTypeIds under 18 have already been drawn above
        if(tileTypeId < 18)
          continue;

        var offsetX = Global.tileset[tileTypeId].offsetx;
        var offsetY = Global.tileset[tileTypeId].offsety;

        this.addToExtra(tileState, imageName, offsetX, offsetY);
      }
    }
    
    var directions = ['n', 'ne', 'nw', 's', 'se', 'sw'];

    for(var index in Global.tileStates) {
      var existingTile = Global.tileStates[index];
      var neighbours = Util.getNeighbours(existingTile.hexX, existingTile.hexY);

      var tileNeighbours = []

      for(var neighbourId in neighbours) {
        var neighbour = neighbours[neighbourId];

        var neighbourTile = Util.getTileAt(neighbour.q, neighbour.r);

        if(neighbourTile == false)
          tileNeighbours.push(neighbour);
      }

      if(tileNeighbours.length > 0) {
        for(var i = 0; i < tileNeighbours.length; i++) {
          const neighbour: { q: number; r: number; d: string; } = tileNeighbours[i];

          this.addToVoid(existingTile.hexX, existingTile.hexY, 'void-' + neighbour.d);
          this.addToVoid(neighbour.q, neighbour.r, 'void');
        }
      }
    }

    // Toggle the initial drawMap completed
    Global.drawMapCompleted = true;
  }

  addToBase(tileState, tileKey, offsetX, offsetY) {
    var pixel = Util.hex_to_pixel(tileState.hexX, tileState.hexY);

    var mapTile = new Tile({
          scene: this,
          x: pixel.x + Number(offsetX),
          y: pixel.y - Number(offsetY),
          key: tileKey,
          hexX: tileState.hexX,
          hexY: tileState.hexY
        });

    mapTile.setInteractive();

    this.base.add(mapTile);  
  }

  addToTrans(tileState, tileKey, offsetX, offsetY) {
    var pixel = Util.hex_to_pixel(tileState.hexX, tileState.hexY);

    var mapTile = new Tile({
          scene: this,
          x: pixel.x + Number(offsetX),
          y: pixel.y - Number(offsetY),
          key: tileKey,
          hexX: tileState.hexX,
          hexY: tileState.hexY
        });

    this.trans.add(mapTile);  
  
  }
  addToExtra(tileState, tileKey, offsetX, offsetY) {
    var pixel = Util.hex_to_pixel(tileState.hexX, tileState.hexY);

    var mapTile = new Tile({
          scene: this,
          x: pixel.x + Number(offsetX),
          y: pixel.y - Number(offsetY),
          key: tileKey,
          hexX: tileState.hexX,
          hexY: tileState.hexY
        });

    this.extra.add(mapTile);  
  
  }

  addToVoid(hexX, hexY, tileKey) {
    Global.voidTiles.push({
      hexX: hexX,
      hexY: hexY,
    });

    var pixel = Util.hex_to_pixel(hexX, hexY);

    var mapTile = new Tile({
          scene: this,
          x: pixel.x,
          y: pixel.y,
          key: tileKey,
          hexX: hexX,
          hexY: hexY
        });

    this.void.add(mapTile);
  }
}
