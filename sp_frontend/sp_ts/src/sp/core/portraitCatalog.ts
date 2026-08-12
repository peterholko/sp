export const HERO_PORTRAITS = [
  'portraits/heroes/hero-01.png',
  'portraits/heroes/hero-02.png',
  'portraits/heroes/hero-03.png',
  'portraits/heroes/hero-04.png',
  'portraits/heroes/hero-05.png',
] as const;

export const VILLAGER_PORTRAITS = [
  'portraits/villagers/villager-01.png',
  'portraits/villagers/villager-02.png',
  'portraits/villagers/villager-03.png',
  'portraits/villagers/villager-04.png',
  'portraits/villagers/villager-05.png',
  'portraits/villagers/villager-06.png',
] as const;

export const DEFAULT_HERO_PORTRAIT = HERO_PORTRAITS[0];

export function portraitUrl(portrait?: string | null): string | null {
  return portrait ? '/static/art/' + portrait : null;
}

export function characterImageUrl(portrait: string | null | undefined, spriteImage: string): string {
  return portraitUrl(portrait)
    ?? '/static/art/' + spriteImage.toLowerCase().replace(/\s/g, '') + '_single.png';
}
