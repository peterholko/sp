import { Global } from './global';
import { Util } from './util';

describe('image preview policy', () => {
  beforeEach(() => {
    Global.imageDefList = {};
  });

  test('unknown definitions do not resolve to raw image paths', () => {
    expect(Util.getImageType('giantrat')).toBeUndefined();
    expect(Util.isSprite('giantrat')).toBeUndefined();
    expect(Util.getImagePreviewName('giantrat')).toBeNull();
  });

  test('known sprites use their single-frame preview', () => {
    Global.imageDefList['giantrat'] = {
      animations: { idle: [0] },
      frames: { width: 72, height: 72 },
    };

    expect(Util.isSprite('giantrat')).toBe(true);
    expect(Util.getImagePreviewName('giantrat')).toBe('giantrat_single.png');
  });

  test('known static and container definitions use their static image', () => {
    Global.imageDefList['shipwreck'] = { frames: { width: 72, height: 72 } };
    Global.imageDefList['stockade'] = { images: ['stockade-base.png'] };

    expect(Util.isSprite('shipwreck')).toBe(false);
    expect(Util.getImagePreviewName('shipwreck')).toBe('shipwreck.png');
    expect(Util.getImagePreviewName('stockade')).toBe('stockade.png');
  });
});
