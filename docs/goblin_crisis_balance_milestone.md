# Milestone 3 — Goblin Crisis Balance and Preparation Loop

## Status

Checkpoint 1, **Balance Audit and Telemetry Baseline**, and Checkpoint 2,
**Pressure and Phase Pacing**, are implementation- and validation-complete.
Checkpoint 1 added observation, controlled headless scenarios, and reporting;
Checkpoint 2 used that evidence to change only the two late phase thresholds
and clarify warning semantics.

> Checkpoint 1 measures the current goblin crisis and intentionally does not make material balance changes.

The bounded runtime matrices and validation records below contain the completed
Checkpoint 1 and 2 evidence. The milestone itself is not complete: Checkpoints
3 and 4 remain deferred.

## Milestone goal and checkpoint plan

The milestone will turn measurements of the current crisis into a deliberate
preparation loop without weakening the persistent-world, personal ownership, or
Safe Logout contracts.

1. **Checkpoint 1 — balance audit and telemetry baseline:** preserve all tuning,
   measure pressure, timing, warnings, preparation, combat outcomes, safety
   invariants, and bounded headless scenarios.
2. **Checkpoint 2 — pressure and phase pacing:** use the baseline to adjust only
   demonstrated pacing problems.
3. **Checkpoint 3 — preparation gameplay and defensive value:** make existing
   preparation choices useful where evidence shows they are not, without
   redesigning the economy.
4. **Checkpoint 4 — assault balance and final validation:** tune and validate the
   first assault, including class, solo, multiplayer, disconnect, and protection
   invariants.

Checkpoint 1 is evidence collection. It does not select or implement new
pressure weights, thresholds, phase minima, warning windows, assault units, NPC
stats or abilities, defensive objects, healing items, rewards, or difficulty
scaling.

## Checkpoint 1 architecture audit

The implementation was inspected in the current checked-out code, including
`sp_server/src/game.rs`, `game_tests.rs`, `headless.rs`, `headless_bot.rs`,
`bin/headless_runner.rs`, `player.rs`, `player_setup.rs`, `combat.rs`,
`ai/npc/npc.rs`, `ai/villager/villager.rs`, `encounter.rs`, `templates.rs`, and
`constants.rs`, plus the object and item templates in `sp_server/templates/`.

### Authority, progression, and scheduling

* `SettlementCrisisState` remains the gameplay authority, with one
  `SettlementCrisis` per player. The ordered phases are `Dormant`, `Signs`,
  `Pressure`, `Preparing`, `AssaultReady`, `AssaultActive`, and `Resolved`.
* `personal_crisis_system` aggregates current settlement facts, derives
  pressure, credits online-active time, and advances at most one pre-assault
  phase per evaluation. It has no spawning, damage, reward, or database
  authority.
* `personal_crisis_assault_system` owns the ready grace, launch, attributed
  units, committed active assault, normal-death evidence, recovery-required
  state, and one-time resolution. A successful `AssaultActive` transition is
  still the commitment point.
* `GameTick` is the global environmental clock. It advances at ten ticks per
  second; a world day is 2,400 ticks. The crisis records both global ticks and
  online-active crisis ticks because disconnect and Offline Protection can make
  them diverge.
* Pre-assault progress and launch require the owner online. Offline Protection
  freezes pre-assault personal state. After launch, an ordinary disconnect does
  not stop the assault; villagers or connected helpers may continue combat and
  may resolve it while the owner is offline. Safe Logout remains unavailable
  during `AssaultActive`.
* The production default remains `SurvivalDirectorMode::PersonalCrisis`.
  Personal mode has no scheduled dusk settlement horde or old automatic crisis
  ladder. `Legacy` remains selectable and its automatic rat, wolf, goblin,
  undead, Pillager, nightly, and legendary systems remain registered behind the
  existing director gate.
* Global day/night, weather, visibility, world-time packets, the shipwreck
  introduction and its follow-ups, resources, harvesting, crafting, farming,
  refining, structures, trade, and villager work remain separately scheduled.
  Checkpoint 1 does not make telemetry an input to any of them.

### Exact Checkpoint 1 baseline pressure configuration

Pressure is recomputed from current facts, not incrementally awarded. If
`PlayerIntroState.danger_unlocked` is false, the complete breakdown is zero.
Once it is true, the current contributors are:

| Authoritative fact | Pressure | Exact semantics |
| --- | ---: | --- |
| Introduction danger unlocked | 10 | Base contribution and safety gate |
| At least three completed owned structures | 20 | One threshold; foundations do not count |
| At least one living owned villager | 15 | Dead villagers do not count |
| `explore_poi` objective complete | 10 | Current objective fact |
| `choose_expansion` objective complete | 15 | Current objective fact |
| Stored gold at 25 / 50 / 100 | 5 / 10 / 15 | Sum across completed owned `Storage` inventories |
| Bound sanctuary level | 2 per level | Capped at 10; current maximum level is 5 |
| Total online-active ticks at 600 / 1,800 / 3,600 | 5 / 10 / 15 | One current tier, not cumulative awards |

The maximum raw sum is 110. The authoritative pressure is clamped to 100.
Completed-structure and living-villager objective flags are not counted again.
An unfinished or dead storage does not contribute gold, and observation does
not consume or move inventory.

Two useful consequences follow directly from the formula, but are not yet
outcome conclusions:

* A player with only danger unlock and time can reach at most pressure 25, so
  time alone can enter `Signs` but cannot enter `Pressure`.
* The non-time facts can sum to 95. A substantially developed settlement can
  therefore satisfy every pressure threshold; minimum phase time still applies.

### Exact Checkpoint 1 baseline phase and launch configuration

Transitions are strictly ordered and limited to one per evaluation:

| Transition | Required pressure | Minimum online-active time in current phase |
| --- | ---: | ---: |
| `Dormant` → `Signs` | 20 | None |
| `Signs` → `Pressure` | 45 | 600 ticks / 60 seconds |
| `Pressure` → `Preparing` | 70 | 1,200 ticks / 120 seconds |
| `Preparing` → `AssaultReady` | 90 | 1,800 ticks / 180 seconds |

