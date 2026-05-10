export const GAME_WIDTH = 666;
export const GAME_HEIGHT = 375;
export const ART = '/static/priv/art'
export const STAT_BAR_WIDTH = 124;
export const STAT_BAR_HEIGHT = 14;
export const LARGE_SCREEN_WIDTH = 1024;

export const DESKTOP_CAMERA_ZOOM = 1.0;
export const DESKTOP_CANVAS_WIDTH = 1200;
export const DESKTOP_CANVAS_HEIGHT = 1000;
export const WIDE_SCREEN_MIN = 1800;

const VIEWPORT_MARGIN = 24;
const DESKTOP_MIN_W = 1100;
const DESKTOP_MIN_H = 760;

export function getDesktopCanvasSize(): { width: number; height: number } {
  if (typeof window === 'undefined') {
    return { width: DESKTOP_CANVAS_WIDTH, height: DESKTOP_CANVAS_HEIGHT };
  }
  const w = Math.min(DESKTOP_CANVAS_WIDTH,  Math.max(DESKTOP_MIN_W, window.innerWidth  - VIEWPORT_MARGIN));
  const h = Math.min(DESKTOP_CANVAS_HEIGHT, Math.max(DESKTOP_MIN_H, window.innerHeight - VIEWPORT_MARGIN));
  return { width: w, height: h };
}

export function isWideScreen(): boolean {
  if (typeof window === 'undefined') return false;
  return isDesktop() && window.innerWidth >= WIDE_SCREEN_MIN;
}

export function isDesktop(): boolean {
  if (typeof window === 'undefined') return false;
  if ((window as any).__SP_DESKTOP__ === true) return true;
  if (window.screen.width <= LARGE_SCREEN_WIDTH) return false;
  if (window.screen.width === 1366 && window.screen.height === 1024) return false;
  return true;
}

export function desktopCameraZoom(): number {
  return isDesktop() ? DESKTOP_CAMERA_ZOOM : 1;
}
export const BUTTON_WIDTH = 50;
export const TRIGGER_INVENTORY = 'inventory';
export const TRIGGER_EQUIP = 'equip';
export const TRIGGER_PLAYER_SELLING_ITEM = 'player_selling_item';
export const TRIGGER_PLAYER_BUYING_ITEM = 'player_buying_item';
export const TRIGGER_REFINING_ITEM = 'refining_item';
export const TRIGGER_STRUCTURE_REFINING_ITEM = 'structure_refining_item';
export const TRIGGER_STRUCTURE_CRAFTING_ITEM = 'structure_crafting_item';

export const SPRITE = 'string';
export const IMAGE = 'image';
export const CONTAINER = 'container';

export const UNIT = 'unit';
export const STRUCTURE = 'structure';
export const CORPSE = 'corpse';

export const HERO = 'hero';
export const NPC = 'npc';
export const VILLAGER = 'villager';
export const CRAFT = 'craft';
export const RESOURCE = 'resource';
export const DEAD = 'dead';
export const FOUNDED = 'founded';
export const BUILDING = 'building';
export const STALLED = 'stalled';
export const PLANNING_UPGRADE = 'planning_upgrade';
export const UPGRADING = 'upgrading';
export const CRAFTING = 'crafting';
export const GATHERING = 'gathering';
export const HARVESTING = 'harvesting';
export const NONE = 'none';
export const OBJ = 'obj';
export const TILE = 'tile';
export const WALL = 'wall';

export const WEAPON = 'Weapon';

export const QUICK = 'quick';
export const PRECISE = 'precise';
export const FIERCE = 'fierce';
export const BLOCK = 'block';

export const EXP_RECIPE_NONE = -1;

export const FALSE = 'false';
export const TRUE = 'true';

// Thirst levels
export const HYDRATED = 'Hydrated';
export const REFRESHED = 'Refreshed';
export const SLIGHTLY_THIRSTY = 'Slightly Thirsty';
export const THIRSTY = 'Thirsty';
export const PARCHED = 'Parched';
export const DEHYDRATED = 'Dehydrated';

// Hunger levels
export const SATIATED = 'Satiated';
export const NOURISHED = 'Nourished';
export const HUNGRY = 'Hungry';
export const PECKISH = 'Peckish';
export const FAMISHED = 'Famished';
export const RAVENOUS = 'Ravenous';

// Fatigue levels
export const ENERGIZED = 'Energized';
export const RESTORED = 'Restored';
export const WEARY = 'Weary';
export const TIRED = 'Tired';
export const EXHAUSTED = 'Exhausted';
export const DEPLETED = 'Depleted';
