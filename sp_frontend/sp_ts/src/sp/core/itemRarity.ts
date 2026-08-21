export type ItemRarity = 'Common' | 'Uncommon' | 'Magic' | 'Rare';

const RARITY_COLORS: Record<ItemRarity, string> = {
  Common: '#b8b8b8',
  Uncommon: '#55c96b',
  Magic: '#6699ff',
  Rare: '#f2c94c',
};

export function itemRarity(item: { attrs?: Record<string, unknown> } | null | undefined): ItemRarity {
  const value = item?.attrs?.Rarity;
  return value === 'Uncommon' || value === 'Magic' || value === 'Rare'
    ? value
    : 'Common';
}

export function rarityColor(rarity: ItemRarity): string {
  return RARITY_COLORS[rarity];
}

export function rarityBorderColor(rarity: ItemRarity): string | null {
  return rarity === 'Common' ? null : rarityColor(rarity);
}

export function rarityDisplayName(item: { name: string; attrs?: Record<string, unknown> }): string {
  const rarity = itemRarity(item);
  return rarity === 'Common' ? item.name : `${rarity} ${item.name}`;
}

export function rarityTooltip(item: { name: string; attrs?: Record<string, unknown> }): string {
  const rarity = itemRarity(item);
  const affixes = item.attrs?.Affixes;
  return typeof affixes === 'string' && affixes.length > 0
    ? `${rarity} ${item.name} — ${affixes}`
    : `${rarity} ${item.name}`;
}

export function isSignatureComponent(item: { attrs?: Record<string, unknown> }): boolean {
  return itemRarity(item) !== 'Common';
}

export function signatureDisplayName(
  item: { name: string; attrs?: Record<string, unknown> } | null | undefined,
): string | null {
  if (!item || !isSignatureComponent(item)) {
    return null;
  }
  return `${itemRarity(item)} ${item.name}`;
}

export function canBeSignatureForRecipe(
  item: { name: string; class: string; subclass: string; attrs?: Record<string, unknown> },
  recipe: { req?: Array<{ type: string }> },
): boolean {
  return isSignatureComponent(item) && (recipe.req || []).some((requirement) =>
    requirement.type === item.name
      || requirement.type === item.class
      || requirement.type === item.subclass
  );
}
