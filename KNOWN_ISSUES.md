# Known Issues

Findings from the 2026-06-10 instrumented-play analysis (5 headless bot runs +
105 recorded runs in `scores`, cross-checked against code). Everything below
is still open, ordered by priority.

Already fixed from that analysis:

- Hero needs warnings, death economy (flat first-death cost + needs reset on
  resurrect), nightly-wave pursuit, and needs/crisis pacing — commit
  `Survival fixes: needs warnings, death economy, wave pursuit, pacing`.
- The three run-poisoning bugs: stale-session login dead-end (sp_axum now
  rotates sessions older than 20h in `/auth` and `/fingerprint-auth`), dirty
  start-location recycling (True Death now removes the player's objects, the
  run's spawns tracked in `RunSpawnedObjs` — setup POIs, nightly waves,
  legendary hideout/boss/followers — plus NPC hostiles within
  `RUN_CLEANUP_RADIUS` of camp; clears the per-run intro/encounter/objectives
  state that kept spawning enemies after death; purges pending map events for
  removed objects, which otherwise panicked the server; and refills the bound
  monolith's Soulshards to the starting 10), and the Burrow's 50 starting gold
  instantly triggering the tier-3 raid (now 20, under the 30-gold threshold).

  Minor residual: the POI-guard `SpawnNPC` game events scheduled at setup
  (+6000 ticks) are not player-attributed; if a run ends before they fire,
  a few enemies still appear at the POIs (not the camp) of the released
  location afterwards.

---

## Open balance / scoring items

### 4. Score barely discriminates play quality (S/M)

A do-nothing run scored 9,068 vs. 14,375 for the best bot run — most of the
spread is free points. Since victory is defined as surviving the longest, the
score *is* the win condition and needs to be trustworthy:

- `waves_survived` increments when the wave **spawns**
  (`nightly_threat_system`), so a wave that kills you still counts as
  "survived". Count it at dawn when the wave is dead/expired instead.
- `defense` includes `crisis_tier * 1000`, which (see issue 3) is free.
  Crisis tier is a threat counter, not an achievement — drop it from score.
- `highest_pressure_level` pegs at "Crisis" within seconds of spawn
  (`build_threat_state_packet`), so the score multiplier is a constant
  ×1.15. Recompute pressure from live threat proximity, or cut the
  multiplier.

### 5. Legendary follower waves are a drip, not a stage (M)

From day 7 (`legendary_threat_system`), a follower wave (Wyvern Rider +
Gryphon + Great Troll, Death Knight every 3rd) spawns every 600 ticks (60
real seconds), accelerating to every 300 ticks after 3 days. That is
continuous pressure with no preparation window — against the
prepare-and-survive design. Batch followers into the nightly dusk pulse
(bigger nights, calm days) instead. Unverified in play: no run has yet
survived to day 7.

### 6. Day-8+ content has never been play-tested (verification task)

The survival director (day ≥ 8), the day-8/10/12/14/16/18 horde composition
tables, the legendary arc combat, hideout clearing, and Monolith sealing have
never been reached by any run (max ever: day 6, pre-rebalance). Extend
`sp_agent/needs_run.py` with waterskin refilling and food foraging and let it
run to day 8–10; tune whatever breaks first.

---

## Open feedback / legibility items

### 7. Threat panel UI is built, fed, and switched off (S)

The server sends `threat_state` (pressure level, known risks with thresholds,
next-night warning) every 5 seconds, and the client has handlers — but the
panel render is commented out in `ObjectivesPanel`
(`sp_frontend/sp_ts/.../objectivesPanel.tsx`). `discovery_event` UI likewise.
Re-enable once pressure (issue 4) is meaningful.

### 8. Hero HP is never pushed by the server (S)

No `stats` packets are sent on damage/regen; the `dmg` packet has no
remaining-HP field. The client reconstructs HP by locally subtracting damage
from a once-fetched value (`network.ts` `processDmg`), which drifts with
regen, potions, and resurrection. Either include remaining HP in `dmg` or
push a stats update on HP change.

### 9. Needs label scale is inverted/mismatched (S)

In `ai/common/common.rs`: Hungry covers 30–60 while Peckish covers 60–75 —
backwards (peckish = mildly hungry). The tiredness label "Exhausted" covers
75–90 but the lethal `Exhausted` component starts at >90 (labelled
"Depleted"), so the word and the mechanic disagree. Also `DEPELTED` typo.
Warning copy (added 2026-06-10) should stay aligned with whatever labels are
chosen.

### 10. Silent wave-spawn failure (S)

`nightly_threat_system` sends the dusk warning Notice *before* attempting to
spawn; if `crisis_spawn_pos` fails (no valid ring position), the player gets
the warning but no wave, with no log line. Log the failure and skip or delay
the warning until the spawn succeeds.

---

## Test-harness gaps (sp_agent)

- `agent.py` ignores `hero_death_state` / `info_true_death` — it kept
  commanding a despawned hero for 15+ minutes. Handle both and stop (or
  re-register) on true death.
- `GameState.current_tick` is never updated from any packet, so the agent
  always sees "Night (tick 0)" and cannot react to dusk. Use the `world`
  packet's `time_of_day` in the summary.
- The system-prompt structure names ("Shelter", "Storage Box") are not valid
  templates — server returns `Invalid structure name`. Use real names
  (Small Tent, Cache/Warehouse, Crafting Tent, ...).
- Hero HP in the agent summary is stale for the same reason as issue 8 —
  poll `get_stats` after combat events, or fix issue 8.
