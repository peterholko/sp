# Known Issues

Findings from the 2026-06-10 instrumented-play analysis (5 headless bot runs +
105 recorded runs in `scores`, cross-checked against code). The four top fixes
from that analysis (hero needs warnings, death economy, wave pursuit, needs/
crisis pacing) landed in `Survival fixes: needs warnings, death economy, wave
pursuit, pacing`. Everything below is still open, ordered by priority.

---

## Run-poisoning bugs

### 1. Stale-session login dead-end (S)

**Symptom:** a returning account cannot connect. The game server accepts the
WebSocket, then drops it; the client sees an immediate disconnect with no
explanation.

**Cause:** `sp_axum` `/auth` (`auth_handler`, `sp_axum/src/main.rs`) reuses the
stored `sessions` row forever — it only generates a new session when no row
exists. The game server (`network.rs`) rejects sessions older than 1 day
(`Session expired: created ... is older than 1 day`). Any account that last
logged in more than a day ago is hard-blocked until its `sessions` row is
deleted by hand.

**Observed:** `agent1` (last session 2026-04-02) could not connect on
2026-06-10 until `DELETE FROM sessions WHERE player_id = 2`.

**Fix:** in `auth_handler`, when the stored session's `created_at` is older
than (or near) the game server's max age, generate a fresh session and update
the row instead of returning the stale one. Alternatively refresh
`created_at` on every successful auth.

### 2. Dirty start-location recycling (M)

**Symptom:** a new hero can spawn into the previous run's leftovers — looted
or duplicate Burrow/Shipwreck, orphaned structures, and still-aggro NPCs.

**Cause:** `true_death_system` (`sp_server/src/game.rs`) releases the start
location back to the pool and despawns the hero, but does not clean up the
dead player's world objects or the hostile NPCs that accumulated around the
camp. With only 5 start locations, every 6th run is guaranteed a recycled,
dirty slot — and it gets worse now that nightly waves actually reach the camp
(leftover wave creatures will camp the spawn).

**Observed:** a fresh hero on recycled `startpos4` was attacked at spawn by
the Cave Bat that killed the previous hero; the tier-3 gold-raid crisis
re-fired for the *dead* player's orphaned Burrow one second after the slot was
released.

**Fix:** on true death, despawn the player's structures/containers (or recycle
them into fresh starter state) and clear hostile NPCs within a radius of the
start position before pushing the location back into `StartLocations`.

### 3. Burrow starting gold instantly triggers the tier-3 crisis (S)

**Symptom:** the goblin wolf-rider raid (crisis tier 3) fires ~80 seconds into
every run, before tiers 1–2 can occur, handing every player +3000 defense
score and +3000 crisis-bonus XP for free and scrambling the escalation ladder.

**Cause:** the starter Burrow spawns with 50 Gold Coins
(`player_setup.rs`, Burrow starting items) — already over the 30-gold
threshold in `goblin_raid_system` (`game.rs`). The "gold attracts raiders"
risk is tripped by the developer-provided kit, not a player choice.

**Observed:** fired at ~80s for every player in every instrumented run,
including after the 2026-06-10 rebalance. Explains why all 105 recorded runs
have `crisis_tier >= 3` regardless of play.

**Fix:** reduce starter gold to 15–20, or exempt starter storage from the
trigger, or raise the threshold above the starting kit's value.

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
