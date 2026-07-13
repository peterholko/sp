# Persistent Personal Crisis Foundation

## Status

Proposed implementation milestone.

## Checkpoint 1 implementation record

Checkpoint 1 is limited to director separation. The goblin crisis state machine,
online/offline crisis timing, assault suspension, crisis packets and UI, and
headless crisis metrics remain deferred to later checkpoints.

### Architecture findings

* `GameTick` is a reflected Bevy resource in `sp_server/src/game.rs`. It defaults
  to `DAWN`, is incremented by `update_game_tick`, and provides global day,
  hour, and time-of-day calculations. It is global rather than player-specific.
* `WorldPlugin` in `sp_server/src/world.rs` registers `day_system` independently
  of the settlement-danger systems. At the time boundaries defined in
  `constants.rs`, `day_system` recalculates non-NPC `Viewshed` ranges from base
  vision, time of day, equipped-item vision, and effects. It also sends the
  existing `ResponsePacket::World { time_of_day, day }`. Weather state is held
  by `WeatherAreas`; `weather_cycle_system` and `weather_effects_system` are
  registered separately in `GamePlugin` and remain unconditional in both
  director modes.
* `PlayerCrisis` and `CrisisState` in `sp_server/src/game.rs` are in-memory,
  per-player legacy state. `PlayerCrisis` contains five automatic ladder flags
  (`rat_spoilage`, `wolf_pack`, `goblin_raid`, `undead_incursion`, and
  `goblin_pillager`) plus `initial_encounter` and `spider_encounter`. Thus the
  introduction and legacy crisis ladder are not yet cleanly separated.
* `rat_event_system` checks stored food every 20 ticks after the 4,800-tick
  introduction grace. `wolf_pack_system` checks distance from spawn every 10
  ticks and has an eight-minute fallback. `goblin_raid_system` checks for 30
  stored gold every 30 ticks and has a ten-minute fallback. The undead and
  pillager systems check every 10 ticks and use three-day/16-minute and
  five-day/24-minute thresholds respectively.
* `nightly_threat_system` checks the global tick for exact `DUSK`, skips global
  day one and players younger than 4,800 ticks, then spawns a day-scaled wave
  outside the sanctuary. On a successful spawn it records the global day in a
  system-local map and increments `PlayerRunScore::waves_survived`.
* `legendary_threat_system` checks every 10 ticks. It creates the Fire Dragon
  hideout at player survival day six, activates the campaign at day seven, and
  sends recurring follower waves. `legendary_death_tracking_system` performs
  cleanup, objective, and score bookkeeping for already-existing legendary
  entities and is not itself an automatic escalation trigger.
* `GamePlugin` registers the legacy ladder, nightly horde, and legendary
  escalation as separate `Update` systems. The introductory
  `initial_encounter_system` is also separate, so it can remain active while the
  automatic systems are mode-gated. Economy plugins and their systems are
  registered independently and require no Checkpoint 1 changes.
* New-player setup in `player_setup.rs` creates `PlayerIntroState` and
  `InitialEncounterState`, including two delayed opening enemies, the
  boar/crab follow-up, spider follow-up, villager timing, merchant, and later
  necromancer data. `initial_encounter_system` still uses the two introduction
  flags in `PlayerCrisis`; this temporary coupling is retained for Checkpoint 1.
* Login enters through `PlayerEvent::Login` in `player.rs`, which schedules a
  `GameEventType::Login`; `game_event_system` later performs login perception
  and queues the sanctuary resend. Network disconnect paths remove the active
  client UUID from the shared `Clients` map. Hero entities can remain in the
  ECS, so they are not authoritative presence. Presence semantics are inspected
  but intentionally unchanged until Checkpoints 2 and 3.
* `build_headless_app` builds the same gameplay plugins without the realtime
  schedule runner or production network. `HeadlessGame` supplies in-process
  client/database channels, pumps `App::update`, exposes world observations and
  run metrics, and drives the existing deterministic bot through
  `headless_runner`.

### Checkpoint 1 affected files

* `sp_server/src/game.rs` — named director mode/configuration and mode gates on
  legacy automatic danger systems.
* `sp_server/src/lib.rs` — explicit personal-crisis default for production and
  headless app builders.
* `sp_server/src/headless.rs` — explicit mode-selecting constructor and focused
  director/introduction regression tests.
