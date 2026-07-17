# Milestone 5 — Core Gameplay and Settlement Lifecycle Polish

## Revised new-player opening follow-up

The opening-session follow-up removes the completed starter Burrow and moves
all survival supplies into the run's existing Shipwreck. Fresh heroes keep
their class, statistics, abilities, recipes, and four basic plans, but their
inventory contains only equipped Tattered Shirt and Tattered Pants. The lit
starter Campfire remains and contains five ordinary Firewood.

The owner Shipwreck manifest is exact: Sharpened Stick x1, Crude Torch x1,
Bedroll x1, Waterskin (Filled) x3, Salted Meat Strip x3, Honeybell Berries x3,
Health Potion x1, Flint Shard x1, Cragroot Maple Resin x1, Cragroot Maple Stick
x1, Springbranch Maple Log x2, Cragroot Maple Timber x1, Valleyrun Copper
Ingot x3, Gold Coins x10, and Fishing Rod x1. Warrior adds Copper Helm x1,
Ranger adds Training Bow x1, and Mage adds Mana x5. The previous per-instance
Copper Helm Defense 3, Training Bow attributes, and starter Health Potion
Healing 10 are preserved instead of silently replacing them with current
template defaults. No Yurt Deed or Mine Deed is present in the hero,
Shipwreck, or Campfire; the existing later POI rewards remain unchanged.
The neutral offshore merchant also keeps its established trade stock. Neither
the merchant nor the later POIs is free player-owned starter storage.

The existing setup architecture has four important consequences:

* `player_setup::new` is the sole fresh-run initializer. Ordinary login only
  resynchronizes the existing ECS world, and duplicate `NewPlayer` input is
  rejected while a hero or start assignment exists.
* The Shipwreck is a neutral `poi`, not a `ClassStructure` or `Storage`. Its
  association with one run is already recorded in `RunSpawnedObjs`; that
  registry is the selected authorization source. The wreck remains neutral so
  generic owned-structure mutation and settlement-wealth systems do not treat
  it as permanent storage. Like the rest of the current run graph, this
  association is runtime ECS state: it survives an ordinary disconnect and
  reconnect, but this follow-up does not add process-restart persistence.
* POIs are already non-attackable and Shipwreck is not one of the temporary
  loot-cache templates, so combat and decay need no redesign. Manual transfer,
  split, inventory/item display, investigation, and the legacy buy path need
  narrow checks because the neutral-POI exceptions otherwise permit another
  player to access or mutate a wreck. The owner uses the normal transfer path;
  Shipwreck is explicitly not accepted as a merchant.
* Shipwreck investigation currently grants a separate random item directly to
  the hero. That conflicts with the exact wreck manifest and manual retrieval,
  so the Shipwreck-specific automatic reward is removed while other POI
  outcomes remain unchanged.

The exact requested manifest exposes one repository conflict: normal hero
logging requires an equipped main-hand item with `Logging`, but none of the
listed salvage or current class items has that attribute. The current Warrior
does not start with an axe, and the Copper Training Axe requires a Crafting
Tent whose own inputs cannot be obtained from this start. The selected narrow
correction gives only the starter Shipwreck's Sharpened Stick instance
`Logging 1`, while retaining its existing combat and Hunting attributes. The
global item template, resource nodes, yields, recipes, and gathering rates are
unchanged. Timber continues to be a legal Log substitute in the existing
construction engine; the guided path uses the two salvaged Logs plus three
newly gathered Logs and leaves the Timber as valuable storage salvage.

The existing 90-second opening-hostile delay is ample only when the player
searches immediately. The revised flow also requires owner Shipwreck
inspection before either scheduled opening enemy can spawn. A late first
inspection pushes the existing two spawn deadlines forward by a bounded
post-search equip grace; early inspection keeps the current schedule. The
existing enemy composition, at-most-once history, boar/crab and Spider
follow-ups, danger unlock, and Offline Protection remain authoritative. The
rescued villager additionally waits for a completed normal Burrow, preserving
the requested search, gather, build, then rescue order without a parallel
tutorial state machine.

The exact implementation surface for this follow-up is:

* `sp_server/src/player_setup.rs` — starter entities and exact inventories;
* `sp_server/src/player.rs` — narrow owner authorization for Shipwreck search,
  inventory display, split, transfer, and legacy buy input;
* `sp_server/src/game.rs` and `game_tests.rs` — manual salvage, post-search
  grace, Burrow-aware intro/objective flow, and focused regressions;
* `sp_server/src/headless.rs`, `headless_bot.rs`, and
  `bin/headless_runner.rs` — production-path opening, reconnect, and reporting
  fixtures that currently assume a free Burrow and hero-carried equipment.

No map, resource node, global item template, recipe, production, crisis,
weather, persistence, database, or deployment redesign is part of this
follow-up.

## Revised-opening validation record

