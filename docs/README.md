# Design documentation guide

This directory contains both living runtime contracts and dated evidence
records. The source code remains authoritative when a historical section
describes an older checkpoint.

## Current runtime contracts

| Document | Current status |
| --- | --- |
| [Core gameplay and settlement lifecycle](core_gameplay_polish_milestone.md) | Checkpoint 1 and the revised opening are implemented. The current worktree also contains the event-driven rescue gate, public standalone-Campfire light, desktop Tutorial & Help surface, and related client presentation. Checkpoint 2 remains reserved. |
| [Persistent personal crisis foundation](persistent_crisis_milestone.md) | The personal director and shared assault lifecycle are implemented. The default sequence is Goblin followed by Undead. |
| [Undead incursion](undead_crisis_milestone.md) | Both checkpoints are merged and functionally complete; the fixed wave is three Zombies, two Skeletons, and one specialized Necromancer. Final combat balance is not claimed. |
| [Safe Logout and Offline Protection](safe_logout_milestone.md) | Checkpoints 1–4 are implemented. Final milestone sign-off retains the two recorded broader validation blockers. The current worktree also exposes recipient-filtered protected-settlement snapshots and a desktop-only sanctuary ward. |
| [Goblin crisis balance milestone](goblin_crisis_balance_milestone.md) | Instrumentation and four balance checkpoints were executed, but the acceptance contract is not closed. Production retains two Wolf Riders and one Goblin Pillager. |

## Proposed or parked work

| Document | Current status |
| --- | --- |
| [Villager resource-site assignments](villager_resource_site_assignments_plan.md) | Design proposal only. The standing-site registry, packets, map badge layer, and candidate picker do not exist in the current source. |
| [Headless bot gear progression](../sp_server/docs/gear_progression_plan.md) | Parked proposal. The current bot builds the opening Burrow and survival structures but does not run the proposed crafting/smithing ladder. |

## Current implementation reference

[Headless harness architecture](../sp_server/docs/headless_harness_plan.md)
describes the implemented in-process Bevy harness, scripted bot, runner modes,
and output artifacts.

## Historical evidence

The following files are immutable-style experiment records. Their configuration,
sample counts, validation commands, and outcomes belong to the revisions named
inside each document:

- [Goblin baseline](goblin_crisis_balance_baseline.md)
- [Goblin Checkpoint 2](goblin_crisis_balance_checkpoint2.md)
- [Goblin Checkpoint 3](goblin_crisis_balance_checkpoint3.md)
- [Goblin Checkpoint 4](goblin_crisis_balance_checkpoint4.md)

Later source changes do not retroactively change those measurements.

## Source-alignment facts

- `sp_server/src/map.rs` defines `WIDTH = 60` and `HEIGHT = 50`; the active
  `sp_server/map/test3.tmx` has the same dimensions.
- `sp_server/templates/player_start.yaml` defines five start locations.
- `SurvivalDirectorMode::PersonalCrisis` is the production and headless
  default. Legacy automatic threat systems remain compiled behind
  `SurvivalDirectorMode::Legacy`.
- Personal crisis, introduction, start-assignment, and Safe Logout coordination
  include runtime-only state. The documents do not claim complete durable
  restart restoration for those graphs.
- Desktop and mobile share protocol and core Phaser code but retain separate UI
  trees. Desktop-only features are identified explicitly in the milestone
  documents.

## Documentation-sync validation

The 2026-07-27 source-alignment pass ran checks without changing source code.

- From `sp_frontend/sp_ts/`, `./check-imports.sh` passed.
- From `sp_frontend/sp_ts/`, `npx tsc --noEmit --skipLibCheck` passed.
- A project-configured emit plus Node execution passed all eight new pure
  presentation/policy scripts for fire animation, protected settlements,
  visibility sources, sanctuary geometry, Campfire actions, tutorial hints,
  tutorial notice routing, and tutorial preferences.
- `npx webpack --mode production --stats=errors-warnings` built both bundles.
  Desktop was 3.38 MiB and mobile was 2.43 MiB; each retained the three existing
  Webpack size/code-splitting warnings.
- From `sp_server/`, `cargo fmt --all -- --check` and `cargo check` passed; Cargo
  reported the existing 67 warning-only diagnostics.
- From `sp_server/`, `cargo test --no-fail-fast` was not fully green. The
  library target passed 579 tests and failed five:
  `campfire_light_bubble_tracks_radius_one_across_movement_and_reconnect`,
  `active_bot_completes_revised_opening_through_production_events`, and the
  Warrior, Ranger, and Mage
  `checkpoint4_*_bot_acquires_and_damages_through_production_events` tests.
  The failures came from randomized start geometry or fixtures expecting more
  setup time than the current five-second post-search grace. The three runner
  test targets passed 9/9, 17/17, and 5/5; day-system integration passed 6/6;
  the one documentation test remained ignored.
- A focused rerun of the Campfire movement/reconnect test passed, confirming
  that its full-suite failure is start-geometry-sensitive. A focused rerun of
  the production-opening bot test failed again on its assertion that deferred
  fixture deadlines should leave every opening-enemy spawn flag clear.

This record does not replace the historical validation sections in the
milestone files. It states the current checkout's observed check status, and no
source fix is included in this documentation-only update. No browser-rendered
QA or artifact-generating headless runner was executed.
