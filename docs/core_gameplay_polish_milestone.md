# Milestone 5 — Core Gameplay and Settlement Lifecycle Polish

## Status

Checkpoint 1, the revised new-player opening, and the source follow-ups recorded
at the top of this document are implemented in the current checkout.
Checkpoint 2 remains reserved. The later “baseline before” and “original
Checkpoint 1” sections are historical records; where they differ, the current
contract in the sections above them takes precedence.

## Revised new-player opening follow-up

The opening-session follow-up removes the completed starter Burrow and moves
all survival supplies into the run's existing Shipwreck. Fresh heroes keep
their class, statistics, abilities, recipes, and five basic plans; their
inventory contains an unequipped Sharpened Stick plus equipped Tattered Shirt
and Tattered Pants. The lit starter Campfire remains and contains 20 ordinary
Firewood.

A lit standalone Campfire provides a one-hex visibility bubble centered on the
Campfire only while a living player hero is standing on it or an adjacent hex.
The nearby hero's player receives that bubble regardless of who owns the
Campfire. Multiple bubbles are combined as a simple union and never increase
one another's range. Any living hero may light a completed standalone Campfire
from within that same one-hex distance by using the hero's own Ignition Tool;
the Campfire must already contain fuel, and lighting it does not grant access
to another player's fuel inventory or other structure controls.
Existing fire-capable shelters retain their previous owner-only, exact-tile
illumination and controls; they do not become public Campfires under this rule.

The server represents each active viewer/Campfire pair in
`CampfireVisibilityState` and sends the Campfire as a perception observer with
its real owner and a one-hex range. The client accepts a foreign object as a
vision source only when the current authoritative observer snapshot designates
it as one. It preserves that observer range across incremental visible-object
updates and clears old observer authority at the next initialization boundary,
preventing remembered foreign structures from becoming permanent light
sources.

Both client shells retain the shared animated overlay for objects whose state
is `burning`. Desktop additionally draws a smaller, translucent, additive
flame over an object whose authoritative presentation is a lit standalone
Campfire. Animation start frames are stable per object so nearby fires do not
move in lockstep. The desktop target strip exposes **Light Campfire** for a
completed unlit standalone Campfire; the server still validates living-hero
identity, idle state, range, fuel, an Ignition Tool, duplicate activation, and
Safe Logout protection. Mobile has the shared light/perception behavior but no
new lighting affordance or lit-Campfire overlay in this follow-up.

The owner Shipwreck manifest is exact: Crude Hatchet x1, Crude Torch x3,
Bedroll x1, Waterskin (Filled) x3, Salted Meat Strip x3,
Honeybell Berries x3, Health Potion x1, Flint Shard x1, Cragroot Maple Resin
x1, Cragroot Maple Stick x1, Honeybell Cloth x5, Springbranch Maple Log x5, Cragroot Maple Timber
x1, Valleyrun Copper Ingot x3, Gold Coins x10, and Fishing Rod x1. Warrior adds Copper Helm x1,
Ranger adds Training Bow x1, and Mage adds Mana x5. The previous per-instance
Copper Helm Defense 3 and Training Bow attributes remain instance-specific.
The starter Health Potion now intentionally uses its canonical template value
of 50 HP. No Yurt Deed or Mine Deed is present in the hero,
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

The starter Shipwreck's Crude Hatchet has `Damage 1` and `Speed 5`, matching
the Sharpened Stick's combat profile, plus `Logging 1` so every class can use
the ordinary lumberjacking path after the opening. The Sharpened Stick keeps
its normal global template attributes. Resource nodes, yields, recipes, and
gathering rates are unchanged. Timber continues to be a legal Log substitute
in the existing construction engine, but the guided path uses the five
salvaged Logs and leaves the Timber as valuable storage salvage.

