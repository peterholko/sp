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
    const config: any = {
      title: "Siege Perilous",
      version: "0.0.1",
      type: Phaser.AUTO,
      parent: "game",
      scale: {
        mode: Phaser.Scale.RESIZE,
        autoCenter: Phaser.Scale.CENTER_BOTH,
        width: '100%',
        height: '100%',
      },
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
    return <div id="game" className={styles.game} />;
  }
}
