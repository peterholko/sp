# Siege Perilous Repository Instructions

## Project identity

Siege Perilous is a persistent shared-world, real-time settlement survival game.

Players control heroes, gather and process resources, recruit villagers, build settlements, survive crises, and interact with other players when their sessions overlap.

The current prototype uses the existing 50×50 map and approximately five settlement locations.

## Product invariants

Unless a task explicitly says otherwise:

* Preserve the existing 50×50 map.
* Preserve the persistent shared-world architecture.
* Preserve the global environmental day/night cycle.
* Preserve weather, visibility, lighting, and night-travel effects.
* Preserve the existing resource and production systems.
* Preserve harvesting, mining, farming, fishing, hunting, refining, smelting, tanning, food processing, crafting, recipes, inventories, work queues, villager professions, and trade.
* Do not collapse existing resources into generic abstractions.
* Do not add 25-player worlds yet.
* Do not implement cross-world interaction yet.
* Do not add new resources, professions, currencies, or crafting tiers unless explicitly requested.
* Personal irreversible danger must not progress while the owning player is offline.
* Personal crisis assaults must not begin unless the owner is online.
* Solo players must remain capable of completing personal content.
* Multiplayer assistance should enhance play but must not be required.

## Scope discipline

* Make focused, reviewable changes.
* Do not attempt to implement an entire long-term design plan in one patch.
* Do not perform broad unrelated refactors.
* Do not silently delete existing gameplay systems.
* Prefer retaining legacy systems behind an explicit configuration mode when replacing them.
* Do not modify deployment, production credentials, or infrastructure unless explicitly requested.
* Do not run destructive database scripts without explicit approval.
* Do not add production dependencies without explaining why they are necessary.

## Repository architecture

Important server areas include:

* `sp_server/src/game.rs` — gameplay systems, crisis logic, sanctuary logic, scoring, system scheduling
* `sp_server/src/world.rs` — environmental time, day/night, weather, visibility
* `sp_server/src/network.rs` — incoming and outgoing protocol packets
* `sp_server/src/player.rs` — player events and player behaviour
* `sp_server/src/player_setup.rs` — player and run setup
* `sp_server/src/event.rs` — game and map events
* `sp_server/src/headless.rs` — in-process headless simulation harness
* `sp_server/src/headless_bot.rs` — deterministic test bot
* `sp_server/src/bin/headless_runner.rs` — multi-run balance and regression runner
* `sp_server/src/resource.rs` — resource system
* `sp_server/src/recipe.rs` — recipes and crafting
* `sp_server/src/farm.rs` — farming
* `sp_server/src/structure.rs` — structures and work queues
* `sp_server/src/trade.rs` — trade
* `sp_server/src/villager_util.rs` and villager AI modules — villager behaviour

Inspect the actual code before assuming these responsibilities are complete or exclusive.

## Bevy and ECS rules

* This project uses Bevy 0.17.
* Account for deferred-command and entity-despawn races.
* Prefer safe command patterns such as `try_insert` when an entity may disappear before deferred commands are applied.
* Do not assume an entity still exists merely because it existed earlier in the same update.
* Keep gameplay state server-authoritative.
* Avoid spawning entities from read-only status or reporting systems.
* Make state transitions idempotent.
* Explicitly attribute spawned crisis enemies to their owning player and assault.
* Avoid per-tick database writes and per-tick log spam.

## Implementation workflow

For substantial changes:

1. Inspect relevant code and existing documentation.
2. Summarize the current architecture.
3. Identify affected files and risks.
4. Write or update the implementation plan.
5. Implement the smallest coherent checkpoint.
6. Format and compile.
7. Run focused tests.
8. Run broader tests where practical.
9. Run the headless harness for gameplay changes.
10. Review the final diff for unrelated modifications.

Do not stop after planning when the task explicitly requests implementation.

## Validation

For Rust server changes, run applicable commands from `sp_server/`:

```bash
cargo fmt --check
cargo check
cargo test
```

Run Clippy when practical:

```bash
cargo clippy --all-targets --all-features
```

For gameplay, crisis, balance, resource, or simulation changes, inspect and run the headless runner using its supported arguments.

Do not claim that a command passed unless it was actually executed successfully.

If a dependency or environment limitation prevents a check, report:

* The exact command
* The exact error
* Which other checks were completed

## Testing expectations

Changes should include focused regression tests when practical.

For gameplay-system changes, test:

* Normal operation
* Duplicate-tick/idempotency behaviour
* Login and reconnect behaviour
* Disconnect behaviour
* Entity cleanup
* Interaction with True Death and start-location recycling
* Compatibility with the introductory encounter
* Compatibility with harvesting, production, crafting, and villager work

Preserve and extend the existing headless test infrastructure rather than creating a separate simulation framework.

## Current design initiative

For work related to the personal-crisis redesign, read and follow:

```text
docs/persistent_crisis_milestone.md
```

Treat that document as the initiative’s design and acceptance-criteria source of truth.

Where repository reality conflicts with the document:

* Do not guess.
* Document the conflict.
* Choose the smallest safe implementation.
* Explain the deviation in the final report.

## Completion report

At the end of an implementation task, report:

1. Architecture inspected
2. Files changed
3. Behaviour before and after
4. Tests and commands run
5. Results
6. Known limitations
7. Deferred follow-up work
8. Any deviations from the requested design

