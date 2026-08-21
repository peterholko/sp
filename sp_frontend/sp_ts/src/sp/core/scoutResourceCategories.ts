export interface ScoutedResourceCategory {
  category: string;
  x: number;
  y: number;
}

export const SCOUT_CATEGORY_ORDER = [
  'Ore',
  'Stone',
  'Timber',
  'Forage',
  'Water',
  'Fish',
  'Game',
] as const;

export const SCOUT_CATEGORY_ICON_KEYS: Record<string, string> = {
  Ore: 'scout-resource-ore',
  Stone: 'scout-resource-stone',
  Timber: 'scout-resource-timber',
  Forage: 'scout-resource-forage',
  Water: 'scout-resource-water',
  Fish: 'scout-resource-fish',
  Game: 'scout-resource-game',
};

// The source assets are 48px. A 24px logical size renders them at their native
// resolution under the default 2x desktop camera zoom.
export const SCOUT_CATEGORY_ICON_ASSET_SIZE = 48;
export const SCOUT_CATEGORY_ICON_SIZE = 24;
export const SCOUT_CATEGORY_ICON_OVERLAP = 2.5;
const SCOUT_CATEGORY_ICONS_PER_ROW = 3;

export interface ScoutedResourceCategoryGroup {
  x: number;
  y: number;
  categories: string[];
}

export type ResourceLayerToggleAction = 'hide' | 'show' | 'request';

export function resourceLayerToggleAction(
  visible: boolean,
  hasCachedIcons: boolean,
): ResourceLayerToggleAction {
  if (visible) {
    return 'hide';
  }
  return hasCachedIcons ? 'show' : 'request';
}

/**
 * Collapse duplicate server entries into one ordered icon set per tile. The
 * category allow-list keeps an unknown future category from producing a
 * missing-texture square on older clients.
 */
export function groupScoutedResourceCategories(
  entries: ScoutedResourceCategory[],
): ScoutedResourceCategoryGroup[] {
  const categoriesByTile = new Map<string, ScoutedResourceCategoryGroup>();

  for (const entry of entries) {
    if (!SCOUT_CATEGORY_ICON_KEYS[entry.category]) {
      continue;
    }

    const key = `${entry.x},${entry.y}`;
    const group = categoriesByTile.get(key) ?? {
      x: entry.x,
      y: entry.y,
      categories: [],
    };
    if (!group.categories.includes(entry.category)) {
      group.categories.push(entry.category);
    }
    categoriesByTile.set(key, group);
  }

  const order = new Map<string, number>(
    SCOUT_CATEGORY_ORDER.map((category, index) => [category, index]),
  );
  const groups = Array.from(categoriesByTile.values());
  for (const group of groups) {
    group.categories.sort(
      (left, right) => (order.get(left) ?? 999) - (order.get(right) ?? 999),
    );
  }

  return groups.sort((left, right) => left.y - right.y || left.x - right.x);
}

/** Return a compact, slightly overlapping stack inside a logical 72px hex. */
export function scoutCategoryIconOffset(index: number, count: number) {
  const row = Math.floor(index / SCOUT_CATEGORY_ICONS_PER_ROW);
  const firstIndexInRow = row * SCOUT_CATEGORY_ICONS_PER_ROW;
  const iconsInRow = Math.min(
    SCOUT_CATEGORY_ICONS_PER_ROW,
    count - firstIndexInRow,
  );
  const column = index - firstIndexInRow;
  const iconStep = SCOUT_CATEGORY_ICON_SIZE - SCOUT_CATEGORY_ICON_OVERLAP;
  const rowWidth = SCOUT_CATEGORY_ICON_SIZE + (iconsInRow - 1) * iconStep;

  return {
    x:
      -rowWidth / 2
      + SCOUT_CATEGORY_ICON_SIZE / 2
      + column * iconStep,
    y: row * iconStep,
  };
}
