export type SessionLaunchDestination = 'landing' | 'hero-creation' | 'game';

export function sessionLaunchDestination(
  needsHero: boolean,
  userInitiated: boolean,
): SessionLaunchDestination {
  if (!needsHero) {
    return 'game';
  }

  return userInitiated ? 'hero-creation' : 'landing';
}
