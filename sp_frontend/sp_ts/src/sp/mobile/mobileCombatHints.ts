export interface MobileCombatHints {
  intent: string;
  counter: string;
}

const INTENT_LABELS: Record<string, string> = {
  'Fast creature looking for an opening': 'Fast opener',
  'Low creature trying to drag the fight close': 'Pulls you close',
  'Tiny pest chipping at close range': 'Close-range pest',
  'Armored pest bracing through light attacks': 'Armored; resists light hits',
  'Startled wildlife trying to stay clear': 'Avoiding combat',
  'Close-range attacker testing your position': 'Close-range pressure',
  'Heavy predator ready to maul anything too close': 'Heavy melee',
  'Ancient brute bearing down with crushing force': 'Crushing melee',
  'Undead pressure advancing steadily': 'Steady advance',
  'Caster seeking distance and corpses to exploit': 'Ranged caster',
  'Raider advancing on your defenders and blocking walls': 'Targets defenders & walls',
  'Raider targeting your stored value and structures': 'Targets stores & structures',
  'Hostile target preparing to attack': 'Preparing attack',
};

const COUNTER_LABELS: Record<string, string> = {
  'Start with quick for control, precise for setup, fierce for damage, or block to buy time.':
    'Quick / Precise / Fierce / Block',
  'Fast enemies reward control: quick chains toward Hamstring, while block protects low stamina.':
    'Quick → Hamstring • Block low',
  'Weak close-range enemies can be finished quickly; block if stamina is low.':
    'Finish fast • Block low',
  'Mobile shell enemies reward quick control or a block before trading damage.':
    'Quick control or Block',
  'Armored enemies reward setup: use precise attacks before committing fierce damage.':
    'Precise setup → Fierce',
  'Passive wildlife rarely presses the fight; use light attacks if you must hunt it.':
    'Use light attacks',
  'Heavy predators punish sloppy trades; block to stabilize, then use precise setup before fierce damage.':
    'Block → Precise → Fierce',
  'Ancient brutes punish sloppy trades; block to stabilize, then use precise setup before fierce damage.':
    'Block → Precise → Fierce',
  'Steady undead can be set up with precise attacks, then punished with a combo finisher.':
    'Precise setup → Finisher',
  'Pressure the caster before corpses become resources; block if you cannot close safely.':
    'Close fast • Block if needed',
  'Follow the visible combo hints or block when the exchange is turning against you.':
    'Follow combo • Block if losing',
};

function compactFallback(value: unknown, maxLength: number): string {
  const normalized = typeof value === 'string'
    ? value.trim().replace(/\s+/g, ' ').replace(/[.!?]+$/, '')
    : '';
  if (!normalized) return '';

  const firstClause = normalized.split(/[;.!?]/, 1)[0].trim();
  if (firstClause.length <= maxLength) return firstClause;

  const words = firstClause.split(' ');
  let result = '';
  for (const word of words) {
    const next = result ? `${result} ${word}` : word;
    if (next.length > maxLength - 1) break;
    result = next;
  }

  return `${result || firstClause.slice(0, maxLength - 1).trimEnd()}…`;
}

function compactLabel(value: unknown, labels: Record<string, string>, maxLength: number): string {
  const normalized = typeof value === 'string' ? value.trim() : '';
  return labels[normalized] || compactFallback(normalized, maxLength);
}

export function mobileCombatHints(enemyIntent: unknown, counterHint: unknown): MobileCombatHints {
  return {
    intent: compactLabel(enemyIntent, INTENT_LABELS, 30),
    counter: compactLabel(counterHint, COUNTER_LABELS, 38),
  };
}