* `sp_server/src/game_tests.rs` — default-mode regression test.
* `sp_server/tests/day_system_test.rs` — personal-mode night-visibility
  regression coverage.
* `docs/persistent_crisis_milestone.md` — this architecture and configuration
  record.

No resource, recipe, crafting, farming, structure, trade, villager AI, map,
network protocol, client, database, or deployment file is changed.

### Design conflicts and selected scope

The milestone-level goal and definition of done describe the completed
four-checkpoint initiative, including a goblin phase machine, offline rules,
assault attribution, networking, and UI. Section 24 explicitly assigns those
features to Checkpoints 2 through 4. The Checkpoint 1 request is narrower, so
this implementation follows Section 24 and does not satisfy or implement those
later milestone-level items yet.

The preferred design separates introductory state from crisis state, but the
current introduction reads and writes `PlayerCrisis.initial_encounter` and
`spider_encounter`. Moving those fields is Checkpoint 2 work and would expand
this patch. Checkpoint 1 therefore retains `PlayerCrisis` unchanged, leaves
`initial_encounter_system` active in both modes, and gates `rat_event_system`
separately because it is the automatic food-spoilage tier rather than the
shipwreck chain.

### Selected configuration approach

The server uses one named Bevy resource:

```rust
enum SurvivalDirectorMode {
    Legacy,
    PersonalCrisis,
}

struct SurvivalDirectorConfig {
    mode: SurvivalDirectorMode,
}
```

`PersonalCrisis` is the `Default` and is passed explicitly by both the
production and standard headless builders. `Legacy` remains selectable by app
construction and by the headless regression harness. A shared Bevy run
condition gates only `rat_event_system`, `wolf_pack_system`,
`goblin_raid_system`, `undead_incursion_system`,
`goblin_pillager_system`, `nightly_threat_system`, and
`legendary_threat_system`. Environmental time, weather, visibility, world-time
packets, introductory encounters, legendary death bookkeeping, and the economy
remain registered in both modes.

## Purpose

This document defines the first implementation milestone in the Siege Perilous persistent-world redesign.

The milestone separates the shared environmental day/night cycle from automatic settlement attacks and establishes the foundation for player-specific crises that respect independent play schedules.

This is not the complete long-term multiplayer redesign.

The milestone is deliberately limited to:

* Preserving global environmental time
* Disabling automatic dusk hordes in the new default mode
* Replacing the current automatic crisis ladder with one explicit goblin crisis
* Establishing online/offline crisis rules
* Preserving the existing resource and production economy
* Adding focused server, client, and headless-test support

---

# 1. Product context

Siege Perilous is intended to become a persistent shared-world action-survival game.

Each player:

* Controls a hero in real time
* Builds an independent settlement
* Gathers and processes resources
* Crafts equipment
* Recruits and assigns villagers
* Survives personal and regional dangers
* Plays on an independent schedule
* Interacts with other players when their sessions overlap

The game should provide a persistent-world feeling without requiring players to coordinate sessions or log in at specific real-world times.

The guiding rule is:

> The world continues when the player leaves, but personal vulnerability must not continue toward irreversible defeat.

---

# 2. Problem statement

The current implementation contains several systems that independently control danger and progression:

* The global day/night cycle
* Automatic nightly hordes
* A tiered crisis ladder
* Survival-time fallback timers
* Day-based undead and pillager triggers
* Legendary Fire Dragon escalation
* Settlement wealth triggers
* Player objectives
* Sanctuary progression

These systems compete with one another.

Examples of resulting problems include:

* Every dusk automatically creates settlement danger.
* Personal crises and night hordes can overlap.
* Players may be forced into combat because of the world clock.
* Players who log in shortly before dusk receive an immediate mandatory attack.
* Day count advances regardless of player activity.
* Different crisis families are treated as mandatory tiers of one run.
* Offline or irregular players cannot safely participate in a persistent world.
* The player may not understand which action caused an attack.
* The game is difficult to tune because several escalation systems stack together.

The redesign assigns different responsibilities to different systems.

The global clock controls environmental conditions.

The personal crisis system controls directed settlement danger.

---

# 3. Milestone goal

Implement the foundation for this relationship:

> The crisis decides what happens. The global clock influences the conditions under which it happens.