The first successful owner Shipwreck investigation is the authoritative
survivor-discovery beat. It records the search facts, reveals that someone is
trapped in the wreck, and arms the run's single randomized wave of one to three
Giant Rats. Repeated investigation cannot replay the discovery, create another
opening wave, or schedule another rescue. The investigation counts only if its
timed action completes while the living hero is still investigating, remains
adjacent to the Shipwreck, and is outside the combat lock. Movement, death, or
combat contact—including damage that does not otherwise establish a combat
lock—cancels that attempt without recording discovery or arming the wave; the
player can investigate again once it is safe. The
successful investigation replaces the old 90-second hostile deadline with a
one-second post-search grace. With the encounter system's one-second polling
cadence, the rats appear within roughly one to two seconds. They spawn on the Shipwreck tile and
immediately fan out to distinct, spatially separated passable adjacent tiles.
If fewer safe adjacent tiles are available, only the rats with reserved paths
move and the blocked remainder stay on the wreck. These scripted opening rats
use a dedicated 6 HP, 0 defense combat profile so ordinary precise Sharpened
Stick attacks from every novice class defeat one in two to three landed hits.
Their Giant Rat base damage is doubled from 2 to 4 to keep the shorter fight
dangerous. Ambient Giant Rats retain their normal template statistics.

The rat wave deliberately interrupts settlement work. While any opening rat
is active, `win_first_fight` temporarily takes priority over an unfinished
`build_burrow` recommendation. Normal server combat-lock behavior cancels a
hero's peaceful Building action after combat contact, but the Burrow
foundation, staged materials, and accumulated construction work remain. Once
the threat is clear, guidance returns to the unfinished Burrow instead of
restarting either task.

The rescued villager is scheduled exactly once only after both independent
facts are true: the entire randomized opening wave has been defeated and the
player owns a completed normal Burrow. Either fact may be completed first. No
elapsed-time-only path can release the villager; the former fixed 1,100-tick
distress call and 1,110-tick rescue-eligibility gate are removed. The rescued
villager, merchant, and introductory Necromancer retain their authored
narrative anchors. After every rat is defeated and the existing later phase
gates are reached, the Wild Boar/Giant Crab follow-up and then the Spider each
choose a randomized valid, passable, reachable, unoccupied tile two to four
tiles from the run's assigned hero start. If no safe candidate exists, spawning
waits and retries instead of overlapping an occupied or invalid tile.
At-most-once history, danger unlock, and Offline Protection remain
authoritative, preserving the search, salvage, complete-the-fight-and-build,
then rescue flow without a parallel tutorial state machine.

The exact implementation surface for this follow-up is:

* `sp_server/src/player_setup.rs` — starter entities and exact inventories;
* `sp_server/src/player.rs` — narrow owner authorization for Shipwreck search,
  inventory display, split, transfer, and legacy buy input;
* `sp_server/src/game.rs` and `game_tests.rs` — manual salvage, post-search
  grace, Burrow-aware intro/objective flow, and focused regressions;
* `sp_server/src/headless.rs`, `headless_bot.rs`, and
  `bin/headless_runner.rs` — production-path opening, reconnect, and reporting
  fixtures updated to recover Shipwreck salvage and build the normal Burrow.
* `sp_frontend/sp_ts/src/sp/core/` — observer-source tracking, incremental
  visibility reconciliation, and shared fire presentation;
* `sp_frontend/sp_ts/src/sp/desktop/` — the public Campfire action and
  desktop-only lit-Campfire animation.

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
  salvaging the five Logs and normal Burrow construction complete.

The event-driven rescue sequencing described above supersedes the fixed
distress/rescue timing exercised by this earlier validation record. The current
source adds focused regressions for interrupted and retryable investigation,
same-update combat and lethal damage, the one-second post-search grace,
Burrow-first and rats-first rescue ordering, reconnect/idempotency, partial
Burrow preservation, public Campfire ignition, overlapping light bubbles,
burnout, shelter compatibility, observer-source policy, and desktop fire
presentation.

During the 2026-07-27 documentation sync, formatting and compilation passed but
the full library test target exposed five current-worktree failures. One
Campfire visibility test could not find its randomized second passable step;
the production opening bot observed the rat wave earlier than its fixture
expected; and the three class-combat fixtures did not finish setup within the
post-search grace. These failures are not rewritten as passing
by the older validation record above, and this documentation-only change does
not alter their source fixtures. A focused Campfire test rerun passed, showing
that failure is sensitive to randomized start geometry; a focused opening-bot
rerun reproduced the unexpected spawn-state assertion.