* `cargo fmt --all -- --check`, `cargo check`, and `git diff --check` passed.
  `cargo check` retained the repository's existing 67 warning-only diagnostics.
* Focused revised-opening coverage passed: five headless setup, authority,
  timing, reconnect, and normal-construction tests; the production opening bot;
  the introductory follow-up; the Burrow/rescue objective tests; the starter
  Logging-item test; and the Shipwreck access and non-attackability tests. Two
  attempted unqualified `--exact` invocations matched zero module-qualified
  tests and are not counted; their substring-filter replacements each ran one
  test and passed.
* The final `cargo test --no-fail-fast` passed all targets: 563 library tests,
  9 Goblin Checkpoint 4 runner tests, 17 headless-runner tests, 5 preparation
  runner tests, and 6 day-system integration tests (600 passed total, zero
  failed). One documentation test remained intentionally ignored. Earlier full
  runs exposed random-start assumptions in the legacy settlement smoke and the
  production opening bot; both fixtures now select the existing deterministic
  production start, and the final full run is green.
* `cargo clippy --all-targets --all-features` passed. It retained the existing
  lint backlog: 1,341 library warnings and 1,364 library-test warnings, plus the
  small existing binary warning sets.
* `cargo run --bin headless_runner -- 1 6000 standard` exited successfully and
  wrote its ignored CSV/JSON artifacts. The randomized run reached True Death
  from a Wolf at tick 5,783 after two in-game days; it had three enemy kills and
  no panic, automatic dusk wave, assault duplication, or crisis/safe-logout
  invariant failure. This is a bounded smoke result, not a claimed simulation
  victory; the focused production-opening test is the deterministic proof that
  salvage, real Log gathering, and normal Burrow construction complete.

## Scope and checkpoint plan

Milestone 5 has two checkpoints:

1. **Checkpoint 1 — Opening session and settlement growth:** connect the
   existing shipwreck, opening combat, campfire, first villager, early
   structures, and Goblin unlock with reliable runtime history and one clear
   recommendation.
2. **Checkpoint 2 — Crisis aftermath, recovery, and full-lifecycle
   validation:** reserved. It is not implemented by Checkpoint 1.

Checkpoint 1 starts from `main` commit `b2115c8`, which contains the completed
Milestone 4 Undead crisis.

## Checkpoint 1 baseline before the revised opening

This section records the opening that the original Checkpoint 1 inherited and
validated. The revised-opening follow-up above supersedes its starting Burrow,
starter inventory, Shipwreck contents, and rescue eligibility; the encounter
history and objective fixes remain current.

* `player_setup::new` assigns one of the existing start locations and creates
  the hero, completed Burrow, lit Campfire, salvage-filled Shipwreck, two human
  corpses, hidden introductory Necromancer and Mausoleum, offshore merchant,
  and fresh runtime introduction state. It preallocates two opening-enemy IDs.
  The Shipwreck includes ten existing Logs and ten Hides; the existing
  Stockade (three Logs, 30 work) and Crafting Tent (five Logs, five Hides,
  100 work) rules remain the early construction path.
* `InitialEncounterState` owns the detailed per-run schedule: opening enemies
  at 900 and 1,200 ticks (both currently Cave Bats), a survivor call at 1,100
  ticks and rescue eligibility at 1,110 ticks, the existing Wild Boar/Giant
  Crab follow-up gate at 2,600 ticks, and Spider gate at 3,600 ticks.
  `PlayerIntroState` owns broad introduction/danger facts;
  `IntroEncounterState` owns follow-up phase facts.
* Investigating the Shipwreck records `scavenge_shipwreck` and the existing
  `explore_poi` fact. Once the survivor readiness time is reached, the
  inspection schedules the rescued villager. That villager is created already
  owned by the player with zero base damage and a Crude Torch, shares the
  existing Watchtower plan, and schedules the merchant after 1,800 ticks and
  introductory Necromancer after 3,000 ticks.
* `objectives_system` observes live structures and villagers every 50 ticks and
  emits the existing `objectives` and `objective_state` packets. The desktop
  Survival Thread selects the one `active` row while continuing to display the
  remaining rows.
* True Death removes only that player's three introduction resources,
  objectives, run objects, and start assignment. A successful fresh run
  initializes new state. Ordinary reconnect retains state; Offline Protection
  freezes the existing introduction deadlines and systems.
* `PlayerIntroState.danger_unlocked` remains the personal-crisis gate at 4,800
  run ticks. Goblin and Undead pressure contributors, phase timing, launch,
  composition, history, Safe Logout, and disconnect rules are separate and are
  not changed here.

## Repository conflicts and four selected fixes

1. **Opening-enemy lifecycle history.** The first enemy has a broad
   `shipwreck_chain_started` guard, while the second and first-fight objective
   infer history from current corpse/entity existence. Corpse removal can make
   the second enemy eligible to spawn again and can erase evidence needed by
   the next phase. `InitialEncounterEntry` will own explicit two-entry spawned
   and defeated flags. The scheduled composition remains unchanged.
