import { ObjectState } from "../../core/objectState";
import {
  VillagerActivityIcon,
  villagerActivityIcon,
} from "../../core/villagerResourceActivity";

export interface SelectedPortraitActivityBadge {
  icon: VillagerActivityIcon,
  label: string,
}

export const SELECTED_PORTRAIT_ACTIVITY_BADGE_SIZE = 28;
export const SELECTED_PORTRAIT_ACTIVITY_BADGE_BORDER = 2;
export const SELECTED_PORTRAIT_ACTIVITY_ICON_SIZE = 24;

/** Returns the activity badge shown on the actively selected villager portrait. */
export function selectedPortraitActivityBadge(
  objectState?: ObjectState,
  activity?: string,
): SelectedPortraitActivityBadge | null {
  if (!objectState) {
    return null;
  }

  const liveState = activity === undefined
    ? objectState
    : { ...objectState, activity };
  const icon = villagerActivityIcon(liveState);
  if (!icon) {
    return null;
  }

  return {
    icon,
    label: 'Activity: ' + (liveState.activity || liveState.state || icon),
  };
}
