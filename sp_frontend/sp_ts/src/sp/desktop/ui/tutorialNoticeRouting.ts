export const DESKTOP_TUTORIAL_NOTICE_MESSAGES = [
  'Survival thread started: search the Shipwreck, recover your supplies, and build a Burrow beside the lit Campfire.',
  "The wreck's supplies are within reach. Transfer the five Logs and any equipment you need, then build a Burrow before danger closes in.",
] as const;

export function isDesktopTutorialNotice(message: unknown): boolean {
  return typeof message === 'string'
    && DESKTOP_TUTORIAL_NOTICE_MESSAGES.some((tutorialMessage) => tutorialMessage === message);
}
