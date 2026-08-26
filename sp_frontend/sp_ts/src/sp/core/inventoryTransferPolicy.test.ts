import {
  inventoryItemTransferLocked,
  inventoryOwnerCanEquip,
} from "./inventoryTransferPolicy";

describe("item-transfer equipment policy", () => {
  test("living heroes and villagers retain equipped-item protection", () => {
    expect(inventoryOwnerCanEquip({ subclass: "hero", state: "none" })).toBe(true);
    expect(inventoryOwnerCanEquip({ subclass: "villager", state: "idle" })).toBe(true);
  });

  test("dead villagers expose formerly equipped items as corpse loot", () => {
    expect(inventoryOwnerCanEquip({ subclass: "villager", state: "dead" })).toBe(false);
  });

  test("dead hero equipment remains protected for resurrection", () => {
    expect(inventoryOwnerCanEquip({ subclass: "hero", state: "dead" })).toBe(true);
  });

  test("non-character inventories do not apply equipment protection", () => {
    expect(inventoryOwnerCanEquip({ subclass: "stockade", state: "none" })).toBe(false);
    expect(inventoryOwnerCanEquip(undefined)).toBe(false);
  });

  test("an equipped legacy stack exposes only its spare quantity", () => {
    const villager = { subclass: "villager", state: "none" };

    expect(inventoryItemTransferLocked(villager, { equipped: true, quantity: 1 })).toBe(true);
    expect(inventoryItemTransferLocked(villager, { equipped: true, quantity: 2 })).toBe(false);
    expect(inventoryItemTransferLocked(villager, { equipped: false, quantity: 1 })).toBe(false);
  });
});