Each transition resets `phase_online_ticks`. Entering `Preparing` turns on the
existing warning state. `AssaultReady` requires another 300 online ticks / 30
seconds before launch is allowed. At that point the preferred launch window is
global dusk or night: ticks 2,000 through 2,399 and ticks 0 through 399 within
the 2,400-tick day. At 1,200 online-ready ticks / 120 seconds the maximum-wait
fallback permits launch at any global time. Offline or protected intervals do
not earn these online ticks.

### Exact current anchor, spawn, and assault configuration

The settlement anchor priority remains:

1. the live monolith named by the hero's exact `BoundMonolith`;
2. a completed owned `Campfire` or `Storage`;
3. another completed owned structure; then
4. the live hero position only when the run has a real `SpawnPositions` entry.

With a bound sanctuary, spawn candidates use offsets one through three outside
that sanctuary's weak radius. The weak radius is `5 + sanctuary level`, so the
effective candidate radii are `6 + level` through `8 + level`. Other anchors
use radii six through eight. Candidate selection is bounded to 96 shuffled
positions and rejects out-of-map, impassable, occupied, duplicate, unpathable,
and neighbouring-settlement-footprint positions. The exact neighbouring-owner
predicate excludes a position when `Map::dist(candidate, structure) < 3`, so
distances zero through two are excluded and distance three is allowed. Failure
to find the full set leaves the crisis ready and does not consume a generation.

The complete first wave remains exactly:

```text
2 Wolf Riders
1 Goblin Pillager
```

Each unit carries `CrisisAssaultUnit { owner_player_id, assault_id,
spawn_generation }`. `Encounter::spawn_npc` provides ordinary loot and combat;
the personal wave replaces the spawned viewshed with range 14 and retains the
ordinary owner-filtered combat brain. It does not install the legacy Wolf Rider
steal behavior or Pillager torch/burning behavior.

| Template | HP | Stamina | Base damage | Damage span | Defence | Speed | Template vision | Personal-wave vision | Kill XP |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Wolf Rider | 75 | 250 | 6 | 5 | 5 | 6 | 4 | 14 | 300 |
| Goblin Pillager | 55 | 200 | 5 | 4 | 4 | 5 | 3 | 14 | 250 |

The two Wolf Rider instances and the Goblin Pillager instance are all created
through `Encounter::spawn_npc`. That generic path calls
`Encounter::generate_loot` exactly once for each unit before it is spawned, so
each of the three wave units receives its own independent set of rolls from the
same ordinary-NPC loot list. The Wolf Rider and Goblin Pillager object
templates contain combat and movement fields but no template-specific loot
field; template, family, crisis phase, and assault ownership do not alter this
list.

| Ordinary loot entry | Configured drop rate | Code quantity range | Quantity actually created on success |
| --- | ---: | --- | ---: |
| Valleyrun Copper Dust | 0.20 / 20% | `1..5` | 1–4 |
| Amitanian Grape | 0.50 / 50% | `1..3` | 1–2 |
| Copper Training Axe | 0.02 / 2% | `1..2` | 1 |
| Honeybell Berries | 0.99 / 99% | `5..10` | 5–9 |
| Mana | 0.75 / 75% | `1..3` | 1–2 |
| Gold Coins | 0.99 / 99% | `1..10` | 1–9 |
| Soulshard | 0.99 / 99% | `1..2` | 1 |

For every entry, the code draws a fresh `f32` from `[0, 1)` and creates the
item only when `drop_rate > roll`. A successful entry then makes a separate
quantity draw with `gen_range(min..max)`; this Rust range is half-open, so the
configured `max` is never produced. The resulting item stack is placed in the
NPC's inventory at spawn time rather than rolled on death, and normal combat
death leaves that pre-generated inventory on the corpse. Loot and quantity
draws use production `rand::thread_rng`; the headless run identifier and
deterministic scenario order do not seed them, so repeated matrix rows need not
receive the same loot.

The personal scorer admits the owner's living human units and blocking walls.
It does not target ordinary structures, storage, or neutral-ID monoliths. Both
target selection and the final action boundary reject cross-owner targets.
Consequently, current “structure damage” from the personal assault is primarily
wall damage; the baseline must not imply that ordinary buildings were exposed
when the attacker's targeting rules exclude them.

### Current heroes, villagers, defences, and sanctuary

New runs use the following novice hero templates:

| Class | HP | Stamina | Mana | Base damage | Damage span | Defence | Speed | Vision | Runtime starting combat equipment |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| Warrior | 110 | 110 | 0 | 2 | 2 | 4 | 5 | 3 | Sharpened Stick; runtime-created Copper Helm with defence 3 |
| Ranger | 80 | 120 | 0 | 1 | 3 | 1 | 7 | 5 | Runtime-created Training Bow with damage 8, attack range 2, accuracy 85, hunting 2 |
| Mage | 60 | 100 | 100 | 1 | 2 | 0 | 5 | 4 | Sharpened Stick and 5 Mana |

All three start with equipped Tattered Shirt and Tattered Pants, a Crude Torch
(equipped off-hand only for a dusk/night spawn), and one runtime-created Health
Potion with healing 10. That runtime potion value intentionally differs from
the generic item-template healing value of 50; the baseline records the actual
starting instance rather than silently treating the YAML value as its effect.

`Human Villager` currently has 500 HP, 10,000 stamina, zero base damage, zero
damage span, zero defence, zero speed, vision two, and base work 25. A villager
is counted as combat-capable only when its current base damage is positive or it
has an equipped weapon. Merely being alive does not imply it can kill an
attacker.

| Existing defence | Base HP | Base defence | Other current behavior |
| --- | ---: | ---: | --- |
| Stockade | 20 | 0 | Blocking wall; level 0 |
| Palisade | 200 | 0 | Blocking wall; level 1 |
| Fieldstone Walls | 400 | 0 | Blocking wall; level 2 |
| Watchtower | 50 | 0 | Vision 5 / light support; not a wall |

The sanctuary maximum remains level five. Upgrade costs are 3, 6, 9, 12, and
15 Soulshards (45 total). Full and weak radii are `3 + level` and `5 + level`.
Each level contributes 0.25 to the existing sanctuary defence amplifier. These
rules are observed, not changed or exposed as a runtime balance interface.

## Instrumentation architecture

### Read-only configuration snapshot

`crisis_balance.rs` defines
`GoblinCrisisBalanceConfigSnapshot`. The public
`goblin_crisis_balance_config_snapshot()` constructor in `game.rs` fills it
from the same constants used by pressure, transition, launch, composition, and
spawn code. It contains every pressure contribution and tier, pressure cap,
phase threshold/minimum, ready grace and maximum wait, launch-window name,
composition, wave vision, spawn distances/offsets, neighbour exclusion, and
candidate limit.

