import assert from "node:assert/strict";
import { villagerPanelPresentation } from "./villagerPanelPresentation";

const deadVillager = villagerPanelPresentation(
  {
    id: 24,
    state: "dead",
    activity: "Getting some food",
    order: "Work Queue",
    thirst: "Hydrated",
    hunger: "Ravenous",
    tiredness: "Restored",
    hp: 500,
    base_hp: 500,
    base_speed: 0,
  },
  { 24: "Logging" },
  {
    id: 24,
    thirst: "Thirsty",
    hunger: "Hungry",
    tiredness: "Tired",
  },
);

for (const [field, value] of Object.entries(deadVillager)) {
  assert.equal(value, "Dead", `${field} must be overridden after villager death`);
}

assert.deepEqual(
  villagerPanelPresentation(
    {
      id: 25,
      state: "gathering",
      activity: "Operating",
      order: "Work Queue",
      thirst: "Hydrated",
      hunger: "Nourished",
      tiredness: "Restored",
      hp: 90,
      base_hp: 110,
      stamina: 20,
      base_stamina: 30,
      base_speed: 5,
    },
    { 25: "Logging" },
    {
      id: 25,
      thirst: "Slightly Thirsty",
      hunger: "Peckish",
      tiredness: "Weary",
    },
  ),
  {
    activity: "Logging",
    order: "Work Queue",
    thirst: "Slightly Thirsty",
    hunger: "Peckish",
    tiredness: "Weary",
    hp: "90 / 110",
    stamina: "20 / 30",
    speed: 5,
    state: "gathering",
  },
  "living villagers must continue using their live activity and needs updates",
);

console.log("Villager panel presentation checks passed");