2. **Authoritative early recommendation.** The packet currently recommends
   Campfire before opening combat even though a fresh run already owns a lit
   Campfire, and its static copy cannot describe a waiting encounter. The
   existing objective facts will drive one ordered recommendation: inspect the
   Shipwreck, defeat the opening threat, establish/use the existing Campfire,
   meet the survivor, put the settler to work, complete a basic settlement, and
   choose an expansion. No action is made mandatory by this ordering.
3. **First-villager purpose.** The rescued villager is already player-owned, so
   `recruit_villager` completes as soon as the entity appears and gives no
   actionable work step. The existing objective resource will record a
   one-time `assign_first_villager` fact only after a real assignment exists.
   Guidance will point to the existing structure Assign action, describe an
   unarmed villager as a worker rather than a defender, and never assign or
   transfer anything automatically.
4. **Completed-structure dead end.** `objectives_system` currently counts every
   `ClassStructure`, including Founded/Building/Stalled foundations, toward
   `build_3_structures`. That can remove settlement guidance before three
   structures function. The objective will count only structures accepted by
   the existing `Structure::is_built` rule. Normal plans, requirements, costs,
   transfers, and construction time remain authoritative.

The one presentation cleanup stays within the existing introductory copy,
notices, and desktop/mobile Survival Thread. The recommended card labels why
the goal matters, the concrete next action, and an optional server-derived
blocker. Other objectives remain visible. No new tutorial screen or objective
framework is introduced.

## Original Checkpoint 1 implementation surface

The exact affected files are:

* `docs/core_gameplay_polish_milestone.md`
* `sp_server/src/game.rs`, `game_tests.rs`, `headless.rs`, `network.rs`, and
  `player_setup.rs`
* `sp_frontend/sp_ts/src/sp/core/network.ts`
* `sp_frontend/sp_ts/src/sp/desktop/ui.tsx`, `ui/introPanel.tsx`,
  `ui/objectivesPanel.tsx`, and `ui/objectivesPanel.guidance.test.tsx`
* `sp_frontend/sp_ts/src/sp/mobile/ui/introPanel.tsx` and
  `ui/objectivesPanel.tsx`

Resource, recipe, crafting, farming, refining, villager-AI, crisis,
Safe Logout, map, and deployment files are unchanged.

## Deferred issues

* Restart persistence for introduction/objective state remains absent with the
  rest of the runtime-only run graph.
* The existing `rat_ids` name and split between three introduction resources
  remain as legacy structure. The lifecycle fix adds only the two requested
  opening arrays and one narrow `phase1_defeated` fact so corpse cleanup cannot
  strand the already-existing Spider follow-up.
* `choose_expansion` retains its existing foundation-based completion fact.
  Tightening that second progression fact was intentionally not folded into
  the one selected completed-structure dead-end fix.
* Merchant, introductory Necromancer, resource chains, optional plans, and
  villager AI may have later polish opportunities, but they are not additional
  Checkpoint 1 fixes.
* Crisis aftermath, repair/recovery guidance, and full-run lifecycle validation
  are reserved for Checkpoint 2.

## Original Checkpoint 1 validation record

* `cargo fmt --check` and `cargo check` passed. `cargo check` retained the
  repository's warning-only output (67 warnings).
* Focused Rust filters passed: introductory encounter 1/1; Checkpoint 1 13/13;
  personal crisis 7/7; Undead crisis 16/16; Safe Logout 65/65; legacy 3/3.
* The first full `cargo test` attempt passed 540 library tests and exposed one
  unrelated random-map fixture miss in
  `occupied_disengage_destination_skips_ability_and_preserves_ranger_actions`.
  That test passed 1/1 in isolation, and the unchanged full command then passed
  all targets: 578 tests passed, zero failed, and one doc test was ignored.
* `cargo clippy --all-targets --all-features` passed with warning-only legacy
  lint debt (1,341 library warnings and 1,364 library-test warnings, plus small
  binary warning sets).
* Frontend type-checking passed; the focused guidance, crisis, and Safe Logout
  scripts passed 3/3; production Webpack passed for desktop and mobile with
  their existing three performance warnings each.
* Exactly three bounded headless smokes passed 3/3. The opening smoke used the
  production Shipwreck investigation, recorded both at-most-once Cave Bat
  deaths, removed corpses, and reached the boar/crab and Spider follow-ups.
  The settlement smoke used the lit Campfire, rescued unarmed villager, real
  assignment, three existing Shipwreck Logs, and normal Stockade work to reach
  three completed structures. The lifecycle smoke preserved encounter and
  recommendation state across ordinary reconnect, then verified True Death
  cleanup and fresh-run reset.

No balance matrix, headless batch, candidate comparison, output artifact,
report runner, or persistent development server was run.

> Checkpoint 1 improves opening clarity and settlement growth using existing systems. It does not perform crisis balance work or redesign the economy.