This is serialized into each balance row and the aggregate report and is used
by tests to detect drift. It is not inserted as an editable production resource,
does not accept environment or network overrides, and is never read by gameplay
as a replacement for the constants.

### Pressure telemetry

The authoritative calculator now first produces `CrisisPressureBreakdown` and
the legacy integer helper returns its `clamped_total`. The fields are
`danger_unlocked`, `structures`, `villagers`, `explore_poi`,
`choose_expansion`, `stored_gold`, `sanctuary`, `online_time`, `raw_total`, and
`clamped_total`. `raw_total` is the saturating sum of the eight contributors;
clamping remains visible.

Snapshots are retained for creation, each phase transition, assault launch,
resolution, the latest/end-of-run observation, and an optional bounded periodic
stream. The periodic interval is disabled by default and enabled only by the
headless balance runner, currently every 600 ticks. A repeated observation of
the same interval tick is deduplicated.

### Phase timing and warning telemetry

`CrisisPhaseTimingTelemetry` records the first global and first online-active
tick for crisis creation and every phase. First-write semantics keep duplicate
evaluation, reconnect, and repeated resolution from replacing history. Derived
durations remain `Option<i32>`: a phase that was not reached is `None`, not
zero. The derived values are Dormant, Signs, Pressure, Preparing,
AssaultReady, assault, total crisis, and total online-active time before launch.

Warning telemetry records the first successfully queued structured
`crisis_status` packet for `Signs`, `Preparing`, `AssaultReady`, and
`AssaultActive`. It records global tick, online-active tick, owner-online state,
and whether the hero was near the settlement. Preparing-to-launch and
ready-to-launch online lead times are derived from those successful sends.
This measures server delivery to its connection queue, not client rendering,
reading, comprehension, or the unreliable arrival time of a separate Notice.

### Preparation snapshots and observable actions

The read-only snapshot system runs after authoritative crisis evaluation and
the assault lifecycle. It records first snapshots on entry into `Preparing`,
entry into `AssaultReady`, and launch, plus the latest state at run end when no
resolution exists. The first `Resolved` sample replaces the prior end snapshot
and is then preserved, so later run ticks cannot rewrite resolution health or
settlement facts. It queries only the owning run and records:

* class/template and current/max hero HP;
* equipped weapon and equipped armor count;
* healing, food, and drink quantities;
* completed structures versus unfinished foundations;
* wall count, current/max wall HP, Stockades, Palisades, and Watchtowers;
* living and combat-capable villagers;
* exact bound sanctuary level;
* completed-storage gold, food, and total item units; and
* whether the hero is inside the selected settlement-near radius.

No full inventory is serialized. Reads use current quantities and do not mutate
an inventory, job, assignment, structure, or crisis.

During `Preparing` and `AssaultReady`, adjacent observations record positive,
observable deltas only: completed structures and walls, structures whose HP
increased, equipment changes, healing-item increases, new living villagers,
assignment changes, sanctuary-level increases, positive run-item and storage-
item deltas, online ticks near/away, return after an away warning, and whether
any such action occurred. These are observations, not inferred intent. The
resource and storage counters are gross positive deltas and can overstate net
gathering when items move among inventories; repair is a count of structures
with an HP increase between observations, not repair-resource cost.

### Assault damage, destruction, and kill attribution

The smallest available exact hook is at the existing combat application
boundary. Combat query items now expose optional `CrisisAssaultUnit` and emit a
`CrisisCombatTelemetryEvent` only when attacker or target is attributed. The
event contains object IDs, player ownership, subclasses, attribution, effective
HP lost, and whether the target died. It covers ordinary attacks, spells, and
the direct player ability path without changing their damage calculations.

The observer accepts only the `assault_id` and generation currently recorded
for the attribution's owner. Incoming damage counts only when the attributed
source's owner also owns the target. Outgoing kills count only when the killed
target is a current attributed attacker. ID sets deduplicate attacker defeats,
damaged/destroyed structures, and killed villagers; hero deaths are deduplicated
by observed alive-to-dead lifecycle transitions. A hero-death transition counted
while `AssaultActive` is phase-bounded rather than killer-source-attributed, so
it can include an ambient or survival death during the assault interval; exact
crisis-attributed damage and attacker defeats remain separate fields.

The outcome records launch/resolution, initial/defeated/remaining units,
duration, hero damage and deaths, whether the hero was alive at first resolution,
villagers and walls at launch, villager death and damage, structure
damage/destruction, wall destruction, owner-player, villager, non-owner-player helper,
and other-defence kills, disconnect/reconnect, offline resolution, Safe Logout
before launch, helper participation (including nonlethal helper damage), and any
cross-player target violation observed at this final boundary.

Ambient, legacy, other-owner, PvP, and environmental damage lack a matching
current attribution and are excluded. Controlled cleanup and True Death cleanup
do not apply normal combat damage or produce an attributed normal-death event,
so they are excluded. This intentionally avoids a broad combat rewrite.

### Lifecycle cleanup

Balance telemetry and its previous-observation cache are runtime-only,
player-keyed resources. Successful fresh-run setup removes the previous run's
entries. True Death uses the existing run cleanup boundary. Nothing is written
to PostgreSQL or the dynamic scene, and no per-tick database or log stream was
added.

## Headless scenarios

The existing `Bot` and production plugin schedule remain the harness. Scenario
labels affect only bot policy, run metadata, and explicit harness actions.
They are not production configuration.

The implemented balance cycle contains twelve driver variants across Warrior,
Ranger, and Mage. The progression-cohort flag is part of each row because a
staged assault probe must never be mistaken for a natural preparation path.

