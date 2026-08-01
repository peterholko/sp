import { canLootAllEnemyCorpse, lootAllItemIds } from "./lootAllPolicy";

describe("corpse loot-all policy", () => {
  const items = [
    { id: 11, quantity: 1 },
    { id: 12, quantity: 3 },
  ];

  test("offers loot all only for a non-player dead inventory with items", () => {
    expect(canLootAllEnemyCorpse({ state: "dead", player: 1000 }, 7, items)).toBe(true);
    expect(canLootAllEnemyCorpse({ state: "none", player: 1000 }, 7, items)).toBe(false);
    expect(canLootAllEnemyCorpse({ state: "dead", player: 7 }, 7, items)).toBe(false);
    expect(canLootAllEnemyCorpse({ state: "dead", player: 1000 }, 7, [])).toBe(false);
  });

  test("returns only valid, non-empty item ids", () => {
    expect(lootAllItemIds([
      { id: 11, quantity: 1 },
      { id: "12", quantity: 2 },
      { id: 13, quantity: 0 },
      { id: "bad", quantity: 1 },
    ])).toEqual([11, 12]);
  });
});