After this milestone:

* Global day/night continues normally.
* Darkness, lighting, visibility, weather, and world time remain shared.
* Reaching dusk alone does not create a settlement horde in the default prototype mode.
* The old automatic crisis ladder does not run in the default prototype mode.
* A player-specific goblin crisis progresses through explicit phases.
* Major crisis assaults begin only while the owning player is online.
* Disconnecting cannot cause unattended irreversible settlement loss.
* The current resource, crafting, refining, farming, fishing, trade, and villager systems remain intact.

---

# 4. Current repository architecture

The relevant server architecture currently includes:

## `sp_server/src/game.rs`

Contains or coordinates:

* `GameTick`
* `PlayerCrisis`
* `CrisisState`
* Introductory encounter state
* Rat event logic
* Wolf pack logic
* Goblin raid logic
* Undead incursion logic
* Goblin pillager logic
* `nightly_threat_system`
* Legendary threat logic
* Sanctuary zones
* Run scoring
* System registration
* Player notices
* Structure processing systems
* True Death and run cleanup

The current `PlayerCrisis` structure mixes two distinct concepts:

* Introductory encounter progression
* Automatic crisis-ladder completion flags

Those responsibilities should be separated.

## `sp_server/src/world.rs`

Contains environmental world-time behaviour including:

* Time-of-day calculation
* Global day/night state
* Visibility modifiers
* Weather
* Vision changes
* World-time updates

This environmental behaviour must remain intact.

## `sp_server/src/player_setup.rs`

Contains:

* Player and hero setup
* Starting locations
* Initial world objects
* Run-spawned object tracking
* Introductory encounter setup

## `sp_server/src/network.rs`

Contains:

* Incoming network packets
* Outgoing response packets
* Login and disconnect messages
* Resource, crafting, structure, villager, trade, and information commands

The crisis redesign may add an outgoing crisis-status packet.

## `sp_server/src/headless.rs`

Contains the in-process headless game harness used to:

* Build the Bevy application
* Advance game ticks without real-time scheduling
* Capture packets
* Observe world state
* Collect per-run metrics

## `sp_server/src/headless_bot.rs`

Contains deterministic scripted player behaviour used for balance and regression testing.

## Existing economy modules

The current economy is implemented across modules including:

* `resource.rs`
* `recipe.rs`
* `farm.rs`
* `structure.rs`
* `trade.rs`
* `item.rs`
* Villager AI and utility modules
* Work queues and assignment systems

These systems are not targets for simplification in this milestone.

---

# 5. Critical constraints

The implementation must follow these constraints.

## World constraints

* Keep the current 50×50 map.
* Keep the current start-location model.
* Do not implement the future 20–25-player world yet.
* Do not implement map resizing.
* Do not implement cross-world or cross-instance interaction.
* Do not implement world migration or seasonal resets.

## Environmental constraints

* Keep the global `GameTick`.
* Keep environmental day/night.
* Keep weather.
* Keep lighting and visibility changes.
* Keep world-time packets.
* Keep night as a dangerous environmental condition.
* Do not use dusk alone as the default trigger for a settlement attack.

## Economy constraints

Preserve the current systems for:

* Gathering
* Mining
* Wood harvesting
* Stone gathering
* Hunting
* Farming
* Fishing
* Butchery
* Refining
* Smelting
* Tanning
* Milling
* Baking
* Herbalism
* Spinning
* Tailoring
* Food preservation
* Crafting
* Recipes
* Inventory
* Work queues
* Villager assignments
* Trade
* Gear progression

Do not:

* Collapse resources into generic categories
* Remove processing stages
* Replace existing materials with generic “metal,” “food,” or equivalent counters
* Add new resource families
* Add new professions
* Add new crafting tiers
* Add new currencies

The existing economy is feature-frozen but remains active and testable.

## Crisis constraints

* Use goblins as the only complete personal crisis in this milestone.
* Do not run the old mandatory wolf → goblin → undead → pillager ladder in the default mode.
* Do not run automatic legendary escalation in the default mode.
* Preserve the old systems behind a named legacy mode where practical.
* Do not delete working content merely because it is disabled in the new prototype mode.
* Keep the introductory shipwreck encounter chain functional.
* Keep its initial enemy, follow-up creature, villager, spider, and related progression intact.