| Scenario / cohort | Current bounded driver |
| --- | --- |
| `passive` / natural | Maintains only existing emergency survival behavior; does not build, recruit, hire, upgrade, or explore voluntarily |
| `basic_survival` / natural | Existing survival/gathering behavior; builds a Campfire from starting Stick and Resin, with no planned walls, villagers, hires, or sanctuary upgrade |
| `prepared_solo` / natural | Attempts up to three Stockades and sanctuary upgrades, has no villagers/hiring, and returns home during Preparing/Ready |
| `fortified_solo` / natural | Attempts up to six Stockades and sanctuary upgrades, has no villagers/hiring, and returns home during Preparing/Ready |
| `no_villagers` / natural | Same six-wall policy as fortified, explicitly without recruitment/hiring |
| `villager_supported` / natural | Six-wall policy plus shipwreck recruitment and legitimate merchant hiring, sanctuary upgrades, and return home |
| `prepared_solo` / staged attainable facts | Three-wall policy after the shared staged progression fixture |
| `fortified_solo` / staged attainable facts | Six-wall policy after the shared staged progression fixture |
| `no_villagers` / staged attainable facts | Six-wall policy without recruitment after the shared staged progression fixture |
| `villager_supported` / staged attainable facts | Six-wall/recruit/hire policy after the shared staged progression fixture |
| `ordinary_disconnect` / staged attainable facts | Three-wall policy; after authoritative `AssaultActive`, disconnect for 100 actual updates, then reconnect if the run continues |
| `safe_logout_before_assault` / staged attainable facts | Three-wall policy; after authoritative `AssaultReady`, normalize the existing Safe Logout eligibility fixture, protect for 250 actual updates, reconnect, then continue |

The staged fixture is headless-only and explicit. It marks the two existing
`explore_poi` and `choose_expansion` objective facts complete, unlocks danger,
relocates the nearest existing monolith to the base and sets sanctuary level
three, adds 18 existing Springbranch Maple Logs to the hero, and adds 100
existing Gold Coins to a completed owned storage. The bot must still create and
complete Stockades using normal player events. The authoritative pressure
calculator, ordered phase minima, ready grace, launch window, spawn selection,
wave composition, AI, damage, and resolution systems are not bypassed. These
rows measure phase gates and combat after a controlled attainable setup; they
do not measure natural launch probability or prove an organic preparation path.
Monolith relocation also changes the anchor and spawn geometry and is therefore
reported as a limitation.

The reporting bot previously encoded Stockade as `Stick ×3`, conflicting with
the authoritative object template's `Log ×3`. Checkpoint 1 corrects only that
headless-driver recipe and makes the driver retry a normal `Build` event when a
combat lock rejected or interrupted the previous attempt. Production recipes,
costs, structure systems, and the resource economy are unchanged.

The Checkpoint 1 matrix order is deterministic and cycles twelve variants × three classes,
for 36 combinations per repetition. `crisis_balance_run_id` contains scenario,
class, natural/staged cohort, repetition, and run index. The Checkpoint 1 runner
did not inject a production RNG seed: bot choices and matrix order are
deterministic, but spawn candidate shuffle, template image, and other production
`thread_rng` use mean repeated simulations are not bit-for-bit seeded replays.
Reports must call these deterministic drivers with repeated production
randomness, not deterministic seeded worlds.

`helper_supported` and `adjacent_settlement` labels exist for schema stability
but are not in the Checkpoint 1 runner cycle. The Checkpoint 1 one-bot view did
not provide a clean independent helper policy or a second legitimate settlement
progression path. Fabricating either would make its outcome less credible than
documenting the omission. Existing focused ownership tests remain the stronger
evidence for isolation until a faithful multi-bot driver exists.

Scenario policy describes attempted behavior, not guaranteed achieved state.
For example, a villager may not be recruited, six walls may not finish, or a
weapon upgrade may not be available before the tick cap. Reports therefore
group by scenario, natural/staged cohort, and actual preparation facts. They
must not treat a label or staged fixture as proof of organic preparation.

The Safe Logout driver deliberately moves the hero into its bound sanctuary and
moves every currently alive, visible-target NPC to a distant map corner so the
existing ten-second eligibility flow can complete. The fixture also rebases the
hero's headless-only recent-combat and recent-damage observations to one tick
older than the production cooldown; it does not change the cooldown or any
production eligibility rule. A later spawn or new damage can still reject or
cancel the request; the bounded driver retains that typed outcome and its
production telemetry as an ordinary row instead of replacing the run with panic
metrics. This normalization is appropriate for freezing and later-resume
observation, but it changes activity history and ambient positioning. Its row
can test whether crisis clocks and later assault state resume correctly; it is
weaker evidence for comparing ambient-combat difficulty with an unmodified
prepared row.

## Runner and report design

The existing `headless_runs.json` remains a flat vector of `RunMetrics` rows.
All existing fields are preserved. Eight additive JSON fields carry the balance
label, class, deterministic run identifier, tick cap, exact cap-reached flag,
explicit staged-fixture flag, constant-derived config, and nested
`CrisisBalanceTelemetry`.

The existing CSV prefix is also preserved without renaming or reordering: its
48 pre-Safe-Logout columns and 25 Safe Logout columns remain the first 73
columns. Checkpoint 1 appends 118 balance columns (191 total) grouped as:

* scenario, class, run ID, tick cap, exact cap-reached flag, staged-fixture flag,
  and serialized config;
* global/online phase entries and derived durations;
* raw/clamped pressure and every contributor;
* launch-or-latest preparation facts and preparation actions;
* launch, resolution, damage, losses, kill attribution, disconnect/helper, and
  cross-owner invariants; and
* warning delivery and near-settlement timing.

An absent optional tick is an empty CSV cell and JSON `null`, not numeric zero.
The `goblin-balance` runner mode is additive to `standard`, `safe-logout`, and
`safe-logout-matrix`.

The dedicated report output is `goblin_crisis_balance_report.json`; the human
report is `docs/goblin_crisis_balance_baseline.md`. Both are produced from the
same aggregate model. Groupings are scenario, natural/staged progression
cohort, scenario × cohort, hero class × cohort, prepared/unprepared,
villagers/no villagers × cohort, connected/disconnected, and helper/solo. Each
aggregate includes its run count, eligible sample counts for optional durations,
launch and resolution numerators/rates, hero-alive-at-resolution samples, mean
and median timings where samples exist, mean damage/loss values, pressure
contributor counts, preparation-action frequency, panic count, and invariant
violations. Rates are never presented without numerator and denominator.

Panicking and timed-out runs remain rows and count in the sample summary; they
must not be silently discarded. Optional timing means/medians use only rows in
which that phase exists and report the resulting sample count. This is necessary
to avoid turning “phase never reached” into a false zero-duration sample.

## Files affected by Checkpoint 1

The implementation boundary is:

