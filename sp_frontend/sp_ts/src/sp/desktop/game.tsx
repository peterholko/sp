/**
 * @author       Peter Holko
 * @copyright    2018 - 2019 Peter Holko
 */

import Phaser from "phaser";
import { ObjectScene } from '../core/scenes/objectScene';
import { MapScene } from '../core/scenes/mapScene';
import { WeatherScene } from "../core/scenes/weatherScene";
import { Global } from '../core/global';

import * as React from "react";
import styles from "./app.module.css";

import { GAME_HEIGHT, GAME_WIDTH, isDesktop, DESKTOP_CANVAS_WIDTH, DESKTOP_CANVAS_HEIGHT, getDesktopCanvasSize } from "../core/config";
import { GameEvent } from "../core/gameEvent";


document.addEventListener("visibilitychange", function () {
  if (document.visibilityState === 'visible') {
    console.log("Tab visisble");
    Global.gameEmitter.emit(GameEvent.VISIBLE, {});
  } else {
    console.log("Tab no longer visisble");
  }
});

export default class Game extends React.Component {
  componentDidMount() {
    const desktop = isDesktop();
    const { width: dw, height: dh } = getDesktopCanvasSize();
    if (desktop) {
      document.documentElement.style.setProperty('--sp-canvas-w', dw + 'px');
      document.documentElement.style.setProperty('--sp-canvas-h', dh + 'px');
    }
    const config: any = {
      title: "Siege Perilous",
      version: "0.0.1",
      width: desktop ? dw : window.innerWidth,
      height: desktop ? dh : window.innerHeight,
      type: Phaser.AUTO,
      parent: "game",
      scene: [MapScene, ObjectScene, WeatherScene],
      input: {
        mouse: true
      },
      render: { pixelArt: true },
      fx: {
        glow: {
          distance: 32,
          quality: 0.1
        }
      }
    };

    new Phaser.Game(config);
  }

  shouldComponentUpdate() {
    return false;
  }

  public render() {
    const className = isDesktop() ? `${styles.game} ${styles.gameDesktop}` : styles.game;
    return <div id="game" className={className} />;
  }
}

/*export function getTileAt(hexX, hexY) {
  var key = hexX + '_' + hexY;

  if (key in Global.tileStates) {
    return Global.tileStates[key];
  } else {
    return false;
  }
}*/