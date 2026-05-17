import * as React from "react";

export interface HeroDeathStateData {
  phase: string;
  hero_id: number;
  hero_name: string;
  resurrect_cost: number;
  soulshards_available: number;
  seconds_remaining: number;
  message: string;
}

interface Props {
  data?: HeroDeathStateData | null;
}

function titleForPhase(phase: string): string {
  if (phase == 'resurrected') {
    return 'The Monolith binds you again';
  }

  if (phase == 'true_death_pending') {
    return 'The Monolith cannot bind you';
  }

  return 'The Monolith weighs your soul';
}

function statusForPhase(phase: string, secondsRemaining: number): string {
  if (phase == 'resurrected') {
    return 'You awaken at the Monolith.';
  }

  if (phase == 'true_death_pending') {
    return secondsRemaining > 0
      ? `True Death in ${secondsRemaining}s`
      : 'True Death is near.';
  }

  return secondsRemaining > 0
    ? `Resurrection check in ${secondsRemaining}s`
    : 'The Monolith is deciding.';
}

export default function HeroDeathOverlay({ data }: Props) {
  const [secondsRemaining, setSecondsRemaining] = React.useState(0);

  React.useEffect(() => {
    const initialSeconds = Math.max(0, data?.seconds_remaining || 0);
    setSecondsRemaining(initialSeconds);

    if (!data || initialSeconds <= 0 || data.phase == 'resurrected') {
      return;
    }

    const timer = window.setInterval(() => {
      setSecondsRemaining((seconds) => Math.max(0, seconds - 1));
    }, 1000);

    return () => window.clearInterval(timer);
  }, [data?.hero_id, data?.phase, data?.seconds_remaining]);

  if (!data) {
    return null;
  }

  const shortage = Math.max(0, data.resurrect_cost - data.soulshards_available);

  return (
    <div style={styles.overlay} role="alert" aria-live="assertive">
      <div style={styles.panel}>
        <div style={styles.eyebrow}>{data.hero_name}</div>
        <h1 style={styles.title}>{titleForPhase(data.phase)}</h1>
        <div style={styles.status}>{statusForPhase(data.phase, secondsRemaining)}</div>
        <div style={styles.divider} />
        <div style={styles.stats}>
          <div style={styles.stat}>
            <span style={styles.statLabel}>Soulshards</span>
            <span style={styles.statValue}>{data.soulshards_available}</span>
          </div>
          <div style={styles.stat}>
            <span style={styles.statLabel}>Cost</span>
            <span style={styles.statValue}>{data.resurrect_cost}</span>
          </div>
          {data.phase == 'true_death_pending' &&
            <div style={styles.stat}>
              <span style={styles.statLabel}>Short</span>
              <span style={styles.statValue}>{shortage}</span>
            </div>
          }
        </div>
        <p style={styles.message}>{data.message}</p>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  overlay: {
    position: 'fixed',
    inset: 0,
    zIndex: 1000000,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    padding: 'max(18px, env(safe-area-inset-top)) max(18px, env(safe-area-inset-right)) max(18px, env(safe-area-inset-bottom)) max(18px, env(safe-area-inset-left))',
    background: 'rgba(0, 0, 0, 0.78)',
    color: '#f4ead1',
    pointerEvents: 'auto',
  },
  panel: {
    width: 'min(520px, 92vw)',
    maxHeight: 'min(520px, 86vh)',
    overflowY: 'auto',
    border: '2px solid rgba(202, 171, 105, 0.8)',
    borderRadius: 8,
    background: 'linear-gradient(180deg, rgba(26, 25, 24, 0.96), rgba(8, 8, 8, 0.98))',
    boxShadow: '0 20px 70px rgba(0, 0, 0, 0.75), inset 0 0 24px rgba(202, 171, 105, 0.12)',
    padding: '22px 24px 24px',
    textAlign: 'center',
  },
  eyebrow: {
    color: '#c9ad70',
    fontFamily: 'serif',
    fontSize: 18,
    letterSpacing: 0,
    marginBottom: 8,
  },
  title: {
    margin: 0,
    fontFamily: 'serif',
    fontSize: 34,
    fontWeight: 500,
    letterSpacing: 0,
    lineHeight: 1.1,
  },
  status: {
    marginTop: 12,
    fontSize: 18,
    color: '#fff8df',
  },
  divider: {
    height: 1,
    margin: '18px 0',
    background: 'rgba(202, 171, 105, 0.55)',
  },
  stats: {
    display: 'grid',
    gridTemplateColumns: 'repeat(auto-fit, minmax(120px, 1fr))',
    gap: 10,
  },
  stat: {
    border: '1px solid rgba(202, 171, 105, 0.45)',
    borderRadius: 6,
    padding: '10px 12px',
    background: 'rgba(255, 255, 255, 0.04)',
  },
  statLabel: {
    display: 'block',
    color: '#c9ad70',
    fontSize: 13,
    marginBottom: 4,
  },
  statValue: {
    display: 'block',
    color: '#fff8df',
    fontSize: 24,
    lineHeight: 1,
  },
  message: {
    margin: '18px 0 0',
    color: '#efe6d1',
    fontSize: 17,
    lineHeight: 1.35,
  },
};