## Offline constraints

* A major personal crisis assault must not begin unless the owner is online.
* Offline crisis time must not advance toward an unavoidable assault.
* Offline personal crises must not destroy the sanctuary.
* Offline personal crises must not permanently end the settlement.
* Offline personal crises must not kill the hero.
* Offline personal crises must not kill important named villagers.
* Do not implement a complex offline combat simulator.

---

# 6. Explicit survival-director modes

Introduce an explicit configuration that selects the active danger model.

Suggested conceptual modes:

```rust
enum SurvivalDirectorMode {
    Legacy,
    PersonalCrisis,
}
```

Names may follow repository conventions.

## Legacy mode

Preserves current behaviour as closely as practical:

* Existing crisis ladder
* Automatic dusk hordes
* Survival director
* Legendary escalation
* Existing day and fallback triggers

Legacy mode exists for comparison and regression testing.

It should not be the default prototype mode.

## Personal-crisis mode

Becomes the default.

In this mode:

* Environmental day/night remains active.
* Introductory encounters remain active.
* Automatic dusk hordes are disabled.
* The old automatic crisis ladder is disabled.
* Legendary escalation is disabled.
* The new personal goblin crisis is active.

The director mode should be represented by a named resource or configuration rather than scattered booleans.

---

# 7. Authoritative player presence

The server requires one authoritative way to determine whether a player is online.

Conceptually:

```rust
fn is_player_online(player_id: i32) -> bool
```

Presence must be derived from active connected clients, not merely from whether the player’s hero entity remains in the Bevy world.

Presence handling must account for:

* Login
* Reconnect
* Clean disconnect
* Unexpected disconnect
* Stale client records
* Headless test clients
* More than one client record, if the architecture allows it

Crisis launch authority remains entirely server-side.

The client must not decide whether an assault can begin.

---

# 8. Separate introductory encounters from crises

The current `PlayerCrisis` contains both crisis-ladder flags and introductory encounter flags.

The new architecture should separate them.

Preferred conceptual structure:

```rust
struct PlayerIntroEncounters {
    initial_encounter: bool,
    spider_encounter: bool,
}
```

and:

```rust
struct SettlementCrisisState {
    crises: HashMap<i32, SettlementCrisis>,
}
```

An alternative compatible implementation may retain the old `PlayerCrisis` temporarily for legacy and introduction support while adding a new personal-crisis resource.

The new personal crisis must not be implemented as additional unrelated booleans added to the old ladder.

---

# 9. Personal crisis data model

Implement an explicit crisis state machine.

Suggested conceptual model:

```rust
enum CrisisKind {
    Goblin,
}

enum CrisisPhase {
    Dormant,
    Signs,
    Pressure,
    Preparing,
    AssaultReady,
    AssaultActive,
    Resolved,
}

struct SettlementCrisis {
    kind: CrisisKind,
    phase: CrisisPhase,
    pressure: i32,
    phase_started_tick: i32,
    online_active_ticks: i32,
    warning_sent: bool,
    assault_id: Option<u64>,
    assault_unit_ids: Vec<i32>,
    resolved_at_tick: Option<i32>,
}
```

The exact structure may be adapted to repository conventions.

Required properties:

* One primary personal crisis per player
* Explicit phase transitions
* Inspectable state
* Testable transitions
* Idempotent updates
* No duplicate assault spawning
* No duplicate resolution rewards
* State can be sent to the player
* State is compatible with the existing world save/reload model
* Introductory encounter state remains separate

---

# 10. Goblin crisis progression

The first crisis should use existing goblin content and existing player progression signals.

It should not require a new quest framework.

## Pressure sources

Potential pressure signals include:

* Structures built
* Settlement wealth
* Stored gold
* Villagers recruited
* Sanctuary upgrades
* Resource extraction
* Production activity
* Objective completion
* Goblin incidents left unresolved
* Online active time

The crisis must not progress only because the global day number increased.

The implementation may use a configurable combination of:

* Settlement-development milestones
* Pressure points
* Minimum online-active time
* Unresolved goblin activity

## Dormant

The player has not yet attracted organized goblin attention.

The introductory encounter may still be active.

## Signs

The settlement has become noticeable.

Possible presentation:

* Distant smoke
* Missing goods
* Goblin tracks
* A warning notice
* A small discovery or status update

The player should understand that settlement growth has attracted attention.

## Pressure

The goblins begin interfering with the player.

Use existing content where practical, such as:

* Wolf Riders
* Theft behaviour
* Ambushes
* Small scouting groups
* Sabotage

This phase must not be settlement-ending.

## Preparing

The goblins are organizing a major raid.

The player receives a clear warning and preparation window.

Preparation should support several existing-system responses, including:

* Building or repairing walls
* Crafting weapons or armour
* Producing healing supplies
* Assigning guards
* Recruiting villagers
* Improving the sanctuary
* Defeating a pressure incident
* Stocking food and resources

The crisis should not require one exact recipe chain.

## AssaultReady

The crisis has satisfied the conditions for a major raid.

Rules:

* The assault does not begin while the owner is offline.
* The state may remain ready across disconnects.
* The player receives a warning on login or transition.
* A preparation grace period is required before launch.

## AssaultActive

A major goblin attack is active.

The initial assault must be:

* Solo-completable
* Supported by existing villagers and defences
* Clearly attributed to the crisis
* Safe against duplicate spawning
* Safe against entity-despawn races

The implementation should reuse existing enemy templates and behaviour where practical.

## Resolved

The crisis is complete after required attackers or objectives are defeated.

Resolution must happen exactly once.

Potential rewards may use existing:

* Items
* Gold
* Soulshards
* Score
* Objectives
* Resources

Do not add an entirely new reward economy.

---

# 11. Relationship with global time

Global time remains shared across the world.

The goblin crisis may prefer thematically appropriate attack timing.

Acceptable launch behaviour:

* If an assault becomes ready during daytime, it may launch at the next dusk while the owner remains online.
* If the player logs in at night with an assault already ready, provide a short grace period before launch.
* If the player logs out before launch, keep the crisis in `AssaultReady`.
* Do not require a player to remain online indefinitely waiting for a specific global tick.

The clock influences timing and atmosphere.

The clock does not independently create the assault.

---

# 12. Removing automatic dusk hordes

The current `nightly_threat_system` creates a wave when the global tick reaches `DUSK`.

In the new default personal-crisis mode:

* Do not schedule or execute that system.
* Do not spawn a wave solely because dusk occurred.
* Do not increment wave score merely because the clock reached dusk.
* Preserve the system for legacy mode where practical.
* Preserve all unrelated environmental night effects.

The following must continue functioning:

* Time-of-day display
* Vision changes
* Darkness
* Torches
* Weather
* Environmental night risk
* Nocturnal creatures not associated with scheduled settlement hordes

---

# 13. Disabling the automatic crisis ladder

In personal-crisis mode, the following old settlement-escalation systems should not run:

* Rat crisis escalation, except introductory behaviour that is genuinely required
* Wolf pack automatic tier
* Old goblin gold-raid tier
* Undead day-based tier
* Goblin pillager day-based tier
* Automatic nightly survival hordes
* Legendary Fire Dragon escalation

The legacy systems should remain available under legacy mode where practical.

Comments and tests that assume numbered days control the default prototype danger should be updated.

---

# 14. Assault entity attribution

Every NPC spawned for a personal crisis assault must be explicitly attributed.

Conceptual component:

```rust
struct CrisisAssaultUnit {
    owner_player_id: i32,
    assault_id: u64,
}
```

The implementation must not rely only on NPC faction ownership because crisis ownership is a separate concept.

Attribution is required for:

* Resolution
* Cleanup
* Disconnect handling
* Reconnect handling
* True Death interaction
* Duplicate prevention
* Metrics
* Debugging

Use existing `RunSpawnedObjs` where appropriate, but do not overload it if a dedicated crisis attribution component is clearer.

---

# 15. Disconnect during an active assault

Logging out must not erase an assault for free.

It also must not allow unattended irreversible destruction.

Preferred policy:

1. The assault remains logically active.
2. The assault enters a suspended state when the owner disconnects.
3. Crisis attackers stop applying irreversible damage to:

   * The sanctuary
   * Owner structures
   * The hero
   * Important named villagers
4. The player receives a reconnect notice.
5. The assault resumes after a short grace period when the player returns.
6. Completion and rewards remain one-time.

If safely suspending live ECS entities is impractical, use this fallback:

1. Despawn only explicitly attributed assault units.
2. Return the crisis to `AssaultReady`.
3. Preserve pressure and warning state.
4. Do not grant loot or completion rewards.
5. Relaunch once after reconnect and grace.
6. Prevent repeated disconnects from farming enemy drops or rewards.

The selected policy must be documented in this file during implementation.

---

# 16. Crisis status packet

Add an outgoing crisis-status response packet.

Suggested conceptual payload:

```rust
ResponsePacket::CrisisStatus {
    kind: String,
    phase: String,
    pressure: i32,
    title: String,
    summary: String,
    next_warning: Option<String>,
    assault_ready: bool,
}
```

Adapt field names to existing network conventions.

Send the status:

* On login
* On reconnect
* On crisis creation
* On phase transition
* On meaningful pressure changes
* On assault launch
* On assault suspension or reset
* On resolution

The server remains authoritative.

---

# 17. Minimal client presentation

Add the smallest useful client presentation.

Display:

* Crisis name
* Current phase
* Short description
* Pressure or escalation indicator
* Clear assault warning

Do not build a full quest journal in this milestone.

If a new UI panel is disproportionately expensive, use an existing status, notice, or event presentation pattern while still implementing the structured packet.

---

# 18. Resource-system integration

The resource economy remains part of the prototype.

The new crisis should use it as a source of meaningful preparation decisions.

Examples of existing-system preparation:

* Gather timber and metal for defences
* Craft improved weapons
* Produce armour
* Make healing items
* Preserve food
* Assign villagers to production
* Stock ammunition or consumables
* Upgrade the sanctuary

The crisis may observe economic activity when calculating pressure.

It must not:

* Rewrite existing recipes
* Rename broad classes of resources
* Introduce generic replacement resources
* Remove processing buildings
* Remove profession roles
* Make one obscure chain mandatory for survival

---

# 19. Scoring compatibility

The current scoring system includes:

* Days survived
* Nights survived
* Waves survived
* Crisis tier
* Legendary kills
* Other progression and defence metrics

This milestone does not include a complete scoring redesign.

Required changes:

* Do not increment wave-related values merely because a horde spawned.
* Ensure a crisis assault resolves and rewards only once.
* Avoid breaking existing database score writes.
* Add a personal-crisis completion value only if it can be done safely without broad schema work.
* Document obsolete fields for future revision.

Avoid database migrations unless required for correctness.

---

# 20. Headless testing requirements

Use the existing in-process headless harness.

Add deterministic coverage for:

1. Global day/night still advances.
2. Visibility still changes at night.
3. Dusk does not create a scheduled horde in personal-crisis mode.
4. Legacy mode can still run the old director.
5. The introductory encounter still progresses.
6. A goblin crisis enters `Signs`.
7. A goblin crisis enters `Pressure`.
8. The player receives a warning before a major assault.
9. Offline-active time does not advance.
10. An offline player cannot transition to `AssaultActive`.
11. An online player can transition to `AssaultActive`.
12. Repeated ticks do not duplicate the assault.
13. Crisis NPCs are attributed.
14. Disconnect follows the selected suspension or reset policy.
15. Reconnect sends current status.
16. Defeating required assault units resolves the crisis exactly once.
17. Existing gathering behaviour still works.
18. Existing crafting and refining behaviour still works.
19. Farming and fishing still work.
20. Villager work queues and assignments still work.
21. True Death cleanup remains safe.
22. Start-location recycling remains safe.

---

# 21. Headless metrics

Extend headless metrics where practical to include:

* Highest crisis phase reached
* Tick or online-active time at each phase
* Major assault launched
* Major assault defeated
* Crisis resolution count
* Duplicate assault count
* Automatic dusk hordes spawned
* Crisis units alive at disconnect
* Crisis state after reconnect

In personal-crisis mode, automatic dusk-horde count should remain zero.

---

# 22. Observability

Add concise structured logging for:

* Crisis creation
* Pressure changes
* Phase transitions
* Warning delivery
* Assault readiness
* Assault launch
* Assault ID
* Disconnect suspension or reset
* Reconnect resume
* Resolution

Include where applicable:

* Player ID
* Old phase
* New phase
* Pressure
* Game tick
* Online state
* Assault ID

