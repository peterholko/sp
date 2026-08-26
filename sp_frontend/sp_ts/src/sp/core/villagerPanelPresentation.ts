export interface VillagerPanelData {
  id: number;
  state?: string;
  activity?: string;
  order?: string;
  thirst?: string;
  hunger?: string;
  tiredness?: string;
  hp?: number;
  base_hp?: number;
  stamina?: number;
  base_stamina?: number;
  base_speed?: number;
}

export interface VillagerNeedsData {
  id?: number;
  thirst?: string;
  hunger?: string;
  tiredness?: string;
}

export interface VillagerPanelPresentation {
  activity?: string;
  order?: string;
  thirst?: string;
  hunger?: string;
  tiredness?: string;
  hp: string;
  stamina: string;
  speed: string | number | undefined;
  state?: string;
}

const DEAD_STATUS = "Dead";

function currentAndMaximum(current?: number, maximum?: number): string {
  return `${current ?? ""} / ${maximum ?? ""}`;
}

/**
 * Build one coherent villager status snapshot from the selected-object packet
 * and its independently streamed activity/needs updates. Death overrides those
 * live caches because their last values describe an AI actor that no longer
 * exists.
 */
export function villagerPanelPresentation(
  villager: VillagerPanelData,
  activityById?: Record<number, string>,
  needsUpdate?: VillagerNeedsData,
): VillagerPanelPresentation {
  if (villager.state?.trim().toLowerCase() === "dead") {
    return {
      activity: DEAD_STATUS,
      order: DEAD_STATUS,
      thirst: DEAD_STATUS,
      hunger: DEAD_STATUS,
      tiredness: DEAD_STATUS,
      hp: DEAD_STATUS,
      stamina: DEAD_STATUS,
      speed: DEAD_STATUS,
      state: DEAD_STATUS,
    };
  }

  const needs = needsUpdate?.id === villager.id ? needsUpdate : villager;

  return {
    activity: activityById?.[villager.id] ?? villager.activity,
    order: villager.order,
    thirst: needs.thirst,
    hunger: needs.hunger,
    tiredness: needs.tiredness,
    hp: currentAndMaximum(villager.hp, villager.base_hp),
    stamina: currentAndMaximum(villager.stamina, villager.base_stamina),
    speed: villager.base_speed,
    state: villager.state,
  };
}