## Desktop Tutorial and Help follow-up

The desktop Survival Thread now serves as the Tutorial and Help surface without
introducing a second objective model. The existing server-authoritative
`objective_state` packet remains the source for the current task, lesson,
action hint, blocker, and progress. Crisis and Safe Logout continue to use the
same outer panel but remain independent of whether tutorial guidance is
enabled.

Tutorial guidance is enabled by default. A keyboard-accessible desktop toggle
stores an explicit, versioned preference per player in browser `localStorage`.
Disabling it hides objective guidance and cancels tutorial reminders without
hiding Crisis or Safe Logout controls. Objective snapshots continue updating
while disabled so re-enabling immediately shows the authoritative current
step. The preference survives reconnect, True Death, and a new run; it is a
desktop-browser preference rather than a new database field.

When an enabled tutorial objective remains unchanged for 60 seconds of
eligible foreground play, the desktop panel displays a dedicated hint using
the objective's existing action hint and optional blocker. An unchanged task
can repeat at most once every 120 seconds. Progress, objective changes,
reconnect, run reset, or re-enabling restart the grace period. Hidden tabs,
stale objective delivery, hero death, active combat, urgent personal crisis,
and pending or protected Safe Logout defer reminders. Hints render inside the
Tutorial and Help presentation rather than entering the normal notification
stack.

The protocol's generic `Notice` packet has no presentation category, and its
scheduled opening notices are shared by desktop and mobile. To keep this
checkpoint desktop-only, the desktop shell narrowly consumes the two exact
opening instructional notices and removes its own duplicate Shipwreck nudge.
Other alerts and completion feedback remain transient notices. Server
scheduling and all mobile source and behavior are unchanged; a typed tutorial
notice category remains a possible later protocol cleanup when the mobile
design is revisited.

The implementation surface is limited to the desktop UI shell, desktop
Survival Thread panel, desktop-only preference/hint/routing helpers and their
focused tests, plus this record. It adds no server system, network packet,
database write, gameplay rule, or mobile UI change.

The required validation for this follow-up is the supported
`npx tsc --noEmit --skipLibCheck` check, the cross-fork import guard, focused
tutorial policy/preference/routing and ObjectivesPanel component checks, and a
finite production Webpack build. Browser QA at 1440×900 and the compact
1024×900 desktop breakpoint should cover the On/Off control, expanded and
collapsed hint layouts, Open guide and Dismiss actions, and a clean console.
That browser check is a manual acceptance item, not implied by source-level
tests.

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
starter inventory, Shipwreck contents, rescue eligibility, opening-enemy
composition and schedule, and hostile spawn placement. The at-most-once
encounter-history and objective fixes remain current.

* `player_setup::new` assigns one of the existing start locations and creates
  the hero, completed Burrow, lit Campfire, salvage-filled Shipwreck, two human
  corpses, hidden introductory Necromancer and Mausoleum, offshore merchant,
  and fresh runtime introduction state. At that checkpoint it preallocated two
  opening-enemy IDs. The Shipwreck includes ten existing Logs and ten Hides;
  the existing Stockade (fifteen Logs, 30 work) and Crafting Tent (five Logs,
  five Hides, 100 work) rules remain the early construction path.
* The historical `InitialEncounterState` schedule used Cave Bats at 900 and
  1,200 ticks, a survivor call at 1,100 ticks and rescue eligibility at 1,110
  ticks, the Wild Boar/Giant Crab follow-up gate at 2,600 ticks, and the Spider
  gate at 3,600 ticks. The current opening removes the fixed survivor call and
  rescue deadline. It uses the first successful Shipwreck investigation for
  discovery, one delayed one-to-three Giant Rat wave, the completed-Burrow plus
  all-rats-defeated rescue gate, and randomized follow-up positions described
  above. `PlayerIntroState` owns broad introduction/danger facts;
  `IntroEncounterState` owns follow-up phase facts.
