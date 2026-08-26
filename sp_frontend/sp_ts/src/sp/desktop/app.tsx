import React, { Component } from "react";
import { Provider } from "react-redux";
import WebFont from 'webfontloader';

import store from "../core/store";
import AppErrorBoundary from '../core/appErrorBoundary';
import { markApplicationBooted } from '../core/appBoot';
import LoginControl from "./login";
import { Global } from '../core/global';

Global.gameEmitter = new Phaser.Events.EventEmitter();
Global.uiEmitter = new Phaser.Events.EventEmitter();

class App extends Component {
  componentDidMount() {
    markApplicationBooted();

    // Load the font using WebFont loader
    try {
      WebFont.load({
        google: {
          families: ['Almendra SC', 'Cinzel', 'IM Fell English', 'Uncial Antiqua'], // Specify the font to load
        },
      });
    } catch (error) {
      console.warn('Unable to load optional web fonts', error);
    }
  }

  render() {
    return (
      <Provider store={store}>
        <AppErrorBoundary>
          <div>
            <LoginControl />
          </div>
        </AppErrorBoundary>
      </Provider>
    );
  }
}

export default App;