* `sp_server/src/crisis_balance.rs` — new read-only data types, configuration
  serialization model, pressure/timing/preparation/warning/outcome telemetry,
  and narrow attributed-combat observer.
* `sp_server/src/lib.rs` — exposes the instrumentation module.
* `sp_server/src/game.rs` — constant-derived snapshot, centralized pressure
  breakdown, transition/launch/resolution observations, preparation snapshot
  system, successful status-delivery timestamps, runtime resources, and
  ordering. Existing numeric values remain unchanged.
* `sp_server/src/game_tests.rs` — focused authoritative pressure-breakdown and
  constant-drift regressions.
* `sp_server/src/combat.rs` — optional attribution in combat queries and a
  narrow post-damage event for exact effective damage and death.
* `sp_server/src/player.rs` — the matching direct ability-damage hook and
  fresh-run removal of runtime balance observations.
* `sp_server/src/headless.rs` — crisis phase in the bot view, balance telemetry
  access/sample interval, and additive `RunMetrics` fields.
* `sp_server/src/headless_bot.rs` — reporting-only scenario policies layered on
  the existing bot.
* `sp_server/src/bin/headless_runner.rs` — `goblin-balance` mode, class/scenario
  cycle, focused disconnect/protection drivers, appended schemas, aggregate
  analysis, and machine/human report generation.
* `docs/goblin_crisis_balance_milestone.md` — this architecture and checkpoint
  record.
* `docs/goblin_crisis_balance_baseline.md` — generated runtime findings and
  limitations.
* `sp_server/goblin_crisis_balance_report.json` — generated machine-readable
  aggregate baseline. The ignored `headless_runs.csv` and `headless_runs.json`
  remain runner intermediates rather than committed deliverables.

Focused tests live beside the code they prove. No map, template, recipe,
resource, farming, refining, structure, trade, villager-AI, network protocol,
client, database-schema, deployment, or infrastructure file is selected for a
gameplay semantic change.

## Repository conflicts and selected resolutions

| Requested analysis | Repository reality | Checkpoint 1 resolution |
| --- | --- | --- |
| Seeded deterministic repetitions | The bot and matrix order are deterministic, but production systems use `thread_rng` and the runner has no injectable world seed | Record a stable run identifier and repeat bounded runs; disclose that worlds are not bit-identical instead of refactoring production RNG |
| Prepared and fortified scenarios | Existing content cannot guarantee best gear, repairs, walls, villagers, or sanctuary tiers before a cap | Drive legitimate attempts and report actual snapshots/actions; do not assign a synthetic prepared score or treat the label as achieved state |
| Natural progression versus assault outcomes | The bounded natural bot path may never combine enough current pressure facts to launch, which would leave combat/disconnect/Safe Logout outcome fields unexercised | Preserve natural rows, add separately flagged staged-attainable-facts probes, never mix their launch probability or time-to-preparation conclusions, and disclose the fixture and changed anchor geometry |
| Headless Stockade recipe | The reporting bot used `Stick ×3`, while the authoritative Stockade template requires `Log ×3`; a combat-locked Build rejection could also strand a foundation forever | Correct the headless-only recipe and retry normal Build events while the foundation remains incomplete; do not change the template, production build system, resources, or costs |
| Helper-supported comparison | One `Bot` controls one player and the existing helper fixture is a focused combat helper, not a complete second-player policy | Keep the label but omit it from the baseline matrix; retain focused helper attribution tests and mark outcome balance insufficient |
| Adjacent-settlement comparison | There is no bounded driver that develops a second legitimate settlement alongside the owner | Keep the label but omit it; rely only on focused ownership/isolation regression tests, not a fabricated balance row |
| Total settlement destruction | Personal attackers target human units and blocking walls, not ordinary buildings, storage, or monoliths | Measure exact attributed wall/structure damage that can occur and state the target-selection boundary; do not interpret zero ordinary-building damage as strong defence |
| Warning usefulness | Server can timestamp a successfully queued structured packet, not when a person sees or understands it | Measure queue delivery and online/near-settlement lead time; defer copy/usability conclusions requiring client or human study |
| Villager contribution | A living Human Villager has zero base damage and may be unarmed | Record both living and combat-capable counts and actual villager kills; never equate recruitment with combat contribution |
| Safe Logout balance parity | The deterministic eligibility fixture repositions the hero and every currently alive, visible-target NPC and rebases headless recent-activity observations beyond the unchanged cooldown; a later spawn or new damage can still cancel | Use it to validate freeze/resume, retain typed rejection/cancellation telemetry, and disclose activity-history/ambient-position bias; do not infer equal combat conditions from the label alone |
| Runtime persistence | Crisis, Safe Logout, and balance telemetry are process-memory state | Add no partial database migration; a server restart cannot continue a report row |

## Validation matrix and evidence policy

Checkpoint 1 requires the following before its status can be advanced:

* pressure sum, clamping, contributor-category, missing-fact, non-mutation, and
  repeated-evaluation tests;
* phase first-write, online-time, missing-phase, reconnect, protection pause,
  and one-time resolution tests;
* preparation counts for built/foundation, wall HP, living villagers,
  equipment, healing, sanctuary, storage, and inventory non-mutation;
* current-attribution damage, unrelated/other-owner/cleanup exclusion,
  destruction/death deduplication, kill roles, offline resolution, and duplicate
  resolution tests;
* JSON/CSV prefix preservation, additive serialization, `None`, scenario,
  config drift, aggregate sample count, mean, and median tests;
* personal-crisis, composition, pressure, thresholds, Safe Logout,
  disconnect, target isolation, no-dusk-horde, and legacy regressions; and
* existing combat, economy, crafting, farming, refining, villager, cleanup, and
  headless suites.

The executed base matrix cycled all 12 implemented driver variants across all
three hero classes: 36 rows at a 20,000-tick cap, exactly one observation per
driver-variant/class cell. All 36 rows were quantitative and none panicked; 33
reached the overall tick cap and 28 were still crisis-unresolved at that cap.
Natural progression launched no assaults in 18 rows. The separate
staged-attainable-facts cohort launched 10 assaults in 18 rows and resolved 6 of
those 10. This small, heavily censored sample supports instrumentation and
obvious-regression findings, not final balance claims.

### Validation record

