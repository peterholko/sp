export const ACCOUNT_SETUP_PROMPT_DAY = 3;

export function shouldShowAccountSetupPrompt(
  playerDay: unknown,
  accountSetupCompleted: boolean,
  heroDead: boolean,
  alreadyPrompted: boolean,
): boolean {
  return typeof playerDay === 'number'
    && Number.isFinite(playerDay)
    && playerDay >= ACCOUNT_SETUP_PROMPT_DAY
    && !accountSetupCompleted
    && !heroDead
    && !alreadyPrompted;
}
