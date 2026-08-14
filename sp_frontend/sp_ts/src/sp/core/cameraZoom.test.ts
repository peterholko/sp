import assert from 'node:assert/strict';
import {
  DESKTOP_CAMERA_ZOOM,
  DESKTOP_CAMERA_ZOOM_IN,
  DESKTOP_CAMERA_ZOOM_OUT,
  combatZoomTransition,
  desktopZoomControl,
} from './config';

assert.equal(DESKTOP_CAMERA_ZOOM, 2, 'desktop maps start at the close 2× zoom');

assert.deepEqual(
  desktopZoomControl(DESKTOP_CAMERA_ZOOM_IN),
  {
    nextZoom: DESKTOP_CAMERA_ZOOM_OUT,
    label: '−',
    title: 'Zoom out to 1×',
  },
  'the default close view offers a minus control that zooms out',
);

assert.deepEqual(
  desktopZoomControl(DESKTOP_CAMERA_ZOOM_OUT),
  {
    nextZoom: DESKTOP_CAMERA_ZOOM_IN,
    label: '+',
    title: 'Zoom in to 2×',
  },
  'the wide view offers a plus control that zooms back in',
);

assert.deepEqual(
  combatZoomTransition(DESKTOP_CAMERA_ZOOM_OUT),
  {
    zoom: DESKTOP_CAMERA_ZOOM_IN,
    restoreZoom: DESKTOP_CAMERA_ZOOM_OUT,
  },
  'combat temporarily brings a zoomed-out player into the close view',
);
assert.equal(
  combatZoomTransition(DESKTOP_CAMERA_ZOOM_IN),
  null,
  'combat leaves a player who is already zoomed in unchanged',
);

console.log('Camera zoom checks passed');
