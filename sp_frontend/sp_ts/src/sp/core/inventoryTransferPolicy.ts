import { DEAD, HERO, VILLAGER } from "./config";

/**
 * Heroes and living villagers must unequip a sole item before transferring it.
 * A legacy equipped stack remains selectable because the server transfers only
 * its spare quantity and leaves one unit equipped. Once a villager is dead,
 * their inventory is corpse loot and its former equipment state must not make
 * the item unselectable. Hero death and resurrection have separate ownership
 * semantics, so hero gear stays guarded.
 */
export function inventoryOwnerCanEquip(objectState: any): boolean {
  return Boolean(
    objectState
      && (
        objectState.subclass == HERO
        || (objectState.subclass == VILLAGER && objectState.state != DEAD)
      )
  );
}

export function inventoryItemTransferLocked(objectState: any, item: any): boolean {
  return inventoryOwnerCanEquip(objectState)
    && Boolean(item?.equipped)
    && (item?.quantity ?? 1) <= 1;
}
