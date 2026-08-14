import { DEAD } from "./config";

export function lootAllItemIds(items: any[]): number[] {
  return (items || [])
    .filter(item => item && Number.isFinite(Number(item.id)) && Number(item.quantity) > 0)
    .map(item => Number(item.id));
}

export function canLootAllEnemyCorpse(
  objectState: any,
  currentPlayerId: string | number,
  items: any[],
): boolean {
  return Boolean(
    objectState
      && objectState.state == DEAD
      && Number(objectState.player) != Number(currentPlayerId)
      && lootAllItemIds(items).length > 0
  );
}

export function canLootAllDroppedBag(objectState: any, items: any[]): boolean {
  return Boolean(
    objectState
      && objectState.template === "Dropped Bag"
      && lootAllItemIds(items).length > 0
  );
}

export function canLootAllTarget(
  objectState: any,
  currentPlayerId: string | number,
  items: any[],
): boolean {
  return canLootAllEnemyCorpse(objectState, currentPlayerId, items)
    || canLootAllDroppedBag(objectState, items);
}
