import React, { Component, useEffect } from "react";
import { Provider } from "react-redux";
import WebFont from 'webfontloader';

import store from "../core/store";
import LoginControl from "./login";
import { Global } from '../core/global';

Global.gameEmitter = new Phaser.Events.EventEmitter();
Global.uiEmitter = new Phaser.Events.EventEmitter();

class App extends Component {
  componentDidMount() {
    // Load the font using WebFont loader
    WebFont.load({
      google: {
        families: ['Almendra SC', 'Cinzel', 'IM Fell English', 'Uncial Antiqua'], // Specify the font to load
      },
    });
  }

  render() {
    return (
      <Provider store={store}>
        <div>
          <LoginControl />
        </div>
      </Provider>
    );
  }
}

export default App;
