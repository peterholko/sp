export interface VillagerActivitySource {
  subclass?: string;
  state?: string;
  activity?: string;
}

export type VillagerActivityIcon =
  | 'gathering'
  | 'logging'
  | 'mining'
  | 'hunting'
  | 'fishing'
  | 'building'
  | 'eating'
  | 'drinking'
  | 'sleeping'
  | 'following'
  | 'fetching-tool'
  | 'hauling';

const VILLAGER_ACTIVITY_ICONS: Record<string, VillagerActivityIcon> = {
  gathering: 'gathering',
  harvesting: 'gathering',
  planting: 'gathering',
  tending: 'gathering',
  logging: 'logging',
  lumberjacking: 'logging',
  mining: 'mining',
  stonecutting: 'mining',
  hunting: 'hunting',
  fishing: 'fishing',
  building: 'building',
  upgrading: 'building',
  repairing: 'building',
  eating: 'eating',
  drinking: 'drinking',
  sleeping: 'sleeping',
};

const GATHERING_ACTIVITY_ICONS: Record<string, VillagerActivityIcon> = {
  gathering: 'gathering',
  harvesting: 'gathering',
  planting: 'gathering',
  tending: 'gathering',
  mining: 'mining',
  stonecutting: 'mining',
  hunting: 'hunting',
  logging: 'logging',
  lumberjacking: 'logging',
};

const WORKFLOW_ACTIVITY_ICONS: Record<string, VillagerActivityIcon> = {
  following: 'following',
  hauling: 'hauling',
  unloading: 'hauling',
  'getting tool': 'fetching-tool',
  'fetching tool': 'fetching-tool',
};

function workflowActivityIcon(activity?: string): VillagerActivityIcon | null {
  const normalizedActivity = activity?.trim().toLowerCase();
  if (!normalizedActivity) {
    return null;
  }

  const exactIcon = WORKFLOW_ACTIVITY_ICONS[normalizedActivity];
  if (exactIcon) {
    return exactIcon;
  }

  // Tool-fetch messages include the required tool type, for example
  // "Fetching Mining tool" or "Fetching Logging tool".
  if (normalizedActivity.startsWith('fetching ') && normalizedActivity.endsWith(' tool')) {
    return 'fetching-tool';
  }

  return null;
}

/** Returns the world-space status icon for a villager's current action. */
export function villagerActivityIcon(
  objectState?: VillagerActivitySource | null,
): VillagerActivityIcon | null {
  if (objectState?.subclass !== 'villager') {
    return null;
  }

  const workflowIcon = workflowActivityIcon(objectState.activity);
  if (workflowIcon) {
    return workflowIcon;
  }

  if (objectState.state === 'gathering' && objectState.activity) {
    const activeResourceIcon = GATHERING_ACTIVITY_ICONS[objectState.activity.toLowerCase()];
    if (activeResourceIcon) {
      return activeResourceIcon;
    }
  }

  return VILLAGER_ACTIVITY_ICONS[objectState.state || ''] || null;
}