* The first successful Shipwreck investigation records `scavenge_shipwreck`
  and the existing `explore_poi` fact, reveals the survivor, and arms the one
  opening wave. The rescued villager is queued only after the entire wave is
  defeated and a normal Burrow is complete. That villager is created already
  owned by the player with zero base damage and a Crude Torch, shares the
  existing Watchtower plan, and schedules the merchant after 1,800 ticks and
  introductory Necromancer after 3,000 ticks.
* `objectives_system` observes the hero inventory, live structures, villagers,
  and persistent villager orders every 50 ticks. Completed Prospect, Gather,
  and Refine events record the post-rescue forest-production history. The
  system emits the existing `objectives` and `objective_state` packets. The
  desktop Survival Thread selects the one `active` row while continuing to
  display the remaining rows.
* True Death removes only that player's introduction resources, objectives,
  run objects, and start assignment. A successful fresh run
  initializes new state. Ordinary reconnect retains state; Offline Protection
  freezes the existing introduction deadlines and systems.
* `PlayerIntroState.danger_unlocked` remains the personal-crisis gate at 4,800
  run ticks. Goblin and Undead pressure contributors, phase timing, launch,
  composition, history, Safe Logout, and disconnect rules are separate and are
  not changed here.

## Repository conflicts and four selected fixes

1. **Opening-enemy lifecycle history.** The original first enemy had a broad
   `shipwreck_chain_started` guard, while the second and first-fight objective
   inferred history from current corpse/entity existence. Corpse removal could
   make the second enemy eligible to spawn again and erase evidence needed by
   the next phase. `InitialEncounterEntry` now owns explicit spawned and
   defeated history for every wave member. The randomized Giant Rat revision
   sizes those lifecycle vectors to the one-to-three-enemy wave, preserving the
   same at-most-once guarantee.
2. **Authoritative early recommendation.** The packet currently recommends
   Campfire before opening combat even though a fresh run already owns a lit
   Campfire, and its static copy cannot describe a waiting encounter. The
   existing objective facts drive the opening recommendation: equip the carried
   Sharpened Stick, inspect the Shipwreck, recover supplies and build the Burrow,
   temporarily prioritize the opening fight whenever its rats are active, then return to unfinished
   construction. Only after both the completed Burrow and defeated wave does
   guidance advance to meeting the survivor, prospecting a forest, assigning
   the settler to persistent Logging, and having the hero hunt and butcher a
   carcass for Hide. The Shelter Tent upgrade follows that material lesson,
   before completing a basic settlement and choosing an expansion. Guidance
   does not automate combat, construction, rescue, gathering, or assignment.
3. **First-villager purpose.** The rescued villager is already player-owned, so
   `recruit_villager` completes as soon as the entity appears and gives no
   actionable work step. The existing objective resource will record a
   one-time `prospect_forest` fact only after the rescued survivor exists and a
   forest Prospect completes. The internal `assign_first_villager` fact now
   requires an actual persistent `Order::Gather { Log }`; a generic structure
   assignment no longer completes it. Guidance then hands hunting and carcass
   refining back to the hero and never assigns, gathers, refines, or transfers
   anything automatically.
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
  remain as legacy structure. The lifecycle history uses dynamically sized
  spawned and defeated vectors plus one narrow `phase1_defeated` fact so corpse
  cleanup cannot strand the already-existing Spider follow-up.
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
* Exactly three bounded headless smokes passed 3/3. The historical opening
  smoke used the production Shipwreck investigation, recorded both at-most-once
  Cave Bat deaths, removed corpses, and reached the boar/crab and Spider
  follow-ups. This validation record predates the current Giant Rat wave and
  randomized follow-up placement.
  The settlement smoke used the lit Campfire, rescued unarmed villager, real
  assignment, ten fixture Shipwreck Logs, and normal Stockade work to reach
  three completed structures. The lifecycle smoke preserved encounter and
  recommendation state across ordinary reconnect, then verified True Death
  cleanup and fresh-run reset.

No balance matrix, headless batch, candidate comparison, output artifact,
report runner, or persistent development server was run.

> Checkpoint 1 improves opening clarity and settlement growth using existing systems. It does not perform crisis balance work or redesign the economy.