All commands in this record were run from `sp_server/` unless noted otherwise:

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed. |
| `cargo check` | Passed. |
| `cargo test` | Passed: 424 library tests, 14 `headless_runner` tests, and 6 day-system integration tests; 0 failed. The one documentation test remained ignored. |
| `cargo clippy --all-targets --all-features` | Passed with exit status 0. The existing warning backlog remains: Clippy reported 1,333 library warnings, 1,348 library-test warnings including duplicates, 2 runner-binary warnings, and 3 runner-test warnings including duplicates. |
| `cargo test crisis_balance::tests --lib` | Passed: 12/12. |
| `cargo test --bin headless_runner` | Passed: 14/14, including schema-prefix, missing-value, aggregate, grouping, panic-row, and report-generation coverage. |
| `cargo test crisis_balance_sampler_records_authoritative_preparation_deltas --lib` | Passed: 1/1. |
| `cargo test checkpoint4_normal_packet_progression_and_runtime_telemetry_headless --lib` | Passed: 1/1. |
| `cargo test checkpoint3_attributed_npc_attack_continues_after_owner_disconnect --lib` | Passed: 1/1. |
| `cargo test checkpoint3_missing_live_unit_stays_committed_and_requires_recovery --lib` | Passed: 1/1. |
| `cargo run --release --bin headless_runner -- 36 20000 goblin-balance` | Passed: wrote 36/36 quantitative rows with 0 caught panics and generated both baseline reports. |
| CSV schema/row audit | Passed: 36 rows, 191 columns, every row width 191, with the legacy prefix intact. |
| JSON and Markdown artifact audit | Passed: report schema version 1, 36 quantitative rows, 0 panics, warning aggregates present, all reported invariants zero, and the near/away sampling limitation disclosed. |

The final matrix recorded 0 automatic dusk hordes, duplicate assault launches,
cross-player target violations, crisis invariant failures, Safe Logout
invariant recoveries, and panics. Safe Logout requests/acceptances/completions/
resumptions were 2/2/2/2, with 0 cancellations in those completed scenario
rows. Both ordinary-disconnect assault rows reconnected; no assault resolved
while its owner was offline in this sample.

## Known Checkpoint 1 limitations

* The baseline is one 36-row base cycle: one observation per 12
  driver-variant by three-class cell. Thirty-three rows reached the overall tick
  cap and 28 were still crisis-unresolved at that cap, so the outcome sample is
  heavily censored and unsuitable for final tuning conclusions.
* Production randomness is not seeded by the runner, so repeated rows are
  bounded samples rather than exact replays.
* In the Checkpoint 1 baseline, helper and adjacent-settlement balance scenarios
  were unimplemented; only focused functional ownership/helper tests were
  available. Checkpoint 2 adds a staged helper driver but continues to omit the
  adjacent-settlement balance scenario.
* Scenario labels describe driver policy, while actual preparation can fail or
  remain incomplete. Raw launch snapshots are the reliable comparison facts.
* Staged-attainable-facts rows directly establish objective/danger facts,
  relocate and level the nearest monolith, and supply existing Logs and Gold.
  They exercise authoritative phase/launch/combat mechanics but are not natural
  launch-rate, time-to-preparation, or organic solo-completability evidence.
* The existing bot is primarily a melee/close-threat survival bot. It does not
  model a skilled human's full class-specific ability, kiting, crafting, repair,
  or tactical wall use, so class results are bot-policy results rather than a
  complete class ceiling.
* The current assault does not target ordinary buildings, storage, or the
  monolith. Structure-damage conclusions are therefore mostly wall conclusions.
* Warning timestamps prove successful server queueing, not rendering,
  comprehension, or whether the warning copy is sufficient.
* Near/away preparation time is interval-sampled. Each elapsed interval is
  assigned wholly to the location observed at its endpoint (600 ticks in the
  balance matrix), so it is directional rather than an exact movement trace.
* Preparation actions are snapshot deltas, not intent. Gross positive resource
  deltas can include transfers, and a repair count is not repaired HP or cost.
* Combat attribution covers the existing ordinary/direct attack and spell
  application paths. Future damage mechanisms must explicitly emit the same
  narrow event or remain outside these totals.
* Runtime-only state is lost on process restart. There is no database schema or
  external analytics service.
* One first-wave crisis exists per current run; this checkpoint does not measure
  repeat-crisis or long-term campaign scaling.

## Checkpoint 2 — pressure and phase pacing

### Selection basis

The original Checkpoint 1 cycle nominated reachability but was not large enough
to select a value: 0/18 natural rows launched while 10/18 separately staged
rows launched. Checkpoint 2 therefore first repeated an expanded old-config
control with three repetitions of each 13-variant by three-class cell.

That 117-row control confirmed the issue:

* 0/54 natural rows launched;
* all 54 natural rows entered `Signs`, 17 entered `Pressure`, and none entered
  `Preparing` or `AssaultReady`;
* natural latest pressure was mean 33.6 and median 27;
* natural developed-policy medians were 31 for prepared solo, 31 for fortified
  solo, 29 with no villagers, and 50 with villagers; and
* staged Pressure lasted mean 2,298.4 global ticks, median 2,999 (n=63), while
  rows waited for the 70 threshold after completing the 1,200-online-tick
  minimum.

No pressure contributor completely dominated. Natural dominant contributors
were online time in 39 rows and structures in 15; the exact formula remained
deterministic and understandable. The smallest safe change was therefore to
preserve every weight and lower only the unreachable late fact gates.

### Exact selected values

| Transition | Checkpoint 1 | Checkpoint 2 | Evidence and design intent |
| --- | ---: | ---: | --- |
| `Dormant` → `Signs` | 20 | 20 | Unchanged. Time-only pressure reaches 25 and should still reveal Signs. |
| `Signs` → `Pressure` | 45 | 45 | Unchanged. Passive/basic natural rows remain below this boundary. |
| `Pressure` → `Preparing` | 70 | 45 | Seventeen old natural rows reached Pressure but none could begin Preparing. Equal fact thresholds make Pressure a deliberate 1,200-online-tick observation phase rather than an indefinite second fact wall. |
| `Preparing` → `AssaultReady` | 90 | 49 | A developed-solo control row reached 49, villager-supported natural pressure had median 50, and the deterministic danger 10 + structures 20 + online 15 + sanctuary level 2 × 2 path equals 49. This retains a further fact requirement after 45. |

The selected current phase path is:

