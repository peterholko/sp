import {
  canBeSignatureForRecipe,
  isSignatureComponent,
  itemRarity,
  rarityBorderColor,
  rarityDisplayName,
  rarityTooltip,
  signatureDisplayName,
} from './itemRarity';

describe('item rarity presentation', () => {
  test('legacy and ordinary items default to Common', () => {
    expect(itemRarity({})).toBe('Common');
    expect(rarityDisplayName({ name: 'bones' })).toBe('bones');
    expect(rarityBorderColor('Common')).toBeNull();
    expect(isSignatureComponent({})).toBe(false);
    expect(signatureDisplayName({ name: 'Cooked Meat' })).toBeNull();
  });

  test('only elevated rarities receive a border colour', () => {
    expect(rarityBorderColor('Uncommon')).toBe('#55c96b');
    expect(rarityBorderColor('Magic')).toBe('#6699ff');
    expect(rarityBorderColor('Rare')).toBe('#f2c94c');
  });

  test('special components expose rarity and affixes', () => {
    const item = {
      name: 'Frostmane Raw Hide',
      attrs: { Rarity: 'Rare', Affixes: "Stout, Hunter's" },
    };
    expect(rarityDisplayName(item)).toBe('Rare Frostmane Raw Hide');
    expect(rarityTooltip(item)).toContain("Stout, Hunter's");
    expect(isSignatureComponent(item)).toBe(true);
    expect(signatureDisplayName(item)).toBe('Rare Frostmane Raw Hide');
  });

  test('signature candidates must satisfy a recipe requirement', () => {
    const item = {
      name: 'Cragroot Maple Timber',
      class: 'Timber',
      subclass: 'Maple Timber',
      attrs: { Rarity: 'Magic' },
    };
    expect(canBeSignatureForRecipe(item, { req: [{ type: 'Timber' }] })).toBe(true);
    expect(canBeSignatureForRecipe(item, { req: [{ type: 'Hide' }] })).toBe(false);
  });
});