Do not log every tick.

Avoid excessive production logging.

---

# 23. Code-quality expectations

* Use server-authoritative state.
* Keep phase transitions idempotent.
* Avoid giant multipurpose systems where focused systems are clearer.
* Do not spawn entities from status-reporting systems.
* Explicitly attribute crisis NPCs.
* Avoid unbounded database writes.
* Avoid per-tick client status packets.
* Account for Bevy deferred-command races.
* Use safe entity commands when an entity may be despawned.
* Do not assume an entity still exists later in the same update.
* Add focused unit and integration tests.
* Preserve current behaviour outside the milestone.

---

# 24. Implementation checkpoints

## Checkpoint 1: Director separation

Implement:

* Survival-director mode
* Personal-crisis mode as default
* Legacy mode
* Disable automatic dusk hordes in personal mode
* Disable old crisis ladder in personal mode
* Preserve environmental day/night
* Preserve introductory encounters
* Add tests

## Checkpoint 2: Crisis state foundation

Implement:

* Separate introduction and crisis state
* Goblin crisis types and phases
* Pressure
* Online-active timing
* Phase transitions
* Persistence compatibility
* Logging
* Tests

## Checkpoint 3: Assault lifecycle

Implement:

* Warning and grace period
* Online-only launch
* Crisis-unit attribution
* Assault resolution
* Disconnect policy
* Reconnect policy
* Duplicate prevention
* Cleanup tests

## Checkpoint 4: Network, UI, and validation

Implement:

* Crisis status packet
* Login/reconnect status delivery
* Minimal client display
* Headless metrics
* Multi-run validation
* Documentation updates

Each checkpoint should remain independently reviewable.

---

# 25. Out of scope

Do not implement during this milestone:

* Larger maps
* Approximately 25-player worlds
* Multiple-world orchestration
* Cross-world trade
* Cross-world travel
* World migration
* Seasonal resets
* Guilds
* PvP
* Distress beacons
* Regional goblin strongholds
* Regional crisis framework
* Offline shops
* Complex offline production
* Offline combat simulation
* Full settlement ruin system
* Successor settlement system
* Fire Dragon redesign
* Ranger redesign
* Mage redesign
* Full leaderboard redesign
* Resource-system simplification
* New crafting professions
* New resource tiers

These should remain documented follow-up work.

---

# 26. Validation commands

Determine exact commands from repository documentation and configuration.

At minimum, run applicable equivalents from `sp_server/`:

```bash
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets --all-features
cargo run --bin headless_runner -- --help
```

Run appropriate headless samples after gameplay changes.

Do not report a command as passing unless it actually ran successfully.

If a dependency or environment prevents a command from running, document:

* The exact command
* The exact error
* The affected validation
* Other checks that did complete

---

# 27. Definition of done

This milestone is complete when:

1. Global day/night remains functional.
2. Environmental darkness and visibility still change normally.
3. Dusk alone no longer spawns settlement hordes in personal-crisis mode.
4. Legacy mode preserves the old director for comparison.
5. The introductory encounter still functions.
6. The old automatic crisis ladder does not run in personal-crisis mode.
7. A goblin personal crisis progresses through explicit phases.
8. Progression uses player or settlement activity rather than only global day count.
9. The player receives clear warnings.
10. A major assault launches only while the owner is online.
11. Disconnect cannot cause unattended irreversible settlement destruction.
12. Reconnect preserves or safely restores crisis state.
13. Crisis enemies are explicitly attributed.
14. Duplicate assaults cannot spawn.
15. Resolution occurs exactly once.
16. Existing resource and production systems remain intact.
17. Existing economy-related tests continue passing.
18. New focused unit and headless tests cover the core rules.
19. The selected disconnect policy is documented.
20. Known limitations and deferred work are recorded.

---

# 28. Expected implementation report

At the end of the milestone, the implementation report should include:

1. Architecture inspected
2. Files changed
3. Behaviour before and after
4. Survival-director configuration
5. Crisis-state design
6. Disconnect and reconnect policy
7. Tests and commands run
8. Headless-run results
9. Known limitations
10. Deferred milestones
11. Any deviations caused by repository architecture

The implementation must not claim that the entire long-term Siege Perilous redesign has been completed.

This milestone implements only the persistent personal-crisis foundation.