| Phase transition | Required pressure | Minimum online-active time in current phase |
| --- | ---: | ---: |
| `Dormant` → `Signs` | 20 | none |
| `Signs` → `Pressure` | 45 | 600 ticks / 60 seconds |
| `Pressure` → `Preparing` | 45 | 1,200 ticks / 120 seconds |
| `Preparing` → `AssaultReady` | 49 | 1,800 ticks / 180 seconds |

Ready still requires 300 online ticks before launch, still prefers global dusk
or night, and still falls back after 1,200 online-ready ticks. Pressure cap 100,
every pressure weight/tier, warning severity/cadence, assault composition,
spawn/identity/target rules, combat, loot, rewards, and economy remain
unchanged.

### Architecture after Checkpoint 2

The crisis architecture is unchanged:

* `personal_crisis_system` remains the only pressure/ordered-phase authority.
  It derives the complete score from current facts and may advance at most one
  phase per evaluation.
* Sharing pressure 45 cannot skip `Pressure`: transition resets the phase clock,
  and the next transition remains blocked for 1,200 online ticks. Preparing
  then remains blocked for 1,800 online ticks and pressure 49.
* `personal_crisis_assault_system` remains the only launch/identity/composition/
  resolution authority. Owner-online and Safe Logout barriers are unchanged.
* Global `GameTick`, environmental day/night, launch-window observation,
  weather, visibility, and world-time delivery remain independent. Personal
  mode still schedules no automatic dusk horde; Legacy scheduling is retained.
* Structured status delivery remains deduplicated. Only the AssaultReady
  summary and existing desktop countdown terminology changed: the 300 ticks are
  now called a **minimum warning**, and zero renders as `complete` rather than
  implying an exact attack ETA.
* The comparison harness now observes a specified player and drives a real
  connected helper with ordinary movement/combat events. It grants no combat
  stats, teleport, fabricated damage, or target exemption beyond production's
  existing personal-owner target filter.

### Expanded matrix and comparison artifacts

The cycle expands from 12 to 13 variants by adding a staged helper-supported
row. Each full side is 13 variants × 3 classes × 3 repetitions = 117 rows:
54 natural and 63 staged. Old and candidate runs have separate immutable-purpose
paths:

* `sp_server/goblin_crisis_balance_checkpoint2_control_report.json` — exact old
  70/90 configuration;
* `sp_server/goblin_crisis_balance_checkpoint2_candidate_report.json` — exact
  new 45/49 configuration; and
* `docs/goblin_crisis_balance_checkpoint2.md` — human old/new comparison,
  status labels, confidence, and Checkpoint 3 recommendations.

The CLI side is fail-closed against the embedded threshold pair. A current
candidate binary cannot overwrite `control` merely by changing the filename
argument, and an old binary cannot claim to be `candidate`.

Both full sides contain 117 quantitative rows and zero caught panics. The
candidate changes were:

* natural launch 0/54 to 9/54;
* staged launch 25/63 to 63/63;
* natural completed Pressure/Preparing/Ready phases 0/0/0 duration samples to
  20/10/9;
* staged Pressure mean/median 2,298.4/2,999 to 1,200/1,200 ticks;
* unresolved-at-cap 81/117 to 67/117; and
* resolution after launch 11/25 to 21/72. The lower conditional percentage,
  44.0% to 29.2%, is a changed exposure denominator and unresolved combat issue,
  not evidence that pacing should be rolled back.

Passive and basic-survival rows remained 0/9 launches each. Candidate natural
completed phases had online-active ranges of 600–3,599 ticks for Signs,
1,200–1,200 for Pressure, 1,800–2,235 for Preparing, and 355–1,200 for Ready.
Warnings were delivered to 54/54 natural Signs, 20/20 Preparing, and 10/10
Ready observations. The nine natural launches had mean/median online warning
lead of 6,526/7,389 ticks from Signs, 2,726.7/2,590 from Preparing, and 839/790
from Ready.

All candidate staged rows launched. Their phase minima were exactly
600/1,200/1,800 online ticks, and their median online-before-launch time stayed
4,801 ticks versus the same 4,801 in control. Pressure reach improved without
erasing the designed warning sequence.

The complete metric and scenario tables, including damage, deaths, classes,
disconnect, Safe Logout, helper, pressure growth, and limitations, are in
`docs/goblin_crisis_balance_checkpoint2.md`.

### Checkpoint 2 files affected

The Checkpoint 2 delta on top of the completed Checkpoint 1 instrumentation is:

* `sp_server/src/game.rs` — two threshold constants and AssaultReady summary;
  no other gameplay value changes.
* `sp_server/src/game_tests.rs` — exact old/new drift, deterministic growth,
  one-step ordering, every phase-minimum boundary, and runtime pressure/
  telemetry equality regressions.
* `sp_server/src/crisis_balance.rs` — deserialize support for report round trips
  and the implemented helper matrix label; no gameplay authority.
* `sp_server/src/headless.rs` — player-scoped read-only observation for the real
  second bot and focused isolation coverage.
* `sp_server/src/headless_bot.rs` — helper driver using normal events and a
  helper-primary policy identical to prepared solo.
* `sp_server/src/bin/headless_runner.rs` — 39-cell cycle, real connected helper,
  three-repetition comparison support, versioned JSON paths, round-trip tests,
  and fail-closed config labeling. The historical Checkpoint 1 Markdown
  renderer remains test-only.
* `sp_frontend/sp_ts/src/sp/core/crisisStatus.ts` and its test — honest zero
  countdown formatting.
* `sp_frontend/sp_ts/src/sp/desktop/ui/objectivesPanel.tsx` and
  `objectivesPanel.crisis.test.tsx` — existing-row wording and rendered copy
  regression; no layout redesign.
* this milestone, `docs/goblin_crisis_balance_checkpoint2.md`, and the two
  machine reports — exact design/evidence record.

Checkpoint 1's `combat.rs`, `player.rs`, `lib.rs`, and telemetry additions
remain unchanged by Checkpoint 2. No map, template, recipe, resource, farm,
refining, trade, villager-AI, database, deployment, or infrastructure file was
selected for a semantic change.

### Repository conflicts and selected resolutions

