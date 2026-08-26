export interface ItemUseDescriptor {
  class?: string;
  subclass?: string;
}

/**
 * Items for which the inventory item panel can issue the server's Use command.
 * Keep this aligned with the authoritative use-item handlers.
 */
export function canUseInventoryItem(item: ItemUseDescriptor | null | undefined): boolean {
  if (!item) {
    return false;
  }

  return item.class === "Potion"
    || item.class === "Deed"
    || item.class === "Food"
    || item.class === "Drink"
    || (item.class === "Medical" && item.subclass === "Bandage")
    || item.subclass === "Bucket"
    || item.subclass === "Fishing Rod"
    || item.subclass === "Waterskin"
    || item.subclass === "Bedroll";
}