| Requested comparison | Repository reality | Checkpoint 2 resolution |
| --- | --- | --- |
| Repeat old versus new | Running the current source twice would apply the same constants, and one unversioned path would overwrite Checkpoint 1 | Freeze a full old binary/report before tuning, use explicit control/candidate paths, validate the embedded config against the label, and preserve the original baseline artifacts |
| Paired repetitions | Production uses `thread_rng` and has no injectable seed | Use identical matrix order, counts, caps, and policies; describe the sides as independent repeated samples, not paired replays |
| Helper supported | Checkpoint 1 had no second action-driving bot | Spawn a real connected Warrior and drive ordinary movement/attack. Only 1/9 candidate helper rows participated and made one kill, so effectiveness remains insufficient rather than fabricated |
| Warning usefulness | Server observes successful queue delivery, not rendering or comprehension | Preserve cadence/severity, clarify only the misleading countdown semantics, and report human usefulness as low confidence |
| Pressure and Preparing both at 45 | A naïve transition loop could skip phases | Preserve one transition per evaluation and online-clock reset; add exact min−1/exact-boundary regressions |
| Control CLI after tuning | A filename-only `control` switch could mislabel 45/49 output as 70/90 | Fail closed unless the binary's Preparing/Ready pair exactly matches the selected comparison side |
| Safe Logout outcome comparison | Old thresholds allowed only one completed control protection lifecycle; candidate allowed eight | Treat the rows as lifecycle/invariant probes, not a causal combat comparison; retain focused Safe Logout tests |

### Checkpoint 2 validation record

All commands below passed. They were run from `sp_server/` unless noted:

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed. |
| `cargo check` | Passed with the existing 70-warning library backlog. |
| `cargo test --quiet` | Passed: 429 library, 16 runner, and 6 day-system integration tests; 0 failed; the documentation test remained ignored. |
| `cargo clippy --all-targets --all-features` | Passed with exit 0 and the existing backlog: 1,333 library, 1,349 library-test including duplicates, 1 runner, and 3 runner-test warnings including a duplicate. |
| `cargo test goblin_balance_checkpoint2 --lib` | Passed 2/2. |
| `cargo test goblin_phase --lib` | Passed 3/3. |
| `cargo test goblin_pressure --lib` | Passed 3/3. |
| `cargo test crisis_balance::tests --lib` | Passed 12/12. |
| Focused runtime pressure/telemetry + reconnect test | Passed 1/1. |
| Focused packet progression/telemetry test | Passed 1/1. |
| Focused ready-clock reconnect test | Passed 1/1. |
| Focused Safe Logout reconnect-barrier test | Passed 1/1. |
| Focused normal-victory assault identity test | Passed 1/1. |
| Focused PersonalCrisis no-dusk test | Passed 1/1. |
| Focused Legacy dusk and no-personal-lifecycle tests | Passed 1/1 each. |
| `cargo test --bin headless_runner` | Passed 16/16. |
| `cargo run --release --bin headless_runner -- 1 1000 standard` | Passed: 1 row, 0 panics/invariant failures. |
| `cargo run --release --bin headless_runner -- 8 2000 safe-logout-matrix` | Passed all 8 variants, 0 panics/invariant failures. |
| Full old 117 × 20,000 control matrix | Passed 117/117 quantitative, 0 panics. |
| Full new 117 × 20,000 candidate matrix | Passed 117/117 quantitative, 0 panics. |
| JSON config/sample/invariant audit | Passed: exactly two config differences, matching workload, all invariants zero. |

From `sp_frontend/sp_ts/`, the focused compile including `src/phaser.d.ts`,
both Node-executed crisis countdown/panel assertions, and
`npx tsc --noEmit --skipLibCheck` passed.

Non-final preflights are recorded, not claimed as passes: a direct release
binary invocation without runtime `CARGO_MANIFEST_DIR` could not locate the
existing map path; a shortened 21-row control probe hit the pre-existing random
`Cannot find item template: "Windstride Stag"` gather panic; and an isolated
frontend compile that omitted `src/phaser.d.ts` could not resolve the generated
global `integer` type. Corrected final commands passed, and neither full balance
matrix reproduced the gather panic. The exact commands and expanded results are
in `docs/goblin_crisis_balance_checkpoint2.md`.

### Known Checkpoint 2 limitations

* Each scenario/cohort cell has nine independent, unseeded observations. Pacing
  reachability has moderate confidence; combat, class, villager, wall, and
  protection outcomes have low confidence.
* The reports share base commit `3fa1b9a` and record a dirty tree. Their config
  snapshots prove exactly two value differences, but old/new source or binary
  hashes were not captured. Ignored raw rows were overwritten by later headless
  modes, so row-level min/max, Safe Logout attempt totals, helper kill, and the
  observed 49-point control row survive only in the contemporaneous analysis,
  not the committed aggregate JSON. Future A/B work should version raw rows.
* The candidate creates more combat exposure. Conditional resolution falls
  from 44.0% to 29.2% even as resolved count rises from 11 to 21. Checkpoint 2
  intentionally does not alter that assault.
* Only 1/9 candidate helper rows records helper participation. The real helper
  driver proves an ordinary-event path, not balanced or reliable assistance.
* The existing bot remains melee-biased, especially limiting Mage/Ranger
  interpretation. No class values changed.
* Warning delivery means server queueing. Desktop wording is covered; the
  current mobile surface still has no crisis-status presentation, and no human
  comprehension study exists.
* Staged fixture, wall-only structure targeting, snapshot-delta, near/away
  sampling, runtime-only persistence, and one-first-wave limitations from
  Checkpoint 1 still apply.
* Adjacent-settlement balance remains outside the matrix. Focused crisis
  ownership/isolation tests remain the evidence.

## Work deferred to Checkpoint 3

Checkpoint 3 owns preparation gameplay and defensive value. Based on the now
reachable warning path, it should use existing resources and systems to measure
and improve return-to-settlement, equip, craft, repair, stock, wall, sanctuary,
healing, and defender choices. It should make optional assistance reliable
without requiring multiplayer and preserve solo completion.

Checkpoint 3 must not silently retune these thresholds or implement final wave/
class combat work without new evidence. Assault composition/stats and final
class validation remain Checkpoint 4.

No Checkpoint 3 gameplay is implemented here. New enemies, objectives,
resources, buildings, recipes, loot, rewards, villager AI, crisis types,
regional crises, offline production, persistence, multiplayer scaling,
cross-world interaction, and larger maps remain out of scope.
